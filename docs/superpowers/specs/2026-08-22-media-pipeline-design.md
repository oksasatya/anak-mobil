# Media pipeline and object storage — design

**Tickets:** [AM-358](https://oksasatyaa.atlassian.net/browse/AM-358) (Postgres job queue) ·
[AM-359](https://oksasatyaa.atlassian.net/browse/AM-359) (media pipeline and object storage) ·
[AM-270](https://oksasatyaa.atlassian.net/browse/AM-270) (the mobile half, subtasks AM-297 and AM-298)

**Date:** 2026-08-22
**Status:** design approved in outline, spec awaiting owner review

---

## Why this work, and why now

This is not the ticket that was picked up first. The session began on
[AM-115](https://oksasatyaa.atlassian.net/browse/AM-115) — the multi-vehicle garage — and reading
the backlog before designing anything is what moved it. Three findings, in the order they changed
the decision:

1. **AM-115's own AC1 asks for a photo on each vehicle card, and no photo exists anywhere.** There
   is no column, no endpoint, and no upload path. Vehicle photos are AM-119.
2. **Tapping a card should open the vehicle detail page**, which is AM-116 — a separate story, size
   L in `docs/mobile-feature-breakdown.md` (E2-4).
3. **AM-116 cannot be completed either, and is blocked harder than AM-115.** Checked against all
   fourteen migrations and every route in `adapter/http/mod.rs`: its hero photo has no source
   (AM-119), its Problems tab has no table and no endpoint (E5), its per-tab empty-state actions
   point at forms that do not exist (E3/E4 — every row in `features/shell/addActions.ts` still
   carries `href: null`), and its AI entry point has no screen, no endpoint, and no conversations
   table (E8).

So the real dependency chain is not `AM-116 → AM-115`. It is:

```
E12 media pipeline ──→ AM-119 photos ──┐
E5 problems ───────────────────────────┼──→ AM-116 detail ──→ AM-115 tap-to-detail
E3/E4 modification + service forms ────┤
E8 AI ─────────────────────────────────┘
```

The media pipeline is the root of the photo branch, and it is also what E3 modification photos, E4
service invoices, E5 problem media, E6 post images, and profile avatars all wait on. The feature
breakdown calls it out as `E12-1 … Build once, properly`. The codebase already names the two
tickets itself — `crates/runtime/src/lib.rs:219` logs
`"worker queue not wired yet — AM-358 (queue), AM-359 (media)"`.

**AM-358 comes before AM-359, and that is not a preference.** AM-359's AC2 requires heavy
processing to run in the worker role rather than on the HTTP request path, and its AC3 requires the
stable object keys that AM-358's queue contract depends on. The worker role exists
(`lib.rs:141-145`) but `run_worker()` only waits for a shutdown signal.

**This is three tickets and it is split deliberately.** One design covers the whole pipeline —
object key shape, the presign contract, and the client's retry all reference each other, and three
separate designs would drift apart. Implementation is three plans, executed in order:
AM-358 → AM-359 → AM-270.

---

## What is already decided and is not reopened here

These are committed in `README.md` and in code. This design fills in what is undecided; it does not
relitigate them.

- **One binary, two process roles.** `Role::Web` and `Role::Worker`, dispatched in `lib.rs`. Image
  compression is CPU-heavy, and running it inside the web process would starve HTTP and SSE during
  an upload spike. That is a blast-radius decision, not a step toward microservices.
- **The job queue is Postgres, not Redis.** The failures that matter — a worker dying mid-job, a
  duplicate notification, a poisoned message — need leases, retries, and dead-lettering either way.
  Redis does not remove that work; it only adds a service to operate.
- **Object storage is S3-compatible**, reached through an adapter, with the domain crate unaware it
  exists.

## What the owner decided in this session

- **Cloudflare R2** as the object store. S3-compatible, so `aws-sdk-s3` is used unchanged, and
  egress is free — which matters because photos are read far more often than they are written, and
  egress is the line item that grows with users rather than with storage.
- **Signed GET URLs with a five-minute TTL**, rather than an authorising relay. The trade-off is
  stated plainly below rather than hidden.

---

## Design

### 1. Object layout

One bucket, three prefixes, and each has a different lifetime and a different mutability.

```
staging/{media_id}                  the client's own bytes, writable by a presigned PUT
processing/{media_id}               an immutable snapshot the worker reads
media/{media_id}/{variant}.webp     what is served; variant ∈ {lg, md, thumb}
```

**Nothing that survives processing is the client's original bytes.** The worker decodes the
snapshot, re-encodes it, writes the variants, and deletes the staging and processing objects. This
is how AM-359 AC4 is met for the file that is *stored* as well as the one that is *served* — a
design that kept an `orig` object would satisfy the "served" half and quietly fail the "stored"
half, because the original is exactly the file carrying the owner's home coordinates.

**Reprocessing overwrites the same deterministic keys**, which is AC3 — but only for a variant set
that never changes, and that qualifier is load-bearing. See "Variant manifest" below.

### 2. Stripping location data is re-encoding

The worker decodes with the `image` crate and re-encodes. That encoder writes no EXIF at all, so
there is no separate strip step and no metadata library to keep current. The client also re-encodes
when it resizes, which is **defence in depth and never the guarantee**: the server does not trust
the client for a privacy property, in the same way it does not trust the client to hide a number
plate.

This matters more than it reads. A photo of a car taken in its owner's driveway carries the owner's
home coordinates. AM-270's AC3 and AM-359's AC4 both call this a privacy defence rather than a size
optimisation, and both are right.

### 3. Tables

```sql
-- AM-358
jobs(
  id, kind, payload jsonb, effect_key text,
  run_at, attempts, max_attempts,
  leased_until, leased_by,
  state,              -- queued | leased | done | dead
  last_error, created_at
)

-- AM-359
media(
  id, owner_user_id,
  state,              -- pending | processing | ready | failed
  mime, byte_size, width, height,
  variants text[],    -- the published set; see "Variant manifest"
  pending_expires_at, created_at, processed_at
)
```

**Attachment is not polymorphic.** Each consuming feature gets its own join table when its ticket
arrives — AM-119's would be `vehicle_photos(vehicle_id, media_id, position, is_cover)`. A single
`media(owner_type, owner_id)` table cannot carry a real foreign key, so a deleted vehicle would
leave its photos behind and nothing in the schema would notice. Per-feature join tables also give
AM-140's cover flag and ordering a natural home instead of a nullable column that means nothing for
five of the six owners.

**This design records that pattern and builds no join table.** The pattern is written down here so
AM-119 does not invent a polymorphic one; building it now would be building another ticket's schema.

### 4. The upload handshake

Three calls, and the state each one leaves behind is the part worth reading.

```
POST /media                  { mime, byte_size }
  → validate against a mime allowlist and a maximum size BEFORE minting anything
  → insert media row, state = pending, pending_expires_at = now() + 24h
  → return { id, upload_url, expires_at }
      upload_url = presigned PUT to staging/{id}, 15 min TTL, Content-Type SIGNED

PUT <upload_url>             the bytes, straight to R2
  → the web process never holds the file            (AM-359 AC1)

POST /media/{id}/complete
  → HEAD staging/{id}: capture ETag and real size
  → reject and delete when the real size is outside bounds, or the magic bytes are not an image
  → CopyObject staging/{id} → processing/{id}, conditional on that ETag
  → flip media.state = processing AND insert the media.process job
      IN ONE POSTGRES TRANSACTION
  → 202

worker
  → read processing/{id} only
  → decode under bounds, re-encode, write media/{id}/{variant}.webp
  → delete processing/{id} and staging/{id}
  → media.state = ready, real width and height recorded
```

`POST /media` is rate-limited per user. It mints a capability, which makes it a write path even
though it writes almost nothing.

**Why the copy to `processing/` exists, and it is not ceremony.** A presigned PUT is reusable until
it expires; R2 does not offer a single-use URL. Without the snapshot, anyone holding that URL —
including a mobile client whose network layer retried — can replace `staging/{id}` *after*
`complete` has been called. The first worker attempt would process file A, and a retry after the
lease expired would process file B, both landing on the same final key. "Deterministic key" would
then mean "last writer wins", which is the precise opposite of idempotent, and it is also a
content-swap vector. The conditional copy costs one R2 operation and closes both.

**Why the state flip and the job insert share a transaction.** Split across two operations, a crash
between them leaves the media in `processing` with no job to move it — no lease can rescue it,
because no lease was ever taken, and the client's UI waits forever.

**Why the size check happens at `complete` rather than at `POST /media`.** Validating the client's
declared `{mime, byte_size}` binds nothing about the bytes that follow. R2 supports signing an exact
`Content-Type`, which binds the header but not the content, and it does not support a presigned POST
policy with `content-length-range`. So the only real enforcement is after the fact: `HEAD` the object,
compare against the bounds, and verify the magic bytes before enqueueing anything.

### 5. Attaching is a separate, guarded step

A feature attaches by posting a `media_id`. The server refuses media that is not `ready`, and
refuses media whose `owner_user_id` is not the caller. Without the ownership check, one account can
attach another account's `media_id` to its own vehicle.

### 6. Serving

Signed GET URLs, **five-minute TTL**, generated server-side and embedded directly in the owning
resource's response (`vehicle.photos[].url`) so that viewing a photo costs no extra round trip.

Not a public bucket with unguessable keys: this product has a per-vehicle `cost_visibility` setting,
and "hard to guess" is not a visibility rule.

**The limitation, stated rather than buried.** A signed URL cannot be revoked before it expires. If
a photo is deleted or a vehicle is made private at 10:01, a URL issued at 10:00 keeps working until
10:05. Five minutes bounds that; it does not remove it. The design that removes it entirely is an
authorising relay — a Cloudflare Worker, or a proxying endpoint in axum — and the owner chose the
window over the extra component. Written here so the next reader knows it was a decision.

### 7. Bounds on decoding

The worker decodes bytes supplied by a stranger. A JPEG that is small on disk can declare hundreds
of millions of pixels; decoding it allocates accordingly, and an unbounded worker meets OOM,
gets its job re-leased, and dies again — taking the whole worker role down until the job
dead-letters.

Every decode is therefore bounded on: source bytes, declared width, declared height, total pixel
count, output bytes, wall-clock duration, and decode concurrency. The `image` crate's `Limits` is
the mechanism for the first several. The work runs inside a bounded `spawn_blocking` because it is
CPU-bound and would otherwise stall the worker's async runtime.

**A validation failure is non-retryable.** A malformed or oversized image will fail identically
every time, and retrying it eleven times before dead-lettering wastes the worker and delays every
other job. Transient failures (R2 unreachable, database gone) retry; content failures do not.

### 8. Variant manifest, and what "deterministic key" does not cover

Overwriting the same keys is idempotent only while the variant set is frozen. Drop `thumb`, or add
an `xl`, and the reprocess writes the new set while the old keys are simply left behind. After two
such changes a media prefix is mostly orphans.

So `media.variants` records the set actually published. After a successful reprocess, the worker
deletes `old − new`. AC3 is met by that cleanup contract, not by overwriting alone.

### 9. Cleaning up, and every state the pair can be left in

The R2 lifecycle rule on `staging/` is a **safety net for objects**, not the source of truth. The
database row is the source of truth, and a rule that only expires objects leaves an orphaned row on
every abandoned path:

| media row | R2 | How it happens | Cleaned by |
|---|---|---|---|
| `pending` | nothing | client got an id, never uploaded | `pending_expires_at` + cleanup job |
| `pending` | staging present | uploaded, never called `complete` | cleanup job, then lifecycle |
| `processing` | staging gone | lifecycle won the race | job fails non-retryably; cleanup job |
| `processing` | partial variants | worker died mid-write | retry overwrites; on dead, cleanup job |
| `failed` | some variants | poison input after partial work | cleanup job sweeps the whole prefix |
| `ready` | full set, staging gone | the only clean terminal state | — |

A `media.cleanup` job sweeps expired `pending` rows and terminal `failed` rows together with their
entire `media/{id}/` prefix.

**The staging lifecycle TTL must exceed lease + maximum backoff + maximum processing time.** Set it
shorter and the lifecycle rule deletes the input from underneath a worker that is still legitimately
retrying. 24 hours against a lease measured in minutes leaves ample margin, and the relationship is
recorded here because the two numbers live in different systems and nothing will connect them
automatically.

### 10. Queue mechanics (AM-358)

`SELECT … FOR UPDATE SKIP LOCKED` to claim, a `leased_until` timestamp, an `attempts` counter with
exponential backoff, and a `dead` state at the cap. One `jobs` table with a `kind` column and a JSONB
payload rather than typed per-kind tables — the acceptance criteria are about mechanics, not about
typing, and a table per kind buys nothing until a kind needs a column.

**Corrected 2026-08-22, during planning. This section originally said "a `leased_until` timestamp
that a reaper expires".** There is no reaper. A separate sweeper is a second writer racing the
claimer to produce a state the claim predicate can simply read for itself, so expiry folds into the
claim as `COALESCE(leased_until, run_at) <= now()` — which is also the expression the index is built
on. The contract the ticket asks for (a lease, and a lease that expires) is unchanged; only the
second process is gone.

**Corrected the same day: AC5's first number is the age of the oldest job still owed work —
`state IN ('queued','leased')`, not `state = 'queued'`.** The literal reading fails in precisely the
situation the metric exists for: a wedged worker holding every job in `leased` drives the count of
`queued` rows to zero, and the operator reads a healthy queue in the middle of the outage. The
second number is the count in `dead`.

**`effect_key` is what makes AC4 true for effects that are not files.** "Processing twice has the
same effect as once" is satisfied for media by overwriting a stable object key, and that argument
does not transfer to a push notification: if the worker dies after the provider accepts the push but
before the job is marked done, the retry sends a second one, and no object key helps. AM-358's own
technical note already asks for this — *"dedupe per efek samping"*. So a job may carry a stable
logical `effect_key`, and a consumer with a side effect outside the database records that key in the
same transaction as the effect, and uses the provider's own idempotency key where one exists.

Without provider-side idempotency support, the honest statement is that the queue is **at-least-once**
and the consumer is responsible for dedupe at the point of the effect.

**Strengthened 2026-08-22, during planning, because "builds the column and the contract" was not
enough.** A nullable text column plus a doc comment enforces nothing, which would have left AC4
entirely unmet by this ticket while reading as though it were handled. So `effect_key` also carries a
**partial unique index over the live states** (`queued`, `leased`): enqueuing the same live effect
twice is a no-op rather than a second job. That is real, testable now, and it closes a case this
pipeline actually has — a client calling `POST /media/{id}/complete` twice must not produce two
processing jobs. Dedupe *after* a job reaches a terminal state remains the consumer's, as above.

**A known gap, recorded rather than fixed.** The four states in §3 mean a `done` row is kept forever
and nothing ever reads it. This is not a correctness problem — the partial indexes keep both the
claim and the stats query off those rows — but there is no retention policy, and the plan flags it in
its own anti-goals rather than inventing one against an approved state set.

### 11. Quotas and retention

A per-user media quota and an explicit terminal retention period. Small numbers, operator-only, no
new service and no observability stack. Without them, one legitimate account with a retrying mobile
client can accumulate unbounded `pending` and `failed` rows and staging objects, and the only thing
in the way is a rate limit that was never designed to be a storage bound.

### 12. Local development

MinIO in `docker-compose.yml`, so `make be-test` runs with no cloud account and CI needs no
credentials. R2 and MinIO are both S3-compatible, so one `aws-sdk-s3` adapter serves both and the
only difference is an endpoint URL in configuration.

### 13. The mobile half (AM-270)

`expo-image-picker` to choose, `expo-image-manipulator` to resize and re-encode — none of the three
packages is installed yet, and installing them is part of AM-270 rather than an assumption this
design may make silently.

**Retry without re-picking (AM-297) is the part that needs specifying.** The local file URI and the
`media_id` are held together. A retry re-PUTs to the same presigned URL while it is still valid;
once it has expired, the client asks for a fresh presign **for the same `media_id`** rather than
calling `POST /media` again. Calling `POST /media` again is the obvious implementation and it is
wrong: it creates a second row, and the first becomes an orphan that only the cleanup job will ever
notice.

One shared component serves vehicle, build, problem, service, and profile uploads, which is AM-270
AC1 and the whole reason this is a cross-cutting ticket rather than five copies of an upload form.

---

## Tidak boleh ada — anti-goals

Absent by decision, not by oversight. A later reader can tell the difference.

- **No `orig` object retained.** There is no archive of the client's original bytes, so a future
  regeneration at higher quality has no source. That is the price of AC4 being true for stored files
  as well as served ones, and it was chosen knowingly.
- **No polymorphic attachment table.** No `media(owner_type, owner_id)`.
- **No join table built here.** `vehicle_photos` belongs to AM-119.
- **No video transcoding and no on-demand CDN image transforms.** Explicitly out of scope on AM-359.
- **No public bucket.**
- **No upload gateway.** Enforcing a size limit *before* the bytes land needs a Worker in front of
  R2. R2 ingress is free and there are no users; the check at `complete` is the answer for now.
- **No avatar upload, no local write outbox (AM-368), no distributed cron.**
- **No new observability stack.** AC5's two numbers come from the `jobs` table.
- **Nothing seeded with fake data**, here as everywhere.

---

## The cross-model review, and what it changed

The design was attacked by a second model (GPT-5.6-Terra) before it reached this document. Eight of
its nine objections were accepted, and two of them were holes rather than refinements:

1. **Staging stayed mutable after `complete`** — the presigned PUT is reusable, so the same final
   key could receive different bytes on a retry. Fixed by the conditional copy to `processing/`
   (§4).
2. **No bounds on decoding** — a pixel bomb takes the worker down and is re-leased into taking it
   down again. Fixed by explicit limits and a non-retryable classification (§7).

Also accepted: the transaction hole in `complete` (§4), lifecycle cleaning objects but never rows
plus the staging-TTL relationship (§9), first-call validation binding nothing about the bytes (§4),
deterministic keys not covering a changed variant set (§8), `effect_key` for non-file side effects
(§10), and the missing quota (§11).

**One objection was accepted as true but answered differently.** Signed GET URLs cannot be revoked
before expiry, which does sit awkwardly beside this product's rule that visibility is enforced
server-side. The proposed fix was an authorising relay; the owner chose a five-minute window
instead, and §6 records both the choice and what it costs.

**One objection failed.** The "over-built for zero users" attack does not land: direct-to-R2
uploads, a single Postgres queue, the lifecycle rule, and per-feature join tables are each demanded
by an acceptance criterion or protect private data. The under-build was the real finding — no quota,
no terminal retention.

---

## Verification

The gate is `make be-check` — `cargo fmt --check` → `cargo clippy --all-targets --all-features -D warnings`
→ `cargo test`. **This repository does not run Sonar**; clippy is the gate, and a brief that hands an
implementer a Sonar block is sending them to a tool that is not installed.

`make mb-check` covers the mobile side: `tsc --noEmit`, `expo lint`, and `bun test test/`. That
runner has no React renderer, so a component cannot be rendered — pure functions are tested
directly, and a rule that only exists inside a component is held by a source-text assertion, the
technique `test/session.test.ts` already uses.

**TDD verdicts, per piece:**

| Piece | Verdict | Why |
|---|---|---|
| Lease, backoff, dead-letter | **yes** | a clear input→output contract, and being wrong is expensive |
| Object key derivation, mime allowlist, size bounds | **yes** | pure functions with exact answers |
| Variant manifest diffing (`old − new`) | **yes** | a set operation, trivially testable, and silently wrong otherwise |
| Worker image processing | **no** | needs binary fixtures; verified by running it against real files |
| Mobile picker and uploader UI | **no** | visual; verified by opening it (§27) |

Integration tests follow `tests/garage_flow.rs`: a real database, a real HTTP client, and MinIO
standing in for R2.

---

## What the owner still owes

- An R2 bucket and its credentials, as `S3_*` keys in the root `.env`. **They are backend
  credentials and must never reach `apps/mobile/.env.*`** — anything prefixed `EXPO_PUBLIC_` is
  compiled into an APK that anyone can unzip.
- The quota numbers in §11, or an instruction to pick conservative defaults.
