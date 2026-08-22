# AM-358 — Postgres job queue with lease, backoff, and dead-letter

> **For agentic workers:** REQUIRED SUB-SKILL: use `executing-plans-hybrid` to run this
> plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the worker role a queue it can actually be trusted with — a `jobs` table
claimed with `FOR UPDATE SKIP LOCKED`, a lease that expires so a dead worker's job comes
back, an attempt counter with exponential backoff and a terminal `dead` state, an
`effect_key` contract for side effects that no object key can make idempotent, and two
numbers an operator can read.

**Architecture:** One table, one `kind` column, one JSONB payload. Claiming is a single
`UPDATE … WHERE id = (SELECT … FOR UPDATE SKIP LOCKED LIMIT 1)` that also increments
`attempts` and takes the lease, so exclusion, fairness, and attempt accounting are one
statement and cannot drift apart. Lease expiry is folded into the claim predicate rather
than swept by a separate reaper — `COALESCE(leased_until, run_at) <= now()` is "when is
this row next claimable", it is the indexed expression, and there is then no second timer
to race the claimer. Retry policy is pure Rust with unit tests; everything that touches a
clock uses Postgres's, because a lease compared against a worker's own clock is a lease
that skews.

**Tech Stack:** Rust 2024 (rust-version 1.96) · sqlx 0.9 with compile-time-checked macros
and a committed `.sqlx` cache · PostgreSQL 17 · tokio · `time` · `uuid` v7 · `serde_json`.
No new crate is added; the sqlx `json` feature is switched on.

**Spec:** [`docs/superpowers/specs/2026-08-22-media-pipeline-design.md`](../specs/2026-08-22-media-pipeline-design.md)
— §3 (tables), §7 (non-retryable validation failures), §9 (cleanup states and the staging
TTL relationship), §10 (queue mechanics and `effect_key`), and the `Tidak boleh ada` block.

**Closes:** [AM-358](https://oksasatyaa.atlassian.net/browse/AM-358)

**Branch:** `feat/AM-358-job-queue` — cut from `dev`, PR into `dev`.

**Out of this plan, deliberately:** AM-359 (media pipeline, the first real job kind) and
AM-270 (the mobile half). This plan ships a queue with **no job kinds**. That is not an
oversight and the code says so out loud: an unknown `kind` is a *permanent* failure that
dead-letters on the first attempt, because a build that does not know a kind will not
learn it by waiting.

---

## Global Constraints

Every task's requirements implicitly include this section.

- **`SQLX_OFFLINE=true` is how this repository builds.** The sqlx macros compile against
  the committed `.sqlx/` directory, not against a live database. **Every task that adds or
  changes a query runs `make be-prepare` and includes `.sqlx/` in its own diff.** CI runs
  `make be-sqlx-check` separately from `make be-check`, and a stale cache fails there
  having passed locally. `be-prepare` builds a throwaway *empty* database on purpose:
  sqlx infers nullability from the query plan, and a seeded database makes it decide
  columns are nullable when they are not.
- **`make be-check` is `be-fmt be-lint be-test be-boundary`.** It does **not** include
  `be-sqlx-check`. Run both.
- **This repository does not run Sonar. Clippy is the gate.** Any brief that hands an
  implementer a Sonar block is sending them to a tool that is not installed.
- **The integration tests report PASSING when `DATABASE_URL` and `REDIS_URL` are absent.**
  A green `cargo test` on a machine with no database proves nothing. Every task whose
  verification is an integration test states `make db-up` first and names an assertion
  that would go red.
- **`crates/domain` imports no framework.** `make be-boundary` proves it. Nothing in this
  plan touches that crate — see Task 2 for why the retry policy stays in `runtime`.
- **Repositories take `&mut PgConnection`, never a pool.** That is what lets a caller own
  the transaction boundary, and it is load-bearing here: spec §4 requires the media state
  flip and the job insert to be **one** transaction.
- **`TIMESTAMPTZ`, never `TIMESTAMP`. UUID v7 primary keys minted by the application.
  `created_at` and `updated_at` on every table, `updated_at` by the `set_updated_at()`
  trigger. Every foreign key gets an index. Closed sets are native enums.**
- **One migration per story, always `-r`.** A migration that has reached `dev` is never
  edited. While this branch is unmerged, unpushed, and nothing else is running against
  your development database, amend in place and `make db-drop`; the moment any of those
  three stops being true, the rule is absolute.
- **Do not commit and do not push.** Work accumulates in the working tree. Commits happen
  once at the end, after the owner reviews. Per-task commits are forbidden here.
- **Product-facing text is Bahasa Indonesia; everything written for developers is
  English** — code, comments, doc comments, this plan. Nothing in AM-358 is product-facing:
  the queue has no HTTP surface and `queue-stats` prints for an operator, in English.
- **Never log a payload, never store one in `last_error`, and never let a credential
  reach it either.** A payload can carry a media id today and something private tomorrow.
  `last_error` is truncated and holds only what the failure said.

  **CORRECTED 2026-08-22 after Task 2's review, and Tasks 4 and 5 depend on this.** The
  rule above was one word short and silent on two things the construction sites will hit:

  1. **`trim_error` must strip control characters, not merely truncate.** PostgreSQL
     `text` cannot hold `U+0000`; the parameter is rejected with `invalid byte sequence
     for encoding "UTF8": 0x00`. A Rust `String` can hold one, and the realistic source is
     AM-359's own territory — a decoder or `from_utf8_lossy` echoing bytes of a malformed
     upload. The statement that then fails is `UPDATE … SET last_error = $n`, which is the
     worst one in this queue to lose: the row stays `leased` with no error recorded and
     only returns five minutes later on lease expiry, so the symptom reads as a mysterious
     lease timeout rather than a bad message. Newlines and `\x1b[` escapes additionally
     forge log lines and drive an operator's terminal through `queue-stats`. The map goes
     BEFORE the take, so truncation still counts 500 visible characters:
     `.chars().map(|c| if c.is_control() { ' ' } else { c }).take(ERROR_MAX_CHARS)`.
  2. **A credential is not a payload, and the doc contract must say so.** Tasks 4 and 5
     will build the message from `format!("{err}")`, and a `reqwest` error's `Display`
     **includes the URL**. An R2 presigned PUT that times out therefore writes its
     `X-Amz-Signature` into `last_error`, readable by anyone with database access or
     `queue-stats`. The doc comment on the message field must read "never a payload **and
     never a credential**", because that comment is the whole contract the AM-359 author
     reads.

### The Postgres clock is the only clock

Every timestamp this plan writes — `run_at`, `leased_until` — is computed by Postgres as
`now() + ($n::double precision * interval '1 second')`, never by the worker and passed in.
With several worker replicas, a lease compared against each worker's own clock is a lease
whose duration depends on whose machine took it. `now()` is transaction start time, one
server, one clock.

### The numbers, and the relationships that bind them

| Constant | Value | Why, and what it is bound to |
|---|---|---|
| `max_attempts` (column default) | `8` | With the curve below, ~21 minutes of retrying. Long enough to ride out a short R2 or database outage. Spec §9 requires the staging lifecycle TTL to exceed *lease + maximum backoff + maximum processing time*: 5 min + 21 min + AM-359's decode bound ≈ **half an hour against a 24-hour TTL**. Raising `max_attempts` past ~14 is what would start eating that margin. |
| `BACKOFF_BASE_SECS` | `10` | First retry ten seconds later. |
| backoff curve | `min(10 · 2^(attempts−1), 900)` | 10s · 20s · 40s · 80s · 160s · 320s · 640s, then dead. |
| `BACKOFF_CAP_SECS` | `900` | Fifteen minutes. Never reached at `max_attempts = 8`; it exists so raising the cap later cannot produce a delay measured in days. |
| `LEASE` | `300` s | **Must exceed the longest a single job can take.** AM-359 bounds image decode on wall-clock; that bound must stay under five minutes, or a worker's own lease expires under it and a second worker starts the same job. The two numbers live in different modules and nothing connects them automatically — this row is the connection, exactly as spec §9 records the staging-TTL relationship for the same reason. |
| `IDLE_POLL` | `1` s | Worst-case latency from enqueue to start when the queue was empty. |
| `ERROR_MAX_CHARS` | `500` | `last_error` truncation, by characters not bytes. |

**No new configuration key.** `Config` keeps exactly `app_env`, `bind_addr`, `log_level`,
`database_url`, `redis_url`. Every number above is a constant because none of them differs
between development and production today, and a key nobody sets is a key that lies about
being tunable. The trigger for promoting one: more than one worker replica whose
concurrency genuinely has to differ per environment. Not before.

### Rust quality gate — paste verbatim into every task that writes Rust

```
# Rust quality gate — write compliant from the first commit (NO Sonar; clippy is the gate)

- clippy::too_many_arguments — ≤7 params (aim ≤5); past that, a params struct / builder.
- clippy::cognitive_complexity — extract named helpers; early-return `?`; flatten with `let ... else`.
- NO `.unwrap()` / `.expect()` / `panic!` / `todo!()` on production paths — deny clippy::unwrap_used +
  clippy::expect_used on non-test code; return `Result` + `?` / `ok_or_else` / `unwrap_or_default`.
  (Tests MAY unwrap/expect freely — the deny is #[cfg(test)]-exempt.)
- Duplicated string literal ≥3× → a module-level `const`.
- `#![forbid(unsafe_code)]` (domain + crate-wide unless a justified, `// SAFETY:`-documented exception).
- Errors: thiserror enums in domain/library; anyhow only at the app/bin boundary; `#[from]` + `.context()`;
  never `let _ = fallible();` (swallowed Result) — handle or `?`. Map domain→HTTP at ONE `IntoResponse` choke point.
- Async: never hold a std::sync::Mutex guard across `.await` (use tokio::sync::Mutex); never block the runtime
  (no std::thread::sleep / sync I/O / heavy CPU in async — use tokio::task::spawn_blocking or rayon); never drop a
  JoinHandle whose error matters; bound fan-out with tokio::sync::Semaphore(n).
- sqlx: parameterized (`$1`) or query!/query_as! macros — never `format!` into SQL.
- Verify before "done": make be-fmt → make be-lint → make be-test → make be-boundary → make be-prepare (when a
  query changed) → make be-sqlx-check.

When fixing one instance of a rule, scan sibling files for the same shape and fix-forward.
When reviewing, check the diff against this list BEFORE marking it compliant.
```

---

## Tidak boleh ada — anti-goals

Absen karena diputuskan, bukan karena kelupaan. Pembaca berikutnya harus bisa membedakan
keduanya.

- **Tidak ada cron terdistribusi antar replika worker.** Ini out-of-scope eksplisit di
  AM-358 sendiri. Menjalankan pekerjaan terjadwal di beberapa replika butuh leader
  election atau kunci per-slot; antrean ini tidak menyediakannya dan tidak berpura-pura.
- **Tidak ada Redis untuk antrean.** Sudah diputuskan di `README.md` dan diulang di bagian
  "What is already decided" spec: lease, retry, dan dead-letter tetap harus ditulis apa pun
  broker-nya, jadi Redis hanya menambah satu layanan untuk dioperasikan.
- **Tidak ada crate antrean pihak ketiga** — bukan `apalis`, bukan `faktory`, bukan
  `sqlxmq`. `sqlx` sudah ada dan cukup.
- **Tidak ada tabel per-`kind`.** Satu tabel `jobs`, satu kolom `kind`, satu payload JSONB.
  Tabel per-kind baru berguna kalau satu kind butuh kolom sendiri, dan belum ada satu pun.
- **Tidak ada tabel dedupe konsumen (`job_effects` atau sejenisnya).** AM-358 membangun
  kolom `effect_key` dan kontraknya; yang mencatat efek adalah konsumen, di transaksinya
  sendiri. Membangun tabelnya sekarang sama dengan membangun skema tiket orang lain —
  aturan yang sama yang menahan `vehicle_photos` di AM-119.
- **Tidak ada handler job nyata.** `media.process` milik AM-359. Kind yang tidak dikenal
  langsung `dead`, bukan di-retry delapan kali.
- **Tidak ada `LISTEN`/`NOTIFY`.** Polling satu detik cukup selama latensi enqueue→mulai
  tidak jadi keluhan.
- **Tidak ada konkurensi di dalam satu proses worker.** Satu job pada satu waktu. AC1
  bicara soal beberapa worker — itu beberapa proses, dan `SKIP LOCKED` yang menanganinya.
- **Tidak ada retensi atau pembersihan baris `done`.** Lihat catatan temuan di bawah:
  baris `done` menumpuk, dan indeks parsial membuat itu tidak menyentuh jalur panas.
- **Tidak ada endpoint HTTP untuk antrean.** AC5 dibaca lewat perintah CLI, sejalan dengan
  `grant-admin`: akses shell adalah otoritas yang lebih tinggi daripada sesi admin mana pun.
- **Tidak ada observability stack baru.** Dua angka AC5 dibaca dari tabel `jobs`.
- **Tidak ada `catch_unwind` di sekitar handler.** Handler yang panic mematikan proses
  worker; lease-nya yang menyelamatkan job itu, dan itu memang mekanisme AC2.
- **Tidak ada data palsu**, di sini seperti di mana pun.

---

## File structure

| File | Responsibility | Task |
|---|---|---|
| `apps/api/crates/runtime/migrations/<ts>_jobs.up.sql` | the `jobs` table, the `job_state` enum, three indexes, the constraints | 1 |
| `apps/api/crates/runtime/migrations/<ts>_jobs.down.sql` | drop both | 1 |
| `apps/api/crates/runtime/src/usecase/jobs.rs` | retry policy (pure), the worker loop, the failure taxonomy | 2, 4, 5 |
| `apps/api/crates/runtime/src/usecase/mod.rs` | `pub mod jobs;` | 2 |
| `apps/api/crates/runtime/src/adapter/postgres/job_repo.rs` | every SQL statement the queue runs | 3, 4, 6 |
| `apps/api/crates/runtime/src/adapter/postgres/mod.rs` | `pub mod job_repo;` | 3 |
| `apps/api/crates/runtime/Cargo.toml` | sqlx `json` feature; tokio `sync` in dev-dependencies | 3 |
| `apps/api/crates/runtime/src/lib.rs` | worker role body, `queue-stats` command, usage line | 5, 6 |
| `apps/api/crates/runtime/tests/job_queue.rs` | claim, lease expiry, backoff, dead-letter, effect-key, the loop | 3, 4, 5, 6 |
| `apps/api/CLAUDE.md` | a "The job queue" section — the at-least-once contract in the file a fresh agent reads | 6 |
| `apps/api/.sqlx/` | regenerated cache | 3, 4, 6 |

---

## Task 1: The `jobs` table

**Files:**
- Create: `apps/api/crates/runtime/migrations/<timestamp>_jobs.up.sql`
- Create: `apps/api/crates/runtime/migrations/<timestamp>_jobs.down.sql`

**Interfaces:**
- Consumes: nothing.
- Produces: the schema every later task queries — table `jobs`, enum
  `job_state ('queued','leased','done','dead')`, columns
  `id, kind, payload, effect_key, state, run_at, attempts, max_attempts, leased_until,
  leased_by, last_error, created_at, updated_at`, and the three indexes named below.

**Serves:** AC1, AC2, AC3, AC4, AC5 — all of them rest on this shape.

**TDD: no** — a migration has no unit under test. Verified by applying it (`make
be-migrate`), reversing it, and by the two existing tests in
`adapter/postgres/migrate.rs` (`embedded_migrations_are_unique_and_ordered`,
`every_migration_has_a_down`), which run in `make be-test` without a database.

**Minimality pass:** No `job_effects` table, no per-kind tables, no `priority` column, no
`scheduled_by`, no partitioning. `max_attempts` and `run_at` carry column defaults rather
than being parameters of every enqueue, because nothing today wants a different value —
which is what lets `NewJob` in Task 3 have three fields.

**Big O:**
- `jobs_claimable_idx` is a **partial** index on `COALESCE(leased_until, run_at)` covering
  only `state IN ('queued','leased')`. The claim in Task 3 is an ordered index scan with
  `LIMIT 1`: **O(log p) time**, where `p` is the number of *pending* rows — plus O(k) rows
  skipped, where `k` is the number of workers claiming simultaneously. Space **O(p)**.
- This is the answer to "what happens to the claim query as dead-letter rows accumulate":
  **nothing.** `done` and `dead` rows are outside the index predicate, so neither the time
  nor the space of the hot path depends on how many jobs have ever run. A non-partial
  index on `run_at` would have made the claim O(log n) over *every* row ever queued and
  grown without bound.
- `jobs_dead_idx` is partial on `state = 'dead'`, so AC5's `count(*)` is an index-only
  scan: **O(d)** in the number of dead rows, not O(n) over the table.

**Rust quality gate:** N/A — this task writes no Rust. The gate for it is `make be-migrate`
followed by a reversal, plus `make be-test`.

- [ ] **Step 1: Create the migration pair**

```bash
cd apps/api/crates/runtime && sqlx migrate add -r jobs
```

Always `-r`. Writing the down migration is what forces you to notice a change that cannot
be undone while choosing a different one is still cheap.

- [ ] **Step 2: Write the up migration**

Paste this into the generated `<timestamp>_jobs.up.sql`, replacing whatever the tool put
there:

```sql
-- The job queue. One table, one `kind`, one JSONB payload.
--
-- Postgres rather than Redis, and that is settled in README.md: the failures that
-- matter -- a worker dying mid-job, a duplicate push notification, a poisoned message
-- -- need leases, retries, and dead-lettering whatever the broker is. Redis removes
-- none of that work and adds a service to operate.
--
-- One table rather than one per kind, because the acceptance criteria are about
-- mechanics and not about typing. A table per kind earns its place when a kind needs a
-- column of its own; none does.

CREATE TYPE job_state AS ENUM ('queued', 'leased', 'done', 'dead');

CREATE TABLE jobs (
    id            UUID PRIMARY KEY,

    kind          TEXT        NOT NULL,
    payload       JSONB       NOT NULL DEFAULT '{}'::jsonb,

    -- A stable logical name for the SIDE EFFECT this job produces, when it has one
    -- outside the database.
    --
    -- "Processing twice has the same effect as once" is satisfied for a media file by
    -- overwriting a deterministic object key. That argument does not transfer to a push
    -- notification: if the worker dies after the provider accepted the push but before
    -- the job was marked done, the retry sends a second one and no object key helps.
    --
    -- So the queue is AT-LEAST-ONCE and says so. Two halves, and only the first is
    -- built here:
    --
    --   * ENQUEUE side, enforced below by jobs_one_live_per_effect: while a job with
    --     this key is queued or leased, enqueuing it again does nothing. That closes
    --     the double-`POST /media/{id}/complete` case without any consumer code.
    --   * EFFECT side, the consumer's own job: record the key in the SAME transaction
    --     as the effect, and use the provider's idempotency key where one exists. The
    --     notification ticket is the first consumer that needs it, and it brings its
    --     own table -- building one here would be building another ticket's schema.
    effect_key    TEXT,

    state         job_state   NOT NULL DEFAULT 'queued',

    -- When this job is next eligible. Set forward by the backoff on every transient
    -- failure.
    run_at        TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Incremented by the CLAIM, not by the settle. A job whose worker is killed mid-run
    -- has still burned an attempt, which is the only thing that makes the cap bite for a
    -- payload that crashes the process rather than returning an error.
    attempts      INTEGER     NOT NULL DEFAULT 0,
    max_attempts  INTEGER     NOT NULL DEFAULT 8,

    -- The lease. NOT NULL exactly while state = 'leased' -- see the constraint below,
    -- which is what makes COALESCE(leased_until, run_at) mean "next claimable at".
    leased_until  TIMESTAMPTZ,
    -- Which worker process holds it. A UUID minted at process start and logged once, so
    -- an operator can join this to a log line without this table needing a hostname.
    leased_by     UUID,

    -- What the last failure said, truncated by the application. Never a payload.
    last_error    TEXT,

    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT jobs_kind_present CHECK (length(btrim(kind)) > 0),
    CONSTRAINT jobs_attempts_sane CHECK (attempts >= 0 AND max_attempts >= 1),
    -- The invariant the claim index depends on. Without it a released job could keep a
    -- stale leased_until, and COALESCE(leased_until, run_at) would return a moment in
    -- the past that has nothing to do with when the job is due.
    CONSTRAINT jobs_lease_matches_state
        CHECK ((state = 'leased') = (leased_until IS NOT NULL))
);

-- "When is this row next claimable", indexed.
--
-- PARTIAL on the two live states, so the cost of claiming depends on how many jobs are
-- PENDING and not on how many have ever run. Done and dead rows are not in this index
-- at all, which is why a growing dead-letter set cannot slow the hot path down.
CREATE INDEX jobs_claimable_idx
    ON jobs (COALESCE(leased_until, run_at))
    WHERE state IN ('queued', 'leased');

-- At most one LIVE job per effect key. Same shape as part_merges_one_live_per_source.
--
-- Scoped to the live states on purpose: once a job is done or dead, the same logical
-- effect may legitimately be requested again -- a re-upload of the same media, a
-- notification for a second event. Terminal "has this effect ever happened" is the
-- consumer's question, answered in the consumer's own transaction.
CREATE UNIQUE INDEX jobs_one_live_per_effect
    ON jobs (effect_key)
    WHERE effect_key IS NOT NULL AND state IN ('queued', 'leased');

-- AC5's second number, as an index-only scan rather than a sequential scan of every job
-- the platform has ever run.
CREATE INDEX jobs_dead_idx
    ON jobs (created_at)
    WHERE state = 'dead';

CREATE TRIGGER jobs_set_updated_at
    BEFORE UPDATE ON jobs
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

COMMENT ON TABLE jobs IS
    'At-least-once job queue. Claimed with FOR UPDATE SKIP LOCKED; a lease that expires returns the job; attempts are capped and the cap is terminal (dead). A consumer with a side effect outside this database is responsible for its own dedupe -- see effect_key.';

COMMENT ON COLUMN jobs.effect_key IS
    'Stable logical name for the side effect. Unique among queued and leased rows, so enqueuing the same live effect twice is a no-op. Terminal dedupe is the consumer''s, recorded in the same transaction as the effect.';

COMMENT ON COLUMN jobs.attempts IS
    'Incremented when the job is CLAIMED, not when it fails, so a worker killed mid-run still burns an attempt.';
```

- [ ] **Step 3: Write the down migration**

`<timestamp>_jobs.down.sql`, matching the shape of `20260816170433_parts.down.sql`:

```sql
DROP TABLE IF EXISTS jobs;
DROP TYPE IF EXISTS job_state;
```

- [ ] **Step 4: Apply it, reverse it, apply it again**

```bash
make db-up
make be-migrate
```

Expected: the migration count in the log line goes from 14 to 15, and `migrations up to
date` follows. Then prove the down migration is real:

```bash
cd apps/api/crates/runtime && sqlx migrate revert
```

Expected: `Applied <timestamp>/revert jobs`. Confirm both objects are gone, then put them
back:

```bash
docker compose exec -T postgres psql -U postgres -d anakmobil -c "\d jobs"
# expected: Did not find any relation named "jobs".
docker compose exec -T postgres psql -U postgres -d anakmobil -c "\dT job_state"
# expected: List of data types (0 rows)
make be-migrate
```

- [ ] **Step 5: Run the migration unit tests**

```bash
make be-test
```

Expected: PASS, including `embedded_migrations_are_unique_and_ordered` and
`every_migration_has_a_down`. These two run without a database, so they are the one part
of this task that is verified in CI regardless.

**Acceptance criteria (a reviewer checks these against the diff):**
1. `job_state` has exactly four values in the spec's order: `queued`, `leased`, `done`, `dead`.
2. `jobs_claimable_idx` is **partial** (`WHERE state IN ('queued','leased')`) and indexes
   the expression `COALESCE(leased_until, run_at)`. A non-partial index or a plain
   `run_at` index is a rejection: it makes the claim's cost grow with total history.
3. `jobs_one_live_per_effect` is UNIQUE, partial on both `effect_key IS NOT NULL` and the
   two live states.
4. `jobs_lease_matches_state` is an equivalence (`=`), not a one-way implication.
5. `attempts` is documented as incremented on claim.
6. Both files exist; the down drops the table **and** the type; `sqlx migrate revert`
   succeeded and was re-applied.
7. Every timestamp column is `TIMESTAMPTZ`; the `set_updated_at()` trigger is wired.

---

## Task 2: Retry policy — pure functions

**Files:**
- Create: `apps/api/crates/runtime/src/usecase/jobs.rs`
- Modify: `apps/api/crates/runtime/src/usecase/mod.rs` — add `pub mod jobs;` in
  alphabetical position (between `garage` and `part_merge`)

**Interfaces:**
- Consumes: nothing. No database, no schema, no sqlx. This task can run **concurrently
  with Task 1**.
- Produces, for Tasks 4 and 5:
  ```rust
  pub enum JobFailure { Transient(String), Permanent(String) }
  impl JobFailure { pub fn message(&self) -> &str }
  pub type JobOutcome = Result<(), JobFailure>;
  pub const LEASE: Duration;                      // 300 s
  enum Settlement { Retry(Duration), Dead }       // private
  fn backoff(attempts: i32) -> Duration;          // private
  fn settle_for(attempts: i32, max_attempts: i32, failure: &JobFailure) -> Settlement;
  fn trim_error(message: &str) -> String;
  ```

**Serves:** AC3 (growing delay, terminal cap) and spec §7 (a validation failure is
non-retryable).

**TDD: yes** — pure functions with exact answers, and being wrong is expensive in a way
that never shows up in a demo. A backoff that overflows or a cap that is off by one is
invisible until production is retrying something forever.

**Where this does NOT go, and why that is a decision rather than laziness.** The brief
asked whether the retry policy belongs in `crates/domain`. It does not. The domain crate
is organised by **bounded context** — `ai`, `build`, `garage`, `identity`, `knowledge`,
`waitlist` — and a retry curve is not one of those; it is a platform mechanic that would
sit in that crate as an orphan module. The two properties the move would have bought are
already here: the functions are pure (no async, no I/O, no clock — `attempts` in,
`Duration` out) and they are unit-tested with no database. Moving them would buy the
`make be-boundary` assertion for code that has no framework import to begin with.

**Minimality pass:** No `RetryPolicy` struct, no builder, no trait, no per-kind policy
table. Three free functions and two small enums. `Settlement`, `backoff`, `settle_for`,
and `trim_error` are all **private** — only `run` in Task 5 calls them, and a `pub` on a
function with one in-crate caller is API nobody asked for.

**Big O:** every function here is O(1) in time and space. `backoff` clamps its shift
before shifting, so there is no input for which it is anything else.

**Rust quality gate:**

```
# Rust quality gate — write compliant from the first commit (NO Sonar; clippy is the gate)

- clippy::too_many_arguments — ≤7 params (aim ≤5); past that, a params struct / builder.
- clippy::cognitive_complexity — extract named helpers; early-return `?`; flatten with `let ... else`.
- NO `.unwrap()` / `.expect()` / `panic!` / `todo!()` on production paths — deny clippy::unwrap_used +
  clippy::expect_used on non-test code; return `Result` + `?` / `ok_or_else` / `unwrap_or_default`.
  (Tests MAY unwrap/expect freely — the deny is #[cfg(test)]-exempt.)
- Duplicated string literal ≥3× → a module-level `const`.
- `#![forbid(unsafe_code)]` (domain + crate-wide unless a justified, `// SAFETY:`-documented exception).
- Errors: thiserror enums in domain/library; anyhow only at the app/bin boundary; `#[from]` + `.context()`;
  never `let _ = fallible();` (swallowed Result) — handle or `?`. Map domain→HTTP at ONE `IntoResponse` choke point.
- Async: never hold a std::sync::Mutex guard across `.await` (use tokio::sync::Mutex); never block the runtime
  (no std::thread::sleep / sync I/O / heavy CPU in async — use tokio::task::spawn_blocking or rayon); never drop a
  JoinHandle whose error matters; bound fan-out with tokio::sync::Semaphore(n).
- sqlx: parameterized (`$1`) or query!/query_as! macros — never `format!` into SQL.
- Verify before "done": make be-fmt → make be-lint → make be-test → make be-boundary → make be-prepare (when a
  query changed) → make be-sqlx-check.

When fixing one instance of a rule, scan sibling files for the same shape and fix-forward.
When reviewing, check the diff against this list BEFORE marking it compliant.
```

- [ ] **Step 1: Create the module with its types and a failing test suite**

Create `apps/api/crates/runtime/src/usecase/jobs.rs`:

```rust
//! The job queue's policy and its worker loop.
//!
//! # At-least-once, said out loud
//!
//! A job can run more than once. That is not a defect to be engineered away — a worker
//! can die between finishing its work and recording that it finished, and no amount of
//! care inside this process closes that window. What the queue guarantees is that a job
//! is never *lost*: a lease that expires returns it, and a job that keeps failing stops
//! rather than retrying forever.
//!
//! What follows from that is the consumer's obligation. A side effect inside this
//! database is made idempotent by the transaction. A side effect *outside* it — an
//! object written to storage, a push handed to a provider — is the consumer's to dedupe,
//! and `jobs.effect_key` exists so it has a stable name to dedupe on.
//!
//! # Two kinds of failure, and the difference is the whole retry design
//!
//! [`JobFailure::Transient`] is the world being briefly unavailable: storage
//! unreachable, the database gone. It comes back, so the job comes back, after a delay
//! that grows.
//!
//! [`JobFailure::Permanent`] is the input being wrong: a malformed image, an unknown
//! job kind. It will fail identically on every attempt, so retrying it eight times
//! wastes the worker and delays every other job on the way to the same dead-letter.
//! Spec §7 makes this a requirement rather than an optimisation.

use std::time::Duration;

/// How long a claimed job is the claiming worker's, before another may take it.
///
/// **Must exceed the longest a single job can run.** AM-359 bounds image decoding on
/// wall-clock time; that bound has to stay under this number, or a worker's own lease
/// expires under it and a second worker starts the same job while the first is still
/// going. The two numbers live in different modules and nothing connects them
/// automatically, which is exactly why this sentence is here.
pub const LEASE: Duration = Duration::from_secs(300);

/// First retry delay. Doubles per attempt, up to [`BACKOFF_CAP_SECS`].
const BACKOFF_BASE_SECS: u64 = 10;

/// Fifteen minutes. Not reached at the default `max_attempts` of 8; it exists so that
/// raising the cap later cannot produce a delay measured in days.
const BACKOFF_CAP_SECS: u64 = 900;

/// How much of a failure message reaches `jobs.last_error`.
const ERROR_MAX_CHARS: usize = 500;

/// Why a job did not finish.
#[derive(Debug)]
pub enum JobFailure {
    /// The world was briefly unavailable. Retry after a growing delay.
    Transient(String),
    /// The input is wrong and will be wrong next time. Dead-letter immediately.
    Permanent(String),
}

impl JobFailure {
    /// What the failure said. Reaches `jobs.last_error`, truncated; never a payload.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Transient(message) | Self::Permanent(message) => message,
        }
    }
}

/// What a handler reports back to the loop.
pub type JobOutcome = Result<(), JobFailure>;

/// What the loop does with a failed job.
#[derive(Debug, PartialEq, Eq)]
enum Settlement {
    Retry(Duration),
    Dead,
}

/// How long to wait before the next attempt.
///
/// `attempts` is the attempt just consumed, one-based — the claim increments before the
/// handler runs — so the first retry waits [`BACKOFF_BASE_SECS`].
///
/// The shift is clamped before it happens. `attempts` comes from a database column that
/// nothing forbids from holding a large number, and `1u64 << 64` panics in a debug
/// build; clamping is cheaper than a `checked_shl` whose `None` arm would need a
/// meaning invented for it.
fn backoff(attempts: i32) -> Duration {
    let shift = attempts.saturating_sub(1).clamp(0, 20) as u32;
    let seconds = BACKOFF_BASE_SECS
        .saturating_mul(1_u64 << shift)
        .min(BACKOFF_CAP_SECS);
    Duration::from_secs(seconds)
}

/// Retry, or stop trying.
///
/// A permanent failure never retries, whatever the attempt count. A transient one
/// retries until the attempt just consumed reaches the cap — `>=` rather than `>`,
/// because `attempts` has already been incremented by the claim, so `attempts == 8`
/// with `max_attempts == 8` means the eighth and final attempt has just been spent.
fn settle_for(attempts: i32, max_attempts: i32, failure: &JobFailure) -> Settlement {
    match failure {
        JobFailure::Permanent(_) => Settlement::Dead,
        JobFailure::Transient(_) if attempts >= max_attempts => Settlement::Dead,
        JobFailure::Transient(_) => Settlement::Retry(backoff(attempts)),
    }
}

/// Cut a failure message down to what `last_error` will hold.
///
/// By characters, not bytes. Slicing a `&str` at a byte index that is not a character
/// boundary panics, and a message carrying Indonesian text or a file name is precisely
/// where a multi-byte character lands on the boundary.
fn trim_error(message: &str) -> String {
    message.chars().take(ERROR_MAX_CHARS).collect()
}
```

Add to `apps/api/crates/runtime/src/usecase/mod.rs`, in alphabetical order:

```rust
pub mod garage;
pub mod jobs;
pub mod part_merge;
```

- [ ] **Step 2: Write the failing tests**

Append to `apps/api/crates/runtime/src/usecase/jobs.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn transient() -> JobFailure {
        JobFailure::Transient("storage unreachable".to_owned())
    }

    fn permanent() -> JobFailure {
        JobFailure::Permanent("not an image".to_owned())
    }

    #[test]
    fn the_delay_doubles_from_the_base() {
        assert_eq!(backoff(1), Duration::from_secs(10));
        assert_eq!(backoff(2), Duration::from_secs(20));
        assert_eq!(backoff(3), Duration::from_secs(40));
        assert_eq!(backoff(7), Duration::from_secs(640));
    }

    #[test]
    fn the_delay_stops_growing_at_the_cap() {
        assert_eq!(backoff(20), Duration::from_secs(BACKOFF_CAP_SECS));
        // The regression this guards: `1u64 << (attempts - 1)` panics in a debug build
        // once the shift passes 63, and `attempts` is a database column.
        assert_eq!(backoff(i32::MAX), Duration::from_secs(BACKOFF_CAP_SECS));
    }

    #[test]
    fn a_zero_or_negative_attempt_count_does_not_panic() {
        assert_eq!(backoff(0), Duration::from_secs(10));
        assert_eq!(backoff(-5), Duration::from_secs(10));
    }

    #[test]
    fn a_transient_failure_retries_below_the_cap() {
        assert_eq!(
            settle_for(1, 8, &transient()),
            Settlement::Retry(Duration::from_secs(10))
        );
        assert_eq!(
            settle_for(7, 8, &transient()),
            Settlement::Retry(Duration::from_secs(640))
        );
    }

    #[test]
    fn the_last_attempt_dead_letters_rather_than_retrying() {
        // AC3's terminal state. `attempts` has already been incremented by the claim,
        // so 8 of 8 means the eighth attempt has been spent, not that a ninth is due.
        assert_eq!(settle_for(8, 8, &transient()), Settlement::Dead);
        assert_eq!(settle_for(9, 8, &transient()), Settlement::Dead);
    }

    #[test]
    fn a_permanent_failure_never_retries() {
        // Spec §7. A malformed image fails identically every time; eight attempts to
        // learn that delays every other job for twenty minutes.
        assert_eq!(settle_for(1, 8, &permanent()), Settlement::Dead);
    }

    #[test]
    fn a_long_message_is_truncated() {
        let long = "x".repeat(ERROR_MAX_CHARS * 2);
        assert_eq!(trim_error(&long).chars().count(), ERROR_MAX_CHARS);
    }

    #[test]
    fn truncation_never_splits_a_character() {
        // The panic this exists to prevent: `&s[..500]` on a string whose 500th byte is
        // mid-character. `€` is U+20AC — THREE bytes — so byte 500 lands inside the 167th
        // character (500 = 3·166 + 2) and a byte-based cut panics.
        //
        // CORRECTED 2026-08-22 after review. This test used to say `ø`, and asserted in a
        // comment that "every character here is three bytes". `ø` is U+00F8, which is
        // TWO bytes, so byte 500 fell on an exact character boundary (500 / 2 = 250) and
        // the byte-slicing implementation this test exists to forbid would not have
        // panicked at all — it would have returned 250 characters and reddened the count
        // assertion instead. The test still failed against a wrong implementation, so it
        // was never an assertion that could not fail; but the specific regression it
        // names was never exercised, and its own comment said otherwise.
        //
        // The doc comment on `trim_error` also used to justify this with "a message
        // carrying Indonesian text is precisely where a multi-byte character lands on the
        // boundary". Indonesian is Latin-script and overwhelmingly ASCII. The realistic
        // multi-byte source is a filename or an emoji echoed by an upstream error.
        let multibyte = "€".repeat(ERROR_MAX_CHARS + 50);
        let trimmed = trim_error(&multibyte);
        assert_eq!(trimmed.chars().count(), ERROR_MAX_CHARS);
    }

    #[test]
    fn a_short_message_survives_intact() {
        assert_eq!(trim_error("storage unreachable"), "storage unreachable");
    }

    #[test]
    fn a_failure_reports_its_own_message() {
        assert_eq!(transient().message(), "storage unreachable");
        assert_eq!(permanent().message(), "not an image");
    }
}
```

- [ ] **Step 3: Run them and watch them fail for the right reason**

Write the tests **before** the function bodies exist — replace each body with a
`Duration::ZERO` / `Settlement::Dead` / `String::new()` stub first, so the failure is an
assertion and not a missing symbol.

```bash
cd apps/api && cargo test --lib usecase::jobs
```

Expected: FAIL on `the_delay_doubles_from_the_base` with
`assertion \`left == right\` failed: left: 0ns, right: 10s`. A failure reading
`cannot find function` means the module is not wired into `usecase/mod.rs`.

- [ ] **Step 4: Fill in the bodies as written in Step 1, and watch them pass**

```bash
cd apps/api && cargo test --lib usecase::jobs
```

Expected: `test result: ok. 10 passed`.

- [ ] **Step 5: Run the gate**

```bash
make be-fmt && make be-lint && make be-test && make be-boundary
```

Expected: `backend gate green`. No `.sqlx` regeneration — this task adds no query.

**`dead_code` will fire here, and the fix is temporary.** `Settlement`, `backoff`,
`settle_for`, and `trim_error` are private and, until Task 5, have callers only inside
`#[cfg(test)]`. `cargo clippy --all-targets` also builds the library *without* that cfg,
so it reports them unused and `-D warnings` turns that into a failure. Put
`#[allow(dead_code)]` on exactly those four items — not `#[expect]`, which becomes an
*unfulfilled expectation* error the moment Task 5 supplies the caller, and not a
crate-level allow. **Task 5 deletes all four.** Record that in `## Execution status` so
it cannot be forgotten. `LEASE`, `JobFailure`, and `JobOutcome` are `pub` and need
nothing.

**Acceptance criteria:**
1. `backoff(i32::MAX)` returns the cap and does not panic in a debug build.
2. `settle_for(8, 8, Transient)` is `Dead`; `settle_for(7, 8, Transient)` is
   `Retry(640s)`.
3. `settle_for(1, 8, Permanent)` is `Dead` — a permanent failure never consults the cap.
4. `trim_error` counts characters, and the multi-byte test passes.
5. Nothing in this file imports `sqlx`, `axum`, or `tokio`. It is pure.
6. `Settlement`, `backoff`, `settle_for`, `trim_error` are private.

---

## Task 3: Enqueue and claim

**Files:**
- Create: `apps/api/crates/runtime/src/adapter/postgres/job_repo.rs`
- Modify: `apps/api/crates/runtime/src/adapter/postgres/mod.rs` — `pub mod job_repo;`
  (alphabetically, between `catalog_repo` and `migrate`)
- Modify: `apps/api/crates/runtime/Cargo.toml` — add `"json"` to sqlx's features and
  `"sync"` to tokio's **dev**-dependency features
- Create: `apps/api/crates/runtime/tests/job_queue.rs`
- Modify: `apps/api/.sqlx/` — regenerated

**Interfaces:**
- Consumes: the schema from Task 1.
- Produces, for Tasks 4, 5, 6 and for AM-359:
  ```rust
  pub struct Job {
      pub id: Uuid,
      pub kind: String,
      pub payload: serde_json::Value,
      pub effect_key: Option<String>,
      /// The attempt just consumed, one-based — incremented by this claim.
      pub attempts: i32,
      pub max_attempts: i32,
  }
  pub struct NewJob<'a> {
      pub kind: &'a str,
      pub payload: &'a serde_json::Value,
      pub effect_key: Option<&'a str>,
  }
  pub async fn enqueue(conn: &mut PgConnection, job: &NewJob<'_>)
      -> Result<Option<Uuid>, sqlx::Error>;
  pub async fn claim(conn: &mut PgConnection, worker: Uuid, lease: Duration)
      -> Result<Option<Job>, sqlx::Error>;
  ```

**Serves:** AC1 (exactly one worker per job), AC2 (an expired lease returns the job),
AC4's enqueue half (the same live `effect_key` cannot be queued twice).

**TDD: yes** — the claim is the contract this entire ticket is about, and its two
failure modes (two workers running one job; a job silently lost with its worker) are
invisible in a single-threaded happy path and expensive in production.

**`enqueue` takes a connection, not a pool.** This is the load-bearing signature of the
plan. Spec §4 requires `POST /media/{id}/complete` to flip `media.state` **and** insert
the job in **one** transaction: split across two, a crash between them leaves media in
`processing` with no job to move it, no lease can rescue it because no lease was ever
taken, and the client's UI waits forever. Taking a `PgPool` here would make that
impossible to write. There is deliberately **no** pool-taking convenience wrapper.

**Minimality pass:** `NewJob` has three fields. `run_at` and `max_attempts` come from
column defaults because nothing wants a different value; the retry path in Task 4 sets
`run_at` itself. No `enqueue_at`, no `enqueue_batch`, no priority. `ON CONFLICT` names
its arbiter index rather than using a bare `DO NOTHING`, so a primary-key collision is
still an error rather than being silently swallowed as a duplicate.

**Big O:**
- `enqueue`: one INSERT — **O(log p)** for `jobs_claimable_idx` and O(log e) for
  `jobs_one_live_per_effect`, where `p` is pending rows and `e` is live keyed rows.
  Space O(1).
- `claim`: an ordered index scan on `jobs_claimable_idx` with `LIMIT 1` —
  **O(log p + k)** time, where `k` is the number of rows skipped because another worker
  has them locked, bounded by the number of simultaneous claimers. Space O(1). It does
  **not** scale with `done` or `dead` rows, because the index is partial. Without
  `SKIP LOCKED` the same query would be O(log p) but would *serialise* every worker
  behind the first, turning N workers into one.

**Rust quality gate:**

```
# Rust quality gate — write compliant from the first commit (NO Sonar; clippy is the gate)

- clippy::too_many_arguments — ≤7 params (aim ≤5); past that, a params struct / builder.
- clippy::cognitive_complexity — extract named helpers; early-return `?`; flatten with `let ... else`.
- NO `.unwrap()` / `.expect()` / `panic!` / `todo!()` on production paths — deny clippy::unwrap_used +
  clippy::expect_used on non-test code; return `Result` + `?` / `ok_or_else` / `unwrap_or_default`.
  (Tests MAY unwrap/expect freely — the deny is #[cfg(test)]-exempt.)
- Duplicated string literal ≥3× → a module-level `const`.
- `#![forbid(unsafe_code)]` (domain + crate-wide unless a justified, `// SAFETY:`-documented exception).
- Errors: thiserror enums in domain/library; anyhow only at the app/bin boundary; `#[from]` + `.context()`;
  never `let _ = fallible();` (swallowed Result) — handle or `?`. Map domain→HTTP at ONE `IntoResponse` choke point.
- Async: never hold a std::sync::Mutex guard across `.await` (use tokio::sync::Mutex); never block the runtime
  (no std::thread::sleep / sync I/O / heavy CPU in async — use tokio::task::spawn_blocking or rayon); never drop a
  JoinHandle whose error matters; bound fan-out with tokio::sync::Semaphore(n).
- sqlx: parameterized (`$1`) or query!/query_as! macros — never `format!` into SQL.
- Verify before "done": make be-fmt → make be-lint → make be-test → make be-boundary → make be-prepare (when a
  query changed) → make be-sqlx-check.

When fixing one instance of a rule, scan sibling files for the same shape and fix-forward.
When reviewing, check the diff against this list BEFORE marking it compliant.
```

- [ ] **Step 1: Turn on two features that are currently off**

**sqlx `json`.** The queue's payload is `JSONB`, and **no jsonb column exists in this
schema today** — so `serde_json::Value` does not yet implement `sqlx::Type<Postgres>` and
a query touching `payload` fails to compile with
``the trait bound `Value: Type<Postgres>` is not satisfied``. In
`apps/api/crates/runtime/Cargo.toml`, change the sqlx line to:

```toml
sqlx = { version = "0.9.0", features = ["runtime-tokio", "tls-rustls-ring", "postgres", "macros", "uuid", "bigdecimal", "time", "json"] }
```

Verified present in `sqlx 0.9.0`'s own manifest. `serde_json` is already a direct
dependency of this crate, so nothing else changes.

**tokio `sync`, in dev-dependencies only.** The runtime's tokio features are
`rt-multi-thread`, `macros`, `signal`, `net`, `time` — `tokio::sync` is behind a feature
that is not among them, so `tokio::sync::Mutex` does not exist yet. Step 2's test file
needs one to serialise its own tests against a shared queue. It goes in
**dev-dependencies**, so the production dependency surface is unchanged:

```toml
[dev-dependencies]
http-body-util = "0.1.3"
tokio = { version = "1.53.1", features = ["test-util", "sync"] }
tower = { version = "0.5.3", features = ["util"] }
```

- [ ] **Step 1b: Confirm how `&serde_json::Value` binds**

`enqueue` in Step 4 binds `job.payload`, which is a `&serde_json::Value`. If the sqlx
macro rejects it, the one-line fix is to wrap it — `sqlx::types::Json(job.payload)` — and
**not** to change `NewJob` to own its payload, because a caller with a `Value` it is
about to reuse should not have to clone it. Record which was needed in
`## Execution status`; a later task should not have to rediscover it.

- [ ] **Step 2: Write the failing integration tests**

Create `apps/api/crates/runtime/tests/job_queue.rs`:

```rust
//! The job queue against a real Postgres.
//!
//! The unit tests in `usecase/jobs.rs` cover the retry curve and cannot touch the SQL,
//! and the SQL is where the interesting failures live: two workers claiming one job, a
//! lease that expires under a worker that died, an effect key queued twice. None of
//! that can be asserted against a fake, and the compiler checking the query proves only
//! that the columns exist.
//!
//! # Every test in this file runs one at a time, and that is not fastidiousness
//!
//! One of them runs the real worker loop, and the loop claims by availability alone — it
//! cannot filter by kind, because a worker has to take every kind there is. Run
//! concurrently with its neighbours, it would claim their jobs out from under them.
//!
//! `cargo` runs the tests inside one binary on parallel threads, so the serialisation
//! has to come from the file itself: every test takes [`QUEUE`] first. No other test
//! binary in this workspace touches `jobs`, so that lock is the whole fence.
//!
//! Two belts remain buckled anyway, because a development database can hold a row left
//! by an interrupted run: helpers release a job they did not enqueue rather than
//! deleting it, and the loop test's handler answers `Transient` for a kind that is not
//! its own.

#![expect(
    clippy::expect_used,
    reason = "test helpers abort rather than propagate"
)]

use anakmobil_runtime::adapter::postgres::job_repo::{self, NewJob};
use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Taken by every test in this file. See the module comment.
static QUEUE: Mutex<()> = Mutex::const_new(());

/// A pool, or a loud failure.
///
/// Same discipline as `tests/garage_flow.rs`: a missing `DATABASE_URL` used to `return`,
/// and cargo reported the test as PASSING, because it captures stderr for passing tests.
/// Skipping is now something you say out loud.
macro_rules! pool {
    () => {{
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            assert!(
                std::env::var("AM_SKIP_INTEGRATION").is_ok(),
                "DATABASE_URL is unset. Run `make be-test`, which loads .env. To skip \
                 the integration suites deliberately, set AM_SKIP_INTEGRATION=1."
            );
            eprintln!("SKIPPED: AM_SKIP_INTEGRATION is set");
            return;
        };
        let Ok(pool) = anakmobil_runtime::adapter::postgres::connect(&database_url) else {
            panic!("DATABASE_URL is set but unusable.");
        };
        if anakmobil_runtime::adapter::postgres::migrate::run(&pool)
            .await
            .is_err()
        {
            panic!(
                "could not migrate the test database. Is Postgres running? `make db-up`."
            );
        }
        pool
    }};
}

/// A job of a kind nothing handles, tagged so a test can find its own rows.
async fn a_queued_job(pool: &PgPool, kind: &str) -> Uuid {
    let payload = serde_json::json!({ "marker": Uuid::now_v7() });
    let mut conn = pool.acquire().await.expect("a connection");
    job_repo::enqueue(
        &mut conn,
        &NewJob {
            kind,
            payload: &payload,
            effect_key: None,
        },
    )
    .await
    .expect("enqueueing")
    .expect("a fresh job is never a duplicate")
}

/// Hand a job back to the queue without touching its attempt count.
///
/// Used on a job this test did not enqueue — a leftover from an interrupted run. It is
/// released rather than deleted, because deleting somebody else's row to tidy up is how
/// a test suite starts destroying evidence.
async fn release(pool: &PgPool, id: Uuid) {
    let mut conn = pool.acquire().await.expect("a connection");
    sqlx::query!(
        "UPDATE jobs SET state = 'queued', leased_until = NULL, leased_by = NULL \
         WHERE id = $1",
        id
    )
    .execute(&mut *conn)
    .await
    .expect("releasing a job");
}

/// Delete a job this test created.
async fn forget(pool: &PgPool, id: Uuid) {
    let mut conn = pool.acquire().await.expect("a connection");
    sqlx::query!("DELETE FROM jobs WHERE id = $1", id)
        .execute(&mut *conn)
        .await
        .expect("cleaning up");
}

/// Claim until `id` comes up, releasing anything else that appears on the way.
///
/// The claim cannot be told which job to take — that is the whole design — so a test
/// that wants a specific one has to claim its way to it.
async fn claim_this(pool: &PgPool, worker: Uuid, id: Uuid) -> job_repo::Job {
    let mut conn = pool.acquire().await.expect("a connection");
    for _ in 0..16 {
        let taken = job_repo::claim(&mut conn, worker, Duration::from_secs(300))
            .await
            .expect("claiming");
        let Some(job) = taken else {
            panic!("the queue emptied before job {id} came up");
        };
        if job.id == id {
            return job;
        }
        drop(conn);
        release(pool, job.id).await;
        conn = pool.acquire().await.expect("a connection");
    }
    panic!("job {id} never came up");
}

/// Claim repeatedly and assert `id` is never handed out, releasing whatever is.
async fn assert_unclaimable(pool: &PgPool, id: Uuid, why: &str) {
    let mut conn = pool.acquire().await.expect("a connection");
    for _ in 0..16 {
        let taken = job_repo::claim(&mut conn, Uuid::now_v7(), Duration::from_secs(300))
            .await
            .expect("claiming");
        let Some(other) = taken else { return };
        assert_ne!(other.id, id, "{why}");
        drop(conn);
        release(pool, other.id).await;
        conn = pool.acquire().await.expect("a connection");
    }
}

#[tokio::test]
async fn four_workers_claim_four_different_jobs() {
    // AC1. The property SKIP LOCKED buys is not "no duplicates" -- plain FOR UPDATE
    // would also produce none, by serialising every worker behind the first. It is that
    // four workers claiming at the same moment get four DIFFERENT jobs and none of them
    // waits. A serialising claim would show up here as claimers returning None, because
    // by the time they were let through the rows they wanted were leased.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let mut queued = Vec::new();
    for _ in 0..4 {
        queued.push(a_queued_job(&pool, "test.claim").await);
    }

    let mut claiming = tokio::task::JoinSet::new();
    for _ in 0..4 {
        let pool = pool.clone();
        claiming.spawn(async move {
            let mut conn = pool.acquire().await.expect("a connection");
            job_repo::claim(&mut conn, Uuid::now_v7(), Duration::from_secs(300))
                .await
                .expect("claiming")
        });
    }

    let mut claimed: Vec<Uuid> = Vec::new();
    while let Some(result) = claiming.join_next().await {
        if let Some(job) = result.expect("the claim task") {
            claimed.push(job.id);
        }
    }

    let mut unique = claimed.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        claimed.len(),
        unique.len(),
        "two workers claimed the same job: {claimed:?}"
    );
    assert_eq!(claimed.len(), 4, "a claimer came back empty on a full queue");

    for id in claimed {
        if !queued.contains(&id) {
            // A leftover from an interrupted run. Give it back.
            release(&pool, id).await;
        }
    }
    for id in queued {
        forget(&pool, id).await;
    }
}

#[tokio::test]
async fn a_live_lease_hides_the_job_from_everyone_else() {
    // AC1's other half, at one job rather than four.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let id = a_queued_job(&pool, "test.lease").await;

    let claimed = claim_this(&pool, Uuid::now_v7(), id).await;
    assert_eq!(claimed.id, id);

    assert_unclaimable(&pool, id, "a leased job was handed out twice").await;
    forget(&pool, id).await;
}

#[tokio::test]
async fn an_expired_lease_hands_the_job_to_the_next_worker() {
    // AC2, and the reason there is no separate reaper: expiry IS the claim predicate.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let id = a_queued_job(&pool, "test.expiry").await;

    // A zero lease expires at the instant it is taken. `claim_this` cannot be used here
    // -- it takes a five-minute lease -- so the first claim is made by hand.
    let mut conn = pool.acquire().await.expect("a connection");
    let first = loop {
        let taken = job_repo::claim(&mut conn, Uuid::now_v7(), Duration::ZERO)
            .await
            .expect("claiming")
            .expect("our job is queued");
        if taken.id == id {
            break taken;
        }
        drop(conn);
        release(&pool, taken.id).await;
        conn = pool.acquire().await.expect("a connection");
    };
    assert_eq!(first.attempts, 1, "the claim increments the attempt counter");
    drop(conn);

    // The second claim runs in a later transaction, so Postgres's now() has moved past
    // leased_until and the job is due again.
    let again = claim_this(&pool, Uuid::now_v7(), id).await;
    assert_eq!(
        again.attempts, 2,
        "a re-claim after an expired lease burns another attempt"
    );

    forget(&pool, id).await;
}

#[tokio::test]
async fn one_live_job_per_effect_key() {
    // AC4's enqueue half. Without this, `POST /media/{id}/complete` called twice queues
    // two jobs for one media, and "processing twice has the same effect as once" is
    // left resting entirely on the object key -- which spec §10 shows is not enough for
    // an effect that is not a file.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let key = format!("test.effect:{}", Uuid::now_v7());
    let payload = serde_json::json!({});
    let mut conn = pool.acquire().await.expect("a connection");

    let new = NewJob {
        kind: "test.effect",
        payload: &payload,
        effect_key: Some(key.as_str()),
    };
    let first = job_repo::enqueue(&mut conn, &new)
        .await
        .expect("enqueueing")
        .expect("the first is never a duplicate");
    let second = job_repo::enqueue(&mut conn, &new).await.expect("enqueueing");
    assert_eq!(second, None, "a live effect key must not queue twice");

    let count = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM jobs WHERE effect_key = $1"#,
        key.as_str()
    )
    .fetch_one(&mut *conn)
    .await
    .expect("counting");
    assert_eq!(count, 1);

    drop(conn);
    forget(&pool, first).await;
}

#[tokio::test]
async fn an_effect_key_is_reusable_once_its_job_is_terminal() {
    // The uniqueness is scoped to the LIVE states on purpose: a re-upload of the same
    // media, or a notification for a second event, must be able to queue again. A
    // constraint over every row would make the first job the last one forever.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let key = format!("test.effect:{}", Uuid::now_v7());
    let payload = serde_json::json!({});
    let mut conn = pool.acquire().await.expect("a connection");

    let new = NewJob {
        kind: "test.effect",
        payload: &payload,
        effect_key: Some(key.as_str()),
    };
    let first = job_repo::enqueue(&mut conn, &new)
        .await
        .expect("enqueueing")
        .expect("the first is never a duplicate");
    sqlx::query!("UPDATE jobs SET state = 'done' WHERE id = $1", first)
        .execute(&mut *conn)
        .await
        .expect("finishing it");

    let second = job_repo::enqueue(&mut conn, &new)
        .await
        .expect("enqueueing")
        .expect("a finished effect key must be reusable, or a re-upload is never processed");

    drop(conn);
    forget(&pool, first).await;
    forget(&pool, second).await;
}
```

- [ ] **Step 3: Run them and watch them fail**

```bash
make db-up
make be-test
```

Expected: FAIL to **compile** — `unresolved import
anakmobil_runtime::adapter::postgres::job_repo`. That is the correct first red: the
module does not exist yet. If instead the whole suite reports `ok` in 0.00s, the guard
is not firing and `DATABASE_URL` is unset — stop and fix that, because a green board
without a database proves nothing.

- [ ] **Step 4: Write the repository**

Create `apps/api/crates/runtime/src/adapter/postgres/job_repo.rs`:

```rust
//! The job queue's SQL.
//!
//! # Every function takes a connection, and that is the point
//!
//! Nothing here takes a `PgPool`. Spec §4 requires a caller to flip a media row's state
//! and insert its job in ONE transaction: crash between two separate operations and the
//! media sits in `processing` with no job to move it, which no lease can rescue because
//! no lease was ever taken. A repository holding its own pool would make that
//! transaction impossible to write.
//!
//! # "Next claimable at" is one indexed expression
//!
//! `COALESCE(leased_until, run_at)`. The migration's `jobs_lease_matches_state`
//! constraint is what makes that meaningful: `leased_until` is non-NULL exactly while
//! the job is leased, so the expression is the lease's expiry for a leased job and the
//! scheduled time for a queued one.
//!
//! That is also why there is no reaper. A separate task sweeping expired leases would
//! be a second writer racing the claimer for the same rows, to produce a state the
//! claim predicate can simply read.

use std::time::Duration;

use sqlx::PgConnection;
use uuid::Uuid;

/// A job, as the worker receives it.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: Uuid,
    pub kind: String,
    pub payload: serde_json::Value,
    /// See the column comment: a stable logical name for this job's side effect.
    pub effect_key: Option<String>,
    /// The attempt just consumed, one-based. The claim increments before the handler
    /// runs, so a worker killed mid-job has still spent this attempt.
    pub attempts: i32,
    pub max_attempts: i32,
}

/// What a caller supplies to queue work.
///
/// Three fields. `run_at` and `max_attempts` come from column defaults because nothing
/// today wants a different value, and the retry path sets `run_at` itself.
#[derive(Debug)]
pub struct NewJob<'a> {
    pub kind: &'a str,
    pub payload: &'a serde_json::Value,
    /// `None` means this job has no side effect worth deduping, and enqueuing it twice
    /// queues it twice.
    pub effect_key: Option<&'a str>,
}

/// Queue one job, inside the caller's transaction.
///
/// Returns `None` when an identical live effect key is already queued or leased — not
/// an error, because "this work is already on its way" is the outcome the caller wanted.
///
/// The `ON CONFLICT` names its arbiter index rather than using a bare `DO NOTHING`, so
/// a primary-key collision still surfaces as an error instead of being reported as a
/// duplicate effect. The `WHERE` clause restates the index predicate exactly, which is
/// what Postgres requires to infer a partial index as the arbiter.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the insert fails.
pub async fn enqueue(
    conn: &mut PgConnection,
    job: &NewJob<'_>,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        INSERT INTO jobs (id, kind, payload, effect_key)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (effect_key)
            WHERE effect_key IS NOT NULL AND state IN ('queued', 'leased')
            DO NOTHING
        RETURNING id
        "#,
        Uuid::now_v7(),
        job.kind,
        job.payload,
        job.effect_key,
    )
    .fetch_optional(conn)
    .await
}

/// Take the next due job, for `lease`.
///
/// One statement, and that is deliberate: exclusion, fairness, the lease, and the
/// attempt increment all happen together or none of them does. Split across a SELECT and
/// an UPDATE, two workers read the same row before either wrote.
///
/// `SKIP LOCKED` is what makes several workers *concurrent* rather than merely correct.
/// Plain `FOR UPDATE` also hands each job to one worker — by making every other worker
/// wait for the first, which is a queue with one consumer wearing a costume.
///
/// The lease is computed by Postgres, never by the caller. With several replicas, a
/// lease measured against each worker's own clock has a different length on each machine.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the statement fails.
pub async fn claim(
    conn: &mut PgConnection,
    worker: Uuid,
    lease: Duration,
) -> Result<Option<Job>, sqlx::Error> {
    sqlx::query_as!(
        Job,
        r#"
        UPDATE jobs SET
            state        = 'leased',
            leased_until = now() + ($2::double precision * interval '1 second'),
            leased_by    = $1,
            attempts     = attempts + 1
        WHERE id = (
            SELECT id FROM jobs
            WHERE state IN ('queued', 'leased')
              AND COALESCE(leased_until, run_at) <= now()
            ORDER BY COALESCE(leased_until, run_at)
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        RETURNING
            id           AS "id!",
            kind         AS "kind!",
            payload      AS "payload!",
            effect_key   AS "effect_key?",
            attempts     AS "attempts!",
            max_attempts AS "max_attempts!"
        "#,
        worker,
        lease.as_secs_f64(),
    )
    .fetch_optional(conn)
    .await
}
```

Add to `apps/api/crates/runtime/src/adapter/postgres/mod.rs`, alphabetically:

```rust
pub mod catalog_repo;
pub mod job_repo;
pub mod migrate;
```

- [ ] **Step 5: Regenerate the sqlx cache**

```bash
make be-prepare
```

Expected: `query data written to .sqlx in the current directory`. New `.sqlx/query-*.json`
files appear, including ones for the test file's queries — the Makefile passes
`-- --all-targets`, so test queries are cached too. **`.sqlx/` is part of this task's
diff.** Skipping this produces a branch that compiles locally and fails
`make be-sqlx-check` in CI.

- [ ] **Step 6: Run the tests and watch them pass**

```bash
make be-test
```

Expected: `test result: ok` for `job_queue`, with all five tests named and none reporting
`SKIPPED`.

- [ ] **Step 7: Run the full gate, including the one CI runs separately**

```bash
make be-check && make be-sqlx-check
```

Expected: `backend gate green`, then `query data up-to-date`.

**Acceptance criteria:**
1. `enqueue` takes `&mut PgConnection`. A `PgPool` parameter anywhere in this file is a
   rejection — it forecloses spec §4's transaction.
2. The claim is a single statement containing `FOR UPDATE SKIP LOCKED`, and it
   increments `attempts`.
3. `leased_until` is computed with Postgres's `now()`. A timestamp passed in from Rust
   is a rejection.
4. `four_workers_claim_four_different_jobs` passes and returns four claims, not one.
5. `an_expired_lease_hands_the_job_to_the_next_worker` passes and asserts
   `attempts == 2` on the re-claim.
6. `one_live_job_per_effect_key` and `an_effect_key_is_reusable_once_its_job_is_terminal`
   both pass — the second is what proves the uniqueness is scoped to live states.
7. `.sqlx/` is in the diff and `make be-sqlx-check` is green.
8. The sqlx `json` feature is on, tokio's `sync` was added to **dev**-dependencies only,
   and **no new crate** was added to either list.
9. Every test in `job_queue.rs` takes `QUEUE` as its first statement. A test that does
   not is a rejection — the loop test will steal its job.

---

## Task 4: Settling a claimed job — done, retry, dead

**Files:**
- Modify: `apps/api/crates/runtime/src/adapter/postgres/job_repo.rs` — three functions
- Modify: `apps/api/crates/runtime/tests/job_queue.rs` — three tests
- Modify: `apps/api/.sqlx/` — regenerated

**Interfaces:**
- Consumes: `job_repo::{Job, claim, enqueue}` (Task 3); the retry policy (Task 2).
- Produces, for Task 5:
  ```rust
  pub async fn mark_done(conn: &mut PgConnection, id: Uuid, worker: Uuid)
      -> Result<bool, sqlx::Error>;
  pub async fn reschedule(conn: &mut PgConnection, id: Uuid, worker: Uuid,
                          delay: Duration, error: &str) -> Result<bool, sqlx::Error>;
  pub async fn mark_dead(conn: &mut PgConnection, id: Uuid, worker: Uuid, error: &str)
      -> Result<bool, sqlx::Error>;
  ```
  Each returns `true` when it changed a row and `false` when the lease had already been
  lost to another worker.

**Serves:** AC3 (growing delay, terminal dead state after the cap).

**TDD: yes** — same reason as Task 3. "Retries forever" and "gave up on the first
hiccup" are both silent in a demo.

**Every settle is scoped to the worker that holds the lease.** `WHERE id = $1 AND state =
'leased' AND leased_by = $2`. This is the same discipline `vehicle_repo` applies to
ownership, for the same reason, and it closes a real race: worker A stalls past its lease,
worker B claims the job and starts running it, A wakes up and marks the job done — without
the `leased_by` predicate, A would terminate a job B is still executing, and B's own
settle would then be the one that silently did nothing. With it, A's UPDATE affects zero
rows, A logs that it lost its lease, and B owns the outcome.

**Minimality pass:** Three functions, each one statement. No `settle(outcome)` dispatcher
in the repository — choosing between them is a decision, and decisions live in
`usecase/jobs.rs` where they are unit-tested without a database. No history table for
attempts: `last_error` holds the most recent one, which is what an operator reads.

**Big O:** each is an UPDATE located by primary key — **O(log n)** time, O(1) space, plus
O(log p) for the two partial indexes the state change moves the row in or out of.

**Rust quality gate:**

```
# Rust quality gate — write compliant from the first commit (NO Sonar; clippy is the gate)

- clippy::too_many_arguments — ≤7 params (aim ≤5); past that, a params struct / builder.
- clippy::cognitive_complexity — extract named helpers; early-return `?`; flatten with `let ... else`.
- NO `.unwrap()` / `.expect()` / `panic!` / `todo!()` on production paths — deny clippy::unwrap_used +
  clippy::expect_used on non-test code; return `Result` + `?` / `ok_or_else` / `unwrap_or_default`.
  (Tests MAY unwrap/expect freely — the deny is #[cfg(test)]-exempt.)
- Duplicated string literal ≥3× → a module-level `const`.
- `#![forbid(unsafe_code)]` (domain + crate-wide unless a justified, `// SAFETY:`-documented exception).
- Errors: thiserror enums in domain/library; anyhow only at the app/bin boundary; `#[from]` + `.context()`;
  never `let _ = fallible();` (swallowed Result) — handle or `?`. Map domain→HTTP at ONE `IntoResponse` choke point.
- Async: never hold a std::sync::Mutex guard across `.await` (use tokio::sync::Mutex); never block the runtime
  (no std::thread::sleep / sync I/O / heavy CPU in async — use tokio::task::spawn_blocking or rayon); never drop a
  JoinHandle whose error matters; bound fan-out with tokio::sync::Semaphore(n).
- sqlx: parameterized (`$1`) or query!/query_as! macros — never `format!` into SQL.
- Verify before "done": make be-fmt → make be-lint → make be-test → make be-boundary → make be-prepare (when a
  query changed) → make be-sqlx-check.

When fixing one instance of a rule, scan sibling files for the same shape and fix-forward.
When reviewing, check the diff against this list BEFORE marking it compliant.
```

- [ ] **Step 1: Write the failing tests**

Append to `apps/api/crates/runtime/tests/job_queue.rs`:

```rust
/// What state is this job in? `::text` because the column is a native enum and the test
/// wants a string, not a Rust type mirroring it.
async fn state_of(pool: &PgPool, id: Uuid) -> String {
    let mut conn = pool.acquire().await.expect("a connection");
    sqlx::query_scalar!(r#"SELECT state::text AS "state!" FROM jobs WHERE id = $1"#, id)
        .fetch_one(&mut *conn)
        .await
        .expect("reading the state")
}

#[tokio::test]
async fn a_transient_failure_comes_back_and_a_delayed_one_does_not() {
    // AC3's first half: the delay is real. A reschedule with no delay is claimable at
    // once; a reschedule with a minute on it is not.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let worker = Uuid::now_v7();
    let id = a_queued_job(&pool, "test.retry").await;
    let job = claim_this(&pool, worker, id).await;
    assert_eq!(job.attempts, 1);

    let mut conn = pool.acquire().await.expect("a connection");
    let settled = job_repo::reschedule(&mut conn, id, worker, Duration::ZERO, "storage down")
        .await
        .expect("rescheduling");
    drop(conn);
    assert!(settled, "the lease holder must be able to settle its own job");
    assert_eq!(state_of(&pool, id).await, "queued");

    let again = claim_this(&pool, worker, id).await;
    assert_eq!(again.attempts, 2, "a retry burns another attempt");

    // Now push it into the future and confirm it hides.
    let mut conn = pool.acquire().await.expect("a connection");
    job_repo::reschedule(&mut conn, id, worker, Duration::from_secs(60), "storage down")
        .await
        .expect("rescheduling");
    drop(conn);
    assert_unclaimable(&pool, id, "a job with a future run_at must not be claimable").await;

    forget(&pool, id).await;
}

#[tokio::test]
async fn a_lost_lease_cannot_be_settled_by_the_worker_that_lost_it() {
    // The race the `leased_by` predicate closes. A stalls past its lease, B takes the
    // job, A wakes and tries to finish it. A must fail, not terminate B's work.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let stalled = Uuid::now_v7();
    let id = a_queued_job(&pool, "test.lostlease").await;
    claim_this(&pool, stalled, id).await;

    let mut conn = pool.acquire().await.expect("a connection");
    // B takes it over -- what an expired lease and a fresh claim would have produced.
    sqlx::query!(
        "UPDATE jobs SET leased_by = $2 WHERE id = $1",
        id,
        Uuid::now_v7()
    )
    .execute(&mut *conn)
    .await
    .expect("handing the lease over");

    let settled = job_repo::mark_done(&mut conn, id, stalled)
        .await
        .expect("marking done");
    drop(conn);
    assert!(
        !settled,
        "a worker that lost its lease must not be able to finish the job"
    );
    assert_eq!(state_of(&pool, id).await, "leased");

    forget(&pool, id).await;
}

#[tokio::test]
async fn a_dead_lettered_job_is_never_claimed_again() {
    // AC3's terminal half. `dead` is outside the claim index's predicate, so this is
    // structural rather than a filter somebody remembered to write.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let worker = Uuid::now_v7();
    let id = a_queued_job(&pool, "test.dead").await;
    claim_this(&pool, worker, id).await;

    let mut conn = pool.acquire().await.expect("a connection");
    let settled = job_repo::mark_dead(&mut conn, id, worker, "not an image")
        .await
        .expect("dead-lettering");
    assert!(settled);
    let error = sqlx::query_scalar!("SELECT last_error FROM jobs WHERE id = $1", id)
        .fetch_one(&mut *conn)
        .await
        .expect("reading the error");
    drop(conn);

    assert_eq!(state_of(&pool, id).await, "dead");
    assert_eq!(error.as_deref(), Some("not an image"));
    assert_unclaimable(&pool, id, "a dead job was claimed again").await;

    forget(&pool, id).await;
}
```

- [ ] **Step 2: Run them and watch them fail**

```bash
make db-up && make be-test
```

Expected: FAIL to compile — `no function or associated item named 'reschedule' found`.

- [ ] **Step 3: Write the three settle functions**

Append to `apps/api/crates/runtime/src/adapter/postgres/job_repo.rs`:

```rust
// The three settles below share one predicate: `id = $1 AND state = 'leased' AND
// leased_by = $2`. Scoped to the worker holding the lease, not merely to the job. A
// worker that stalled past its lease and woke to find another worker running its job
// must not be able to terminate that job — its UPDATE has to affect nothing, so it can
// report having lost the lease instead of silently colliding. Each returns `true` only
// when it actually changed a row.

/// Finish a job.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the statement fails.
pub async fn mark_done(
    conn: &mut PgConnection,
    id: Uuid,
    worker: Uuid,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        r#"
        UPDATE jobs SET state = 'done', leased_until = NULL, leased_by = NULL
        WHERE id = $1 AND state = 'leased' AND leased_by = $2
        "#,
        id,
        worker,
    )
    .execute(conn)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Return a job to the queue, due after `delay`.
///
/// The delay is applied against Postgres's clock for the same reason the lease is.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the statement fails.
pub async fn reschedule(
    conn: &mut PgConnection,
    id: Uuid,
    worker: Uuid,
    delay: Duration,
    error: &str,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        r#"
        UPDATE jobs SET
            state        = 'queued',
            run_at       = now() + ($3::double precision * interval '1 second'),
            leased_until = NULL,
            leased_by    = NULL,
            last_error   = $4
        WHERE id = $1 AND state = 'leased' AND leased_by = $2
        "#,
        id,
        worker,
        delay.as_secs_f64(),
        error,
    )
    .execute(conn)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Stop trying.
///
/// Terminal. `dead` sits outside `jobs_claimable_idx`'s predicate, so the job is not
/// merely filtered out of the claim — it is not in the index the claim reads.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the statement fails.
pub async fn mark_dead(
    conn: &mut PgConnection,
    id: Uuid,
    worker: Uuid,
    error: &str,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        r#"
        UPDATE jobs SET
            state        = 'dead',
            leased_until = NULL,
            leased_by    = NULL,
            last_error   = $3
        WHERE id = $1 AND state = 'leased' AND leased_by = $2
        "#,
        id,
        worker,
        error,
    )
    .execute(conn)
    .await?;
    Ok(done.rows_affected() == 1)
}
```

- [ ] **Step 4: Regenerate the cache and run the tests**

```bash
make be-prepare && make be-test
```

Expected: `test result: ok` for `job_queue`, now eight tests.

- [ ] **Step 5: Full gate**

```bash
make be-check && make be-sqlx-check
```

Expected: `backend gate green`, then `query data up-to-date`.

**Acceptance criteria:**
1. All three statements carry `AND state = 'leased' AND leased_by = $2`. Missing
   `leased_by` is a rejection — it is the race the test named after it proves.
2. All three return `bool` derived from `rows_affected() == 1`.
3. `reschedule` computes `run_at` with Postgres's `now()`.
4. `a_lost_lease_cannot_be_settled_by_the_worker_that_lost_it` passes and the job is
   still `leased` afterwards.
5. `a_dead_lettered_job_is_never_claimed_again` passes and `last_error` holds the message.
6. `.sqlx/` is in the diff.

---

## Task 5: The worker loop, and the worker role

**Files:**
- Modify: `apps/api/crates/runtime/src/usecase/jobs.rs` — the loop, the constants it needs
- Modify: `apps/api/crates/runtime/src/lib.rs` — `run_worker`'s body and doc comment, the
  `dispatch` handler
- Modify: `apps/api/crates/runtime/tests/job_queue.rs` — one test
- Modify: `apps/api/.sqlx/` — only if a query changed (it should not)

**Interfaces:**
- Consumes: everything from Tasks 2, 3, 4.
- Produces:
  ```rust
  pub use crate::adapter::postgres::job_repo::Job;
  pub async fn run<H, Fut>(pool: &PgPool, handle: H, shutdown: impl Future<Output = ()>)
  where H: Fn(Job) -> Fut, Fut: Future<Output = JobOutcome>;
  ```

**Serves:** AC1, AC2, AC3 end to end — this is the thing that makes them observable in a
running process rather than only in a repository call.

**TDD: yes** — the loop has a checkable contract (it drains what is queued, it settles
what it ran, it stops on the signal *between* jobs and not during one), and the failure
mode of getting the shutdown check wrong is abandoning a job mid-flight, which looks like
success in a log.

**Deliberately sequential: one job at a time, one worker per process.** AC1 says several
workers, and several workers is several *processes* — which `SKIP LOCKED` already serves.
In-process fan-out is a different axis with its own cost (a `Semaphore`, a `JoinSet`,
bounded concurrency, and a settle path that can no longer assume one outstanding job) and
nothing measures it as necessary at zero users. The upgrade path is named in a
`ponytail:` comment in the code, with its trigger: when one worker replica is
demonstrably the bottleneck.

**The shutdown check sits at the top of each iteration, never inside `tokio::select!`
against the handler.** `select!` cancels the losing branch, so a signal arriving mid-job
would **drop the handler's future at whatever await point it had reached** — a half-done
job whose lease is still held. Instead: check whether the signal has already arrived
(with a `biased` select against `std::future::ready`), and if it has not, run the job to
completion. A process killed mid-job is what the lease is for.

**Minimality pass:** No handler registry, no trait, no `dyn` dispatch, no
`Box<dyn Future>`. `run` is generic over a closure; `lib.rs` passes a plain `async fn`.
No `catch_unwind` — a panicking handler kills the worker and the lease returns its job,
which is AC2 working rather than a gap. No heartbeat table (see below).

**The heartbeat promise in `lib.rs` is answered by AC5, not by new machinery.** The
current doc comment says a wedged worker needs "a progress heartbeat from the loop
itself, so it arrives with the loop in AM-358". It does arrive — as the *age of the
oldest pending job* in Task 6. A wedged loop stops settling, so that number climbs
without bound, which is exactly what a heartbeat would have told an operator, using a
column that already exists. Rewrite the comment to say so rather than leaving a promise
that reads as unkept.

**Big O:** per iteration, one claim (**O(log p + k)**) plus one settle (**O(log n)**).
Idle cost is one claim per second per worker — O(log p) against a partial index, which is
an index-only probe returning nothing when the queue is empty. Space O(1): exactly one
`Job` is held at a time, and the payload is the only unbounded part of it.

**Rust quality gate:**

```
# Rust quality gate — write compliant from the first commit (NO Sonar; clippy is the gate)

- clippy::too_many_arguments — ≤7 params (aim ≤5); past that, a params struct / builder.
- clippy::cognitive_complexity — extract named helpers; early-return `?`; flatten with `let ... else`.
- NO `.unwrap()` / `.expect()` / `panic!` / `todo!()` on production paths — deny clippy::unwrap_used +
  clippy::expect_used on non-test code; return `Result` + `?` / `ok_or_else` / `unwrap_or_default`.
  (Tests MAY unwrap/expect freely — the deny is #[cfg(test)]-exempt.)
- Duplicated string literal ≥3× → a module-level `const`.
- `#![forbid(unsafe_code)]` (domain + crate-wide unless a justified, `// SAFETY:`-documented exception).
- Errors: thiserror enums in domain/library; anyhow only at the app/bin boundary; `#[from]` + `.context()`;
  never `let _ = fallible();` (swallowed Result) — handle or `?`. Map domain→HTTP at ONE `IntoResponse` choke point.
- Async: never hold a std::sync::Mutex guard across `.await` (use tokio::sync::Mutex); never block the runtime
  (no std::thread::sleep / sync I/O / heavy CPU in async — use tokio::task::spawn_blocking or rayon); never drop a
  JoinHandle whose error matters; bound fan-out with tokio::sync::Semaphore(n).
- sqlx: parameterized (`$1`) or query!/query_as! macros — never `format!` into SQL.
- Verify before "done": make be-fmt → make be-lint → make be-test → make be-boundary → make be-prepare (when a
  query changed) → make be-sqlx-check.

When fixing one instance of a rule, scan sibling files for the same shape and fix-forward.
When reviewing, check the diff against this list BEFORE marking it compliant.
```

- [ ] **Step 1: Write the failing test**

Append to `apps/api/crates/runtime/tests/job_queue.rs`:

```rust
#[tokio::test]
async fn the_loop_drains_what_it_is_given_and_stops_when_told() {
    // The loop's whole contract in one test: it claims, it runs the handler, it settles
    // the result, and it stops on the signal BETWEEN jobs rather than abandoning one
    // mid-flight. The third job's handler fires the signal, so if the loop checked
    // shutdown inside a select! against the handler, this job would be dropped
    // un-settled and the assertion below would find it still leased.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let mine: Vec<Uuid> = {
        let mut ids = Vec::new();
        for _ in 0..3 {
            ids.push(a_queued_job(&pool, "test.loop").await);
        }
        ids
    };

    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let stop = std::sync::Mutex::new(Some(stop));
    let handled = std::sync::Mutex::new(Vec::<Uuid>::new());

    anakmobil_runtime::usecase::jobs::run(
        &pool,
        |job| {
            // These two rebindings are load-bearing, not noise. Without them the
            // `async move` block below would move `handled` and `stop` out of the
            // closure's environment, making the closure `FnOnce` — and `run` needs
            // `Fn`. Taking a reference first means the block moves a `&`, which is Copy.
            let handled = &handled;
            let stop = &stop;
            async move {
                if job.kind != "test.loop" {
                    // A leftover from an interrupted run. Transient, so it goes back to
                    // the queue rather than being finished or killed on somebody else's
                    // behalf. QUEUE already stops the concurrent case.
                    return Err(anakmobil_runtime::usecase::jobs::JobFailure::Transient(
                        "not this test's job".to_owned(),
                    ));
                }
                let count = {
                    let mut seen = handled.lock().expect("the handled list");
                    seen.push(job.id);
                    seen.len()
                };
                // CORRECTED 2026-08-22 during execution. This was written as a nested
                // `if count == 3 { if let Some(sender) = ... }`, which trips
                // `clippy::collapsible_if` — and this repository lints with `-D warnings`,
                // so the plan's own literal code did not pass its own gate. Let-chains are
                // stable at this crate's `rust-version = 1.96`.
                if count == 3
                    && let Some(sender) = stop.lock().expect("the stop channel").take()
                {
                    let _ = sender.send(());
                }
                Ok(())
            }
        },
        async {
            let _ = stopped.await;
        },
    )
    .await;

    let seen = handled.into_inner().expect("the handled list");
    assert_eq!(seen.len(), 3, "the loop must drain what it was given");
    for id in &mine {
        assert!(seen.contains(id), "job {id} was never handled");
        assert_eq!(
            state_of(&pool, *id).await,
            "done",
            "the loop must settle a job it ran, even the one that triggered shutdown"
        );
    }

    for id in mine {
        forget(&pool, id).await;
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
make db-up && make be-test
```

Expected: FAIL to compile — `cannot find function 'run' in module
anakmobil_runtime::usecase::jobs`.

- [ ] **Step 3: Write the loop**

Append to `apps/api/crates/runtime/src/usecase/jobs.rs`, and add the imports it needs at
the top of the file (`sqlx::PgPool`, `uuid::Uuid`, `crate::adapter::postgres::job_repo`):

```rust
pub use crate::adapter::postgres::job_repo::Job;

/// How long to wait before asking again, when the queue was empty.
const IDLE_POLL: Duration = Duration::from_secs(1);

/// Claim, run, settle, repeat — until the shutdown signal arrives.
///
/// # One job at a time
///
/// AC1's "several workers at once" is several *processes*, which the claim's
/// `SKIP LOCKED` already serves. In-process concurrency is a separate axis, and it is
/// not free: a semaphore to bound it, a `JoinSet` to collect it, and a settle path that
/// can no longer assume one outstanding job.
///
/// ponytail: sequential on purpose. When one replica is measurably the bottleneck, the
/// upgrade is a `tokio::sync::Semaphore` around a `JoinSet` of claim-run-settle tasks,
/// each with its own connection — not a second queue.
///
/// # Where the shutdown check sits, and why it is not a `select!` around the handler
///
/// `tokio::select!` cancels the branch that loses, so a signal arriving while a handler
/// is running would drop that handler's future at whatever await point it had reached —
/// a job half-done, its lease still held, and a log line saying the worker shut down
/// cleanly. So the signal is only *checked* between jobs. A process killed mid-job is
/// what the lease exists for: the job comes back.
///
/// # A database failure does not kill the worker
///
/// It logs and waits. A worker that exits on every hiccup becomes a restart loop whose
/// recovery is slower than simply asking again a second later, and a settle that fails
/// leaves the job leased — which the lease then returns. At-least-once, working as
/// described.
pub async fn run<H, Fut>(pool: &PgPool, handle: H, shutdown: impl Future<Output = ()>)
where
    H: Fn(Job) -> Fut,
    Fut: Future<Output = JobOutcome>,
{
    let worker = Uuid::now_v7();
    // Logged once so an operator can join `jobs.leased_by` to a process without this
    // table needing to carry a hostname.
    tracing::info!(%worker, "worker loop started");

    let mut shutdown = std::pin::pin!(shutdown);

    loop {
        let stopping = tokio::select! {
            biased;
            () = shutdown.as_mut() => true,
            () = std::future::ready(()) => false,
        };
        if stopping {
            break;
        }

        match claim_one(pool, worker).await {
            Ok(Some(job)) => run_one(pool, worker, &handle, job).await,
            Ok(None) => {
                tokio::select! {
                    () = shutdown.as_mut() => break,
                    () = tokio::time::sleep(IDLE_POLL) => {}
                }
            }
            Err(err) => {
                tracing::error!(cause = %err, "could not claim a job");
                tokio::select! {
                    () = shutdown.as_mut() => break,
                    () = tokio::time::sleep(IDLE_POLL) => {}
                }
            }
        }
    }

    tracing::info!(%worker, "worker loop stopped");
}

/// Take the next job, or report that there was none.
async fn claim_one(pool: &PgPool, worker: Uuid) -> Result<Option<Job>, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    job_repo::claim(&mut conn, worker, LEASE).await
}

/// Run one job and record what happened.
///
/// Extracted from [`run`] so neither has a shape clippy's cognitive-complexity lint has
/// to argue about, and so the settle decision reads as one `match` rather than as three
/// arms nested inside a loop.
async fn run_one<H, Fut>(pool: &PgPool, worker: Uuid, handle: &H, job: Job)
where
    H: Fn(Job) -> Fut,
    Fut: Future<Output = JobOutcome>,
{
    let id = job.id;
    let attempts = job.attempts;
    let max_attempts = job.max_attempts;

    let outcome = handle(job).await;

    if let Err(err) = settle(pool, worker, id, attempts, max_attempts, outcome).await {
        // The job stays leased. Its lease expires and another worker takes it — which
        // is the at-least-once contract doing its job, not a leak.
        tracing::error!(job = %id, cause = %err, "could not record a job's outcome");
    }
}

/// Write the outcome to the row.
///
/// Six parameters, under the workspace's `too_many_arguments` threshold of seven and
/// over the ≤5 this repository aims for. Left as parameters deliberately: a struct
/// bundling them would have exactly one construction site and one read site, which is
/// an abstraction with no second caller to justify it. If a reviewer disagrees, fold
/// this back into `run_one` and take the cognitive-complexity cost instead — do not add
/// the struct.
async fn settle(
    pool: &PgPool,
    worker: Uuid,
    id: Uuid,
    attempts: i32,
    max_attempts: i32,
    outcome: JobOutcome,
) -> Result<(), sqlx::Error> {
    let mut conn = pool.acquire().await?;

    let settled = match outcome {
        Ok(()) => job_repo::mark_done(&mut conn, id, worker).await?,
        Err(failure) => {
            let message = trim_error(failure.message());
            match settle_for(attempts, max_attempts, &failure) {
                Settlement::Retry(delay) => {
                    tracing::warn!(
                        job = %id,
                        attempts,
                        retry_in_s = delay.as_secs(),
                        "job failed, retrying"
                    );
                    job_repo::reschedule(&mut conn, id, worker, delay, &message).await?
                }
                Settlement::Dead => {
                    tracing::warn!(job = %id, attempts, "job dead-lettered");
                    job_repo::mark_dead(&mut conn, id, worker, &message).await?
                }
            }
        }
    };

    if !settled {
        // Another worker holds this job now, which means this one stalled past its own
        // lease and the work has just been done twice. Worth saying out loud: it is the
        // signal that LEASE is shorter than a real job takes.
        tracing::warn!(job = %id, %worker, "lost the lease before settling");
    }

    Ok(())
}
```

**Delete the four `#[allow(dead_code)]` attributes added in Task 2.** `Settlement`,
`backoff`, `settle_for`, and `trim_error` all have a non-test caller now, and an
attribute that is no longer true is how the next real dead symbol goes unnoticed.

- [ ] **Step 4: Wire the worker role**

In `apps/api/crates/runtime/src/lib.rs`, change the match arm:

```rust
    match role {
        Role::Web => run_web(&config).await?,
        Role::Worker => run_worker(&config).await?,
        Role::Migrate => run_migrate(&config).await?,
    }
```

Replace the whole of `run_worker` — body and doc comment — with:

```rust
/// Background worker role.
///
/// Claims from the Postgres queue, runs the job, and records the outcome; a lease that
/// expires hands an unfinished job to the next worker. See [`usecase::jobs`] for the
/// retry curve, the failure taxonomy, and why the loop is sequential.
///
/// Deliberately does **not** run migrations. One role owns applying them, and a worker
/// starting against a schema the web role has not migrated yet is exactly the case
/// expand-and-contract makes safe: a column is added in one release and only removed in
/// a later one, so an older reader keeps working.
///
/// No HTTP probe port here, deliberately. One was designed and dropped: a listener
/// running beside a job loop answers `200` while that loop is deadlocked, which is
/// precisely the failure it would have been added to catch.
///
/// The comment this replaces promised a progress heartbeat instead, "arriving with the
/// loop in AM-358". It has arrived, and it needed no new machinery: a wedged loop stops
/// settling, so the **age of the oldest pending job** — `anakmobil queue-stats` — climbs
/// without bound. That is the same signal a heartbeat table would have carried, read
/// from a column that already exists.
async fn run_worker(config: &Config) -> anyhow::Result<()> {
    let pool = adapter::postgres::connect(config.database_url.expose())?;

    usecase::jobs::run(&pool, dispatch, shutdown::signal_received()).await;

    tracing::info!("shutdown signal received, in-flight job finished");
    pool.close().await;
    Ok(())
}

/// Every job kind this build knows how to run.
///
/// None yet. `media.process` arrives with AM-359, and until it does an unknown kind is a
/// **permanent** failure rather than a transient one: a build that does not know a kind
/// will not learn it by waiting, and eight attempts to discover that would delay every
/// other job for twenty minutes to reach the same dead-letter.
///
/// The job's kind is logged; its payload never is. A payload carries a media id today
/// and something private tomorrow.
async fn dispatch(job: usecase::jobs::Job) -> usecase::jobs::JobOutcome {
    Err(usecase::jobs::JobFailure::Permanent(format!(
        "unknown job kind `{}`",
        job.kind
    )))
}
```

- [ ] **Step 5: Run the tests**

```bash
make db-up && make be-test
```

Expected: `test result: ok` for `job_queue`, now nine tests. If
`the_loop_drains_what_it_is_given_and_stops_when_told` reports two handled instead of
three, the shutdown check is firing before the third job settles — move it back to the
top of the iteration.

- [ ] **Step 6: Run the worker by hand and read what it says**

```bash
make be-worker
```

Expected on stdout: a `starting` line with `role=worker`, then
`worker loop started` carrying a `worker` UUID. Nothing else while the queue is empty —
**no** log line per poll. Then, in another terminal, queue something it cannot handle:

```bash
docker compose exec -T postgres psql -U postgres -d anakmobil -c \
  "INSERT INTO jobs (id, kind, payload) VALUES (gen_random_uuid(), 'nope', '{}'::jsonb)"
```

Expected within a second: one `WARN … job dead-lettered` line naming the job id, and no
retry. Then `SELECT state, attempts, last_error FROM jobs WHERE kind = 'nope';` shows
`dead | 1 | unknown job kind \`nope\``. Attempts of 8 there means the unknown kind is
being classified as transient — that is the bug this step exists to catch. Clean up:

```bash
docker compose exec -T postgres psql -U postgres -d anakmobil -c \
  "DELETE FROM jobs WHERE kind = 'nope'"
```

Finally `Ctrl-C` the worker. Expected: `worker loop stopped`, then
`shutdown signal received, in-flight job finished`, then `stopped role=worker`, and the
process exits rather than hanging.

- [ ] **Step 7: Full gate**

```bash
make be-check && make be-sqlx-check
```

Expected: `backend gate green`, then `query data up-to-date`. No `.sqlx` change is
expected from this task; if `be-sqlx-check` fails, a query was edited and
`make be-prepare` was skipped.

**Acceptance criteria:**
1. The shutdown signal is checked at the top of the loop and in the idle branch, and
   **never** in a `select!` racing the handler. A `select!` whose arms include
   `handle(job)` is a rejection.
2. `run` returns `()`; a database failure logs and retries rather than exiting.
3. `dispatch` classifies an unknown kind as `Permanent`.
4. `run_worker`'s doc comment no longer promises a future heartbeat, and says what
   replaced it.
5. `the_loop_drains_what_it_is_given_and_stops_when_told` passes with all three jobs
   `done`.
6. The manual run in Step 6 dead-letters at attempt 1, and `Ctrl-C` exits cleanly.
7. No payload appears in any log line.
8. Any `#[allow(dead_code)]` from Task 2 is gone.

---

## Task 6: `queue-stats` — the two numbers an operator reads

**Files:**
- Modify: `apps/api/crates/runtime/src/adapter/postgres/job_repo.rs` — `QueueStats`, `stats`
- Modify: `apps/api/crates/runtime/src/lib.rs` — the command, the usage line, its tests
- Modify: `apps/api/crates/runtime/tests/job_queue.rs` — one test
- Modify: `apps/api/CLAUDE.md` — a "The job queue" section
- Modify: `apps/api/.sqlx/` — regenerated

**Interfaces:**
- Consumes: the schema (Task 1), the settle functions (Task 4).
- Produces:
  ```rust
  pub struct QueueStats {
      pub oldest_pending_age_seconds: Option<f64>,
      pub dead: i64,
  }
  pub async fn stats(conn: &mut PgConnection) -> Result<QueueStats, sqlx::Error>;
  ```
  and the command `anakmobil queue-stats`.

**Serves:** AC5.

**TDD: yes** — the query has an exact answer, and the interesting part of it is the part
easiest to get wrong: whether a *leased* job counts as pending.

**"Oldest queued" is read as "oldest not yet finished", and that is a deliberate
departure from the spec's wording.** Spec §10 says "the age of the oldest queued job". Read
literally as `state = 'queued'`, the metric goes to **zero** in exactly the situation an
operator needs it — a wedged worker holding every job in `leased` forever would report a
healthy queue. So the number is `min(created_at)` over `state IN ('queued','leased')`:
the age of the oldest job the platform still owes work on. This is also what makes the
metric the heartbeat that `lib.rs` promised. Recorded here rather than changed silently.

**A CLI command, not an HTTP endpoint.** "An operator" here means somebody with shell
access, which this repository already treats as a higher authority than any admin session
— `grant-admin` says so in its own doc comment. A route would need an `Admin` extractor,
a DTO, a rate-limit decision, and a place in the router, to publish two integers to a
backoffice that is not scaffolded. Matched by hand next to `grant-admin`, because it is
not a process role: `Role` models what the process *is*, and `queue-stats` prints and
exits.

**Minimality pass:** Two numbers, two lines of output, no JSON, no `--watch`, no
per-kind breakdown, no oldest-dead. One query, two scalar subselects.

**Big O:**
- `count(*) WHERE state = 'dead'` is an index-only scan of `jobs_dead_idx` — **O(d)** in
  dead rows, not O(n) over every job ever run. That index is the reason this command
  cannot become a sequential scan of a large table at the exact moment somebody is
  debugging.
- `min(created_at) WHERE state IN ('queued','leased')` is **O(p)** in pending rows.
  `jobs_claimable_idx` is ordered by `COALESCE(leased_until, run_at)`, not by
  `created_at`, so Postgres matches on the predicate and scans. That is acceptable
  because a human runs this on demand and `p` is small whenever the queue is healthy —
  the case where `p` is large is the case where you are already reading the number. The
  trigger for adding `CREATE INDEX … ON jobs (created_at) WHERE state IN
  ('queued','leased')`: this ever runs on a timer rather than by hand.
- Space O(1).

**Rust quality gate:**

```
# Rust quality gate — write compliant from the first commit (NO Sonar; clippy is the gate)

- clippy::too_many_arguments — ≤7 params (aim ≤5); past that, a params struct / builder.
- clippy::cognitive_complexity — extract named helpers; early-return `?`; flatten with `let ... else`.
- NO `.unwrap()` / `.expect()` / `panic!` / `todo!()` on production paths — deny clippy::unwrap_used +
  clippy::expect_used on non-test code; return `Result` + `?` / `ok_or_else` / `unwrap_or_default`.
  (Tests MAY unwrap/expect freely — the deny is #[cfg(test)]-exempt.)
- Duplicated string literal ≥3× → a module-level `const`.
- `#![forbid(unsafe_code)]` (domain + crate-wide unless a justified, `// SAFETY:`-documented exception).
- Errors: thiserror enums in domain/library; anyhow only at the app/bin boundary; `#[from]` + `.context()`;
  never `let _ = fallible();` (swallowed Result) — handle or `?`. Map domain→HTTP at ONE `IntoResponse` choke point.
- Async: never hold a std::sync::Mutex guard across `.await` (use tokio::sync::Mutex); never block the runtime
  (no std::thread::sleep / sync I/O / heavy CPU in async — use tokio::task::spawn_blocking or rayon); never drop a
  JoinHandle whose error matters; bound fan-out with tokio::sync::Semaphore(n).
- sqlx: parameterized (`$1`) or query!/query_as! macros — never `format!` into SQL.
- Verify before "done": make be-fmt → make be-lint → make be-test → make be-boundary → make be-prepare (when a
  query changed) → make be-sqlx-check.

When fixing one instance of a rule, scan sibling files for the same shape and fix-forward.
When reviewing, check the diff against this list BEFORE marking it compliant.
```

- [ ] **Step 1: Write the failing test**

Append to `apps/api/crates/runtime/tests/job_queue.rs`:

```rust
#[tokio::test]
async fn a_leased_job_still_counts_as_pending_and_a_dead_one_is_counted() {
    // AC5, and the one thing about it that is easy to get wrong. Read as
    // `state = 'queued'` alone, the age would fall to zero exactly when a wedged worker
    // is holding every job leased -- the situation the number exists to reveal. So the
    // job this test leaves pending is LEASED, and nothing is queued at all.
    let _serial = QUEUE.lock().await;
    let pool = pool!();
    let worker = Uuid::now_v7();
    let mut conn = pool.acquire().await.expect("a connection");

    let pending = sqlx::query_scalar!(
        r#"SELECT count(*) AS "pending!" FROM jobs WHERE state IN ('queued', 'leased')"#
    )
    .fetch_one(&mut *conn)
    .await
    .expect("counting");
    assert_eq!(
        pending, 0,
        "an interrupted run left work in the queue; clear it with \
         `DELETE FROM jobs WHERE kind LIKE 'test.%'` and run again"
    );

    let before = job_repo::stats(&mut conn).await.expect("reading the stats");
    assert_eq!(before.oldest_pending_age_seconds, None, "nothing is pending yet");
    drop(conn);

    // One job, claimed and left leased. Nothing is in `queued`.
    let leased = a_queued_job(&pool, "test.stats").await;
    claim_this(&pool, worker, leased).await;

    // A second job, dead-lettered.
    let doomed = a_queued_job(&pool, "test.stats").await;
    claim_this(&pool, worker, doomed).await;
    let mut conn = pool.acquire().await.expect("a connection");
    job_repo::mark_dead(&mut conn, doomed, worker, "poison")
        .await
        .expect("dead-lettering");

    let after = job_repo::stats(&mut conn).await.expect("reading the stats");
    drop(conn);

    assert!(
        after.oldest_pending_age_seconds.is_some(),
        "a leased job is still owed work; reading `pending` as `queued` only would \
         report a healthy queue while a wedged worker holds everything"
    );
    assert_eq!(
        after.dead,
        before.dead + 1,
        "a dead-lettered job must show up in the dead count"
    );

    forget(&pool, leased).await;
    forget(&pool, doomed).await;
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
make db-up && make be-test
```

Expected: FAIL to compile — `no function named 'stats'`.

- [ ] **Step 3: Write the query**

Append to `apps/api/crates/runtime/src/adapter/postgres/job_repo.rs`:

```rust
/// AC5's two numbers.
#[derive(Debug, Clone)]
pub struct QueueStats {
    /// How long the oldest job the platform still owes work on has been waiting.
    ///
    /// `None` when nothing is pending. Counts **leased** jobs as well as queued ones:
    /// read as queued-only, this falls to zero precisely when a wedged worker is holding
    /// everything, which is the case it exists to reveal.
    pub oldest_pending_age_seconds: Option<f64>,
    /// How many jobs gave up.
    pub dead: i64,
}

/// Read the queue's health.
///
/// Two scalar subselects in one round trip. No metrics service, no exporter, no second
/// store — the answer is already in this table.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn stats(conn: &mut PgConnection) -> Result<QueueStats, sqlx::Error> {
    sqlx::query_as!(
        QueueStats,
        r#"
        SELECT
            (SELECT extract(epoch FROM now() - min(created_at))::double precision
               FROM jobs WHERE state IN ('queued', 'leased'))
                AS "oldest_pending_age_seconds?",
            (SELECT count(*) FROM jobs WHERE state = 'dead') AS "dead!"
        "#
    )
    .fetch_one(conn)
    .await
}
```

- [ ] **Step 4: Add the command**

In `apps/api/crates/runtime/src/lib.rs`, beside `GRANT_ADMIN`:

```rust
/// The other command that is not a process role.
///
/// It prints two numbers and exits, so it is not a description of what the process *is*
/// — `Role` models that. Matched by hand for the same reason `grant-admin` is.
///
/// Shell access rather than an admin session, deliberately, and the same argument
/// `grant-admin` makes: this is an operator's question about the platform, not a
/// person's question about their own data, so it does not want an HTTP surface, a DTO,
/// or a rate limit.
const QUEUE_STATS: &str = "queue-stats";
```

Extend the usage constant:

```rust
    const USAGE: &'static str = "usage: anakmobil <web|worker|migrate>\n       \
                                 anakmobil grant-admin <email>\n       \
                                 anakmobil queue-stats";
```

In `run()`, immediately after the `GRANT_ADMIN` block:

```rust
    if command.as_deref() == Some(QUEUE_STATS) {
        let config = Config::from_env()?;
        logging::init(config.app_env, &config.log_level)?;
        return run_queue_stats(&config).await;
    }
```

And the function, beside `run_grant_admin`:

```rust
/// Print the queue's two numbers and exit.
///
/// The age of the oldest job still owed work, and how many gave up. `println!` rather
/// than a log line: this is output a person asked for, not an event.
async fn run_queue_stats(config: &Config) -> anyhow::Result<()> {
    let pool = adapter::postgres::connect(config.database_url.expose())?;
    let mut conn = pool.acquire().await?;
    let stats = adapter::postgres::job_repo::stats(&mut conn).await?;
    drop(conn);

    match stats.oldest_pending_age_seconds {
        Some(age) => println!("oldest pending job: {age:.0}s"),
        None => println!("oldest pending job: none"),
    }
    println!("dead jobs: {}", stats.dead);

    pool.close().await;
    Ok(())
}
```

- [ ] **Step 5: Extend the two usage tests**

In `lib.rs`'s `mod tests`, add one test and extend one:

```rust
    #[test]
    fn queue_stats_is_not_a_process_role() {
        // Same prohibition as `grant-admin`. `Role` is what the process IS; this prints
        // and exits.
        let err = Role::parse(Some(QUEUE_STATS)).unwrap_err();
        assert!(err.contains("unknown role"));
    }
```

and in `the_usage_line_names_every_way_to_start_this_binary`, add:

```rust
        assert!(err.contains(QUEUE_STATS));
```

- [ ] **Step 6: Regenerate the cache and run everything**

```bash
make be-prepare && make be-test
```

Expected: `test result: ok` for `job_queue` (ten tests) and for the lib tests, including
`queue_stats_is_not_a_process_role`.

- [ ] **Step 7: Run the command by hand**

```bash
cd apps/api && cargo run --bin anakmobil -- queue-stats
```

Expected on an empty queue:

```
oldest pending job: none
dead jobs: 0
```

Then queue something and read it again:

```bash
docker compose exec -T postgres psql -U postgres -d anakmobil -c \
  "INSERT INTO jobs (id, kind, payload) VALUES (gen_random_uuid(), 'nope', '{}'::jsonb)"
cd apps/api && cargo run --bin anakmobil -- queue-stats
```

Expected: `oldest pending job: 0s` and the same dead count. Clean up with
`DELETE FROM jobs WHERE kind = 'nope'`.

Also check the usage line:

```bash
cd apps/api && cargo run --bin anakmobil -- nonsense
```

Expected: the error names `nonsense` and lists all three lines, `queue-stats` among them.

- [ ] **Step 8: Write the queue into `apps/api/CLAUDE.md`**

Insert a section between "## sqlx" and "## Authentication" — the file a fresh agent
reads, so the contract does not live only in a doc comment:

```markdown
## The job queue

One `jobs` table, claimed with `SELECT … FOR UPDATE SKIP LOCKED`. `web` enqueues,
`worker` claims. Redis is not involved and will not be — the failures that matter need
leases, retries, and dead-lettering whatever the broker is.

**It is at-least-once, and that is a contract rather than a limitation.** A worker can
die between finishing its work and recording that it finished. What the queue guarantees
is that a job is never lost: the lease expires and another worker takes it.

- **A side effect inside this database** is made idempotent by the transaction that
  writes it. `enqueue` takes a `&mut PgConnection` precisely so a caller can flip its own
  row and queue the job together.
- **A side effect outside it** — an object in storage, a push handed to a provider — is
  the consumer's to dedupe. `jobs.effect_key` gives it a stable name to dedupe on, and a
  partial unique index makes enqueuing the same *live* key a no-op. Terminal dedupe is
  the consumer's own, recorded in the same transaction as the effect.
- **A validation failure never retries.** `JobFailure::Permanent` dead-letters on the
  first attempt; only `Transient` backs off. A malformed input fails identically every
  time, and eight attempts to discover that delay every other job.
- **`LEASE` must exceed the longest a job can take.** It is 300 seconds. A job that runs
  longer has its own lease expire under it and gets a second worker — the log line for
  that is `lost the lease before settling`, and it means the number is wrong, not that
  the worker is.
- **`attempts` increments on the claim, not on the failure**, so a worker killed mid-job
  still burns an attempt and a crash-looping payload still reaches the cap.

`anakmobil queue-stats` prints the age of the oldest job still owed work and how many
gave up. There is no metrics service and no probe port: a listener beside a job loop
answers `200` while the loop is deadlocked, whereas the oldest-pending age climbs.
```

- [ ] **Step 9: Full gate**

```bash
make be-check && make be-sqlx-check
```

Expected: `backend gate green`, then `query data up-to-date`.

**Acceptance criteria:**
1. `oldest_pending_age_seconds` is computed over `state IN ('queued','leased')`, not
   `state = 'queued'`. Queued-only is a rejection and the test names why.
2. It is `Option<f64>` and prints `none` on an empty queue rather than `0s`.
3. `queue-stats` is matched by hand before `Role::parse`, and `Role::parse("queue-stats")`
   still errors.
4. The usage line names all three ways to start the binary, and the existing test
   asserting that passes with the addition.
5. `a_leased_job_still_counts_as_pending_and_a_dead_one_is_counted` passes.
6. `apps/api/CLAUDE.md` carries the at-least-once contract.
7. `.sqlx/` is in the diff.

---

## Execution mode

**1. What runs in parallel, and what is serialised on what.**

Tasks **1 and 2 run concurrently.** Task 1 touches only two new migration files; Task 2
touches only `usecase/jobs.rs` and one line of `usecase/mod.rs`. No shared file, no
shared interface — Task 2 is pure arithmetic that does not know a database exists.

Everything after that is **serial**, and each link names its shared file:

| Pair | Serialised on |
|---|---|
| 1 → 3 | the schema itself: Task 3's queries do not compile until the table exists |
| 2 → 4 | `settle_for` and `backoff`, which Task 4's tests exercise through the settle functions |
| 3 → 4 | `adapter/postgres/job_repo.rs`, and `.sqlx/`, which both regenerate |
| 3, 4 → 5 | `usecase/jobs.rs` (Task 2 and 5 both write it) and every repository function the loop calls |
| 5 → 6 | `lib.rs`, which both edit, and `job_repo.rs` again |

That is serial reached by analysis, not by habit: the chain is real, because five of the
six tasks write one of two files. The controller should **not** try to widen it — a
second writer in `job_repo.rs` produces a merge, not parallelism.

**2. The environment card — paste verbatim into every task brief.**

```
ENVIRONMENT — facts a fresh context cannot discover and will get wrong

1. SQLX_OFFLINE=true. The sqlx macros compile against the committed .sqlx/ directory and
   ignore any live database. EVERY new or changed query needs `make be-prepare`, and
   .sqlx/ is part of that task's diff. CI runs `make be-sqlx-check` SEPARATELY and it
   fails on a stale cache. `be-prepare` uses a throwaway EMPTY database on purpose:
   nullability is inferred from the query plan, and a seeded database makes sqlx decide
   NOT NULL columns are nullable.
2. `make be-check` = be-fmt + be-lint + be-test + be-boundary. It does NOT include
   be-sqlx-check. Run both. be-lint is clippy with -D warnings.
3. THIS REPOSITORY DOES NOT RUN SONAR. Clippy is the gate. The Rust quality-gate block in
   the task brief is the list; there is no Sonar block anywhere in this plan.
4. The integration tests report PASSING when DATABASE_URL is absent — cargo captures
   stderr for passing tests. Run `make db-up` first and `make be-test` (which loads .env),
   never a bare `cargo test`. A suite finishing in 0.00s executed nothing.
5. Migrations live in crates/runtime/migrations/ because sqlx::migrate! resolves relative
   to CARGO_MANIFEST_DIR. Always `sqlx migrate add -r`. sqlx takes an advisory lock before
   applying — do not re-implement that. While this branch is unmerged, unpushed, and
   nothing else is running against your dev database, a migration may be amended in place
   followed by `make db-drop`; otherwise never.
6. crates/domain has no axum/sqlx/reqwest/redis/tracing/tokio in its Cargo.toml. `use
   sqlx::…` there is error[E0432], not a lint. `make be-boundary` asserts it. NOTHING in
   this plan touches that crate.
7. Repositories take `&mut PgConnection`, never a PgPool. The use case owns the
   transaction. This is not style: spec §4 needs a media state flip and a job insert in
   ONE transaction.
8. sqlx 0.9's `json` feature is NOT currently enabled and there is no jsonb column in the
   schema yet, so serde_json::Value does not implement Type<Postgres> until Task 3 turns
   it on. The error is "the trait bound `Value: Type<Postgres>` is not satisfied".
9. Native Postgres enums map to Rust with #[derive(sqlx::Type)] +
   #[sqlx(type_name = "…", rename_all = "snake_case")] — see build_repo::Visibility. This
   plan needs NO Rust mirror of job_state; states are only ever written as SQL literals
   and read as ::text in tests.
10. Timestamps are computed by Postgres: `now() + ($n::double precision * interval '1
    second')`. Never build one in Rust and pass it in — several replicas, several clocks.
11. `updated_at` is maintained by the set_updated_at() trigger. Never set it in an UPDATE.
12. DO NOT COMMIT and do not push. Per-task commits are forbidden; work accumulates in the
    working tree for the owner's review.
13. The worker stub at lib.rs:218 logs exactly "worker queue not wired yet — AM-358
    (queue), AM-359 (media)". Task 5 replaces that whole function, doc comment included.
14. Config keys are exactly app_env, bind_addr, log_level, database_url, redis_url. This
    plan adds NONE. Every number is a constant; see the plan's Global Constraints table.
15. tokio is built without the io-std feature — there is no async stdin. tokio HAS the
    `time` and `macros` features, so sleep and select! are available.
16. ADDED 2026-08-22 during execution. NEVER run a bare `cargo run --bin anakmobil --
    worker` (or any bare cargo invocation) from a shell that has not exported
    SQLX_OFFLINE=true. Without it the macros fall back to the LIVE database, and its
    query-plan nullability inference disagrees with the committed cache on queries this
    task never touched — the first symptom is `the trait bound Uuid: From<Option<Uuid>>
    is not satisfied` in vehicle_repo.rs, which looks like a defect in unrelated code and
    is not one. `make be-worker` is correct because the Makefile supplies `-include .env`,
    `export`, and `SQLX_OFFLINE ?= true`. Use the make targets.
```

**3. Where the risk concentrates.**

**Task 1**, by a wide margin. It is the only task whose mistakes are structural — a
column, a constraint, or an index that five later tasks are built on. Three specific
things, any of which is a rewrite if found late:

- `jobs_claimable_idx` being non-partial or indexing `run_at` instead of the `COALESCE`
  expression. Everything after it would still work and would quietly degrade with total
  history rather than with pending work.
- `jobs_lease_matches_state` being a one-way implication instead of an equivalence. The
  claim predicate's meaning depends on it.
- `jobs_one_live_per_effect` scoped to every row instead of the live states. That makes
  a re-upload permanently un-processable, and the symptom would appear in AM-359, not
  here.

Per the execution loop's carve-out, a review finding against any of those is fixed
**immediately**, not deferred to the fix pass.

**Task 3** is second: the claim statement is the one piece of SQL in this plan whose
correctness is not obvious by reading it, and the property it buys (`SKIP LOCKED` making
workers concurrent rather than merely correct) is invisible in every single-threaded test.

**4. What this plan knows it is missing.**

- **No job kind exists.** Nothing exercises a real payload end to end until AM-359. The
  loop is proven against a handler a test supplied, which is honest but is not the same
  as proven against work.
- **True simultaneity is asserted obliquely.** `four_workers_claim_four_different_jobs`
  spawns four tasks and asserts four distinct claims; it cannot force them into the same
  microsecond. A regression that removed `SKIP LOCKED` would most likely still pass it,
  showing up instead as a claim that blocks. Stated rather than papered over.
- **The queue tests are serialised by a file-local mutex, which is a fence and not a
  transaction.** No other test binary in this workspace touches `jobs`, so within one
  `cargo test` run it is airtight; a row left behind by an interrupted run is not.
  `a_leased_job_still_counts_as_pending_and_a_dead_one_is_counted` fails loudly and
  tells you how to clear it, rather than reporting a wrong number.
- **`make be-test` is single-run and the queue tests are the slowest part of it.**
  Serialising ten tests that each do several round trips is measurably slower than
  running them in parallel; the alternative — a database per test via `#[sqlx::test]` —
  was rejected because it panics when `DATABASE_URL` is absent and would break the
  documented `AM_SKIP_INTEGRATION=1 cargo test` path.
- **No `done`-row retention.** Spec §3 fixes the four states, so `done` rows are kept and
  accumulate forever. The partial indexes mean this never touches the claim or the stats,
  so it is a storage question rather than a correctness one — see the findings below.
- **`LEASE` is unverified against real work.** Five minutes is chosen; whether AM-359's
  decode bound actually fits under it is that ticket's to confirm.

---

## Execution status

- [x] **Task 1** — The `jobs` table. Migration `20260822115146_jobs.{up,down}.sql`,
      written by a dispatched `sonnet`. All three structural risks verified against the
      LIVE schema rather than the DDL text: `jobs_claimable_idx` is partial on
      `COALESCE(leased_until, run_at)` over the live states and `EXPLAIN` confirms the
      claim query uses it rather than a sequential scan; `jobs_lease_matches_state` is
      the `=` equivalence form; `jobs_one_live_per_effect` is unique over the live states
      only, so a terminal job's effect key is reusable. `job_state` reads
      `queued(1), leased(2), done(3), dead(4)` from `pg_enum`. Reverse verified: `\d jobs`
      finds no relation, `\dT job_state` returns 0 rows. Nothing in the plan was wrong.
- [x] **Task 2** — Retry policy, pure functions. `usecase/jobs.rs` (new) plus one line in
      `usecase/mod.rs`, written by a dispatched `sonnet` under TDD. Red first: 7 of 10
      failed against stubbed bodies, the first being `the_delay_doubles_from_the_base`
      (`left: 0ns, right: 10s`) — red for the intended reason. **Three tests passed against
      the stub**, which the implementer disclosed; that is exactly the cannot-fail shape,
      and the reviewer was pointed at it by name. All O(1) time and space; the backoff
      shift clamps to `[0, 20]` so `i32::MAX` attempts does constant work.
      *Plan correction found:* Step 5 says `#[allow(dead_code)]` on "exactly those four
      items"; clippy actually reports **seven** before any allow is added, because three
      private consts are reachable only from the dead functions. It resolves once the four
      allows are in — an allowed item is a live root — but a reader checking clippy partway
      through would see three extra errors and doubt the plan.
- [x] **Task 3** — Enqueue and claim. `adapter/postgres/job_repo.rs` and
      `tests/job_queue.rs` (both new), sqlx's `json` feature enabled in `Cargo.toml`, a
      module line in `adapter/postgres/mod.rs`, and six new `.sqlx/` cache entries.
      Written by a dispatched `sonnet`, **which ended its turn mid-task** — it left a
      `be-test` run in the background and never returned to read its exit code, so the
      task's state had to be reconstructed from the working tree. The code was complete;
      the gates had never been run.
      **Gate failed on the controller's first run** — `BE_CHECK_EXIT=2`, zero tests
      executed: `error: `panic` should not be present in production code` at
      `tests/job_queue.rs:121` and `:130`, both inside the helper `claim_this`.
      **Written inline by the controller**, per the four inline tests (nothing else was
      ready, one file, `TDD: no` for a lint fix, and it touches nothing on the floor
      list). The cause is a real asymmetry worth recording: `apps/api/clippy.toml` sets
      `allow-panic-in-tests = true`, but the three lints disagree about what "in a test"
      means — an integration-test CRATE satisfies `expect_used`, while `clippy::panic`
      requires being inside a `#[test]` FUNCTION. So a `panic!` in a plain helper is
      denied while the `.expect()` calls beside it are not, and `panic!` in the other
      test files passes only because it sits inside a macro that expands into a `#[test]`.
      `#[expect(clippy::panic)]` was rejected: `apps/api/CLAUDE.md` says "Never lower a
      threshold, blanket-`#[allow]` a lint, or delete a failing test to go green. Fix the
      cause." The loop now `break`s into an `Option` and one `.expect()` reports both
      failure modes. Sent to Task 3's reviewer flagged as controller-written.
      **Step 1b's open question is resolved: `&serde_json::Value` needs NO
      `sqlx::types::Json` wrapper** — recorded here because Step 1b asked for it.
      **Gates after the fix: `be-check` exit 0, `be-sqlx-check` exit 0, 403 tests passed,
      0 failed.** `tests/job_queue.rs` ran 5 tests in 0.65s — not 0.00s, so it executed.
- [x] **Task 4** — Settling a claimed job. `mark_done`, `reschedule`, `mark_dead` in
      `job_repo.rs`, three tests plus a `state_of` helper in `tests/job_queue.rs`, six new
      `.sqlx/` entries. Written by a dispatched `sonnet`, which ran its gates to completion
      and reported them properly. Red first for the intended reason: `error[E0425]: cannot
      find function 'reschedule' in module 'job_repo'`.
      *Plan gap it found and closed itself:* the Step-3 code block predates today's
      Global-Constraints correction, so it carries no "never a payload, never a credential"
      doc language — and Task 4 is the first code that ever writes `jobs.last_error`. It
      added that contract to both doc comments. **This is intentional, not drift**, and the
      plan's Step 3 block should be updated to match.

## Incident — two writers, one database, and the controller caused it

Recorded here rather than in the ledger because it is a failure of how this run was
sequenced, not a defect in anyone's diff.

Task 3's implementer reported "waiting for `be-test`" and its completion notification
fired. **The controller read that as finished and dispatched Task 4 into the same working
tree.** It was not finished — it kept running for another half hour, and for part of that
window two separate `cargo test` processes were claiming rows from the same live `jobs`
table. `tests/job_queue.rs` serialises itself with a **process-local** `QUEUE: Mutex<()>`,
which cannot see across two `cargo test` invocations, so both runs interfered and
`job_queue` showed two failures — both `"the job never came up: the queue emptied, or 16
claims went by without it"`, the signature of a claimer eating rows a different process
left behind. The plan's own "what this plan knows it is missing" section predicted exactly
this ("a fence and not a transaction… a row left behind by an interrupted run is not").

**Diagnosis, verified rather than assumed:** the `jobs` table held 5 orphan rows
(4 `queued`, 1 `leased`) and one stray `make be-check` was still running. Both were
cleared, and the gates re-run alone.

**Two things the concurrency destroyed that are worth knowing about.** Task 3's brief told
its implementer to write gate output to `/tmp/t3-check.log` — the same path the controller
had used — so **the evidence file for the original clippy failure was overwritten** by that
agent's own later, passing run. The failure was real and its exact text was quoted into the
Task 3 entry above when it happened (`tests/job_queue.rs:121:13` and `:130:5`), but the log
no longer shows it. And that same overwrite explains the implementer's report that it
"could not reproduce the clippy failure": it ran clippy against code the controller had
already fixed.

**What changes for the rest of this run:** a task is finished when its gates have been run
to completion and their exit codes read — not when a notification arrives. Where an
implementer's own report says it is still waiting, that is the report, not the
notification, that is true.

- [x] **Task 5** — The worker loop and the worker role. `usecase/jobs.rs` gains `run`,
      `claim_one`, `run_one`, `settle`, `IDLE_POLL`; `lib.rs`'s `run_worker` now takes
      `&Config`, connects, runs the loop with `dispatch`, and closes the pool, with its doc
      comment rewritten so it no longer promises a heartbeat that has now arrived.
      **All four `#[allow(dead_code)]` from Task 2 are gone** — `Settlement`, `backoff`,
      `settle_for`, and `trim_error` all have real non-test callers now, which is exactly
      the deletion point the `ponytail:` comment named. No `.sqlx` change, as the plan
      predicted. Red first for the intended reason: `error[E0425]: cannot find function
      'run' in module anakmobil_runtime::usecase::jobs`.
      **The F9 prohibition held, and was verified rather than asserted**: the implementer
      grepped its own diff for `?job`, `{job:?}`, and `%job` — zero hits. Every
      `tracing::*!` call carries scalars only (`job = %id`, `attempts`, `worker`,
      `cause = %err`), and `dispatch`'s message formats `job.kind` alone, never the payload.
      **The worker was run by hand and its real output read**, which is the part a green
      suite cannot answer: `worker loop started` with a uuid, then silence while idle (no
      per-poll line), then on an unhandleable job
      `WARN job dead-lettered job=… attempts=1` with the row reaching `dead | 1 | unknown
      job kind ``nope``` — attempt 1, not 8, so a permanent failure does not retry. On
      SIGINT: `worker loop stopped`, `shutdown signal received, in-flight job finished`,
      `stopped role=worker`, and the process exited.
      *Plan defect it found and worked around:* the plan's own Step 1 test code trips
      **`clippy::collapsible_if`**, and this repository lints with `-D warnings` — so the
      plan's literal code did not pass the plan's own gate. Fixed with a let-chain (stable
      at `rust-version = 1.96`). **Recipe corrected in the plan the same turn.**
      *Its other reported plan defect does not hold*: it said Step 6 should name
      `make be-worker` rather than "run the worker by hand", but Step 6's bash block
      already says `make be-worker`, and the implementer used it. What IS real is the trap
      underneath — a bare `cargo run` without `SQLX_OFFLINE=true` fails with
      `the trait bound Uuid: From<Option<Uuid>> is not satisfied` in `vehicle_repo.rs`,
      which looks like a defect in code nobody touched. **Added to the environment card as
      point 16.**
      *Correctly left alone:* `trim_error` still does not strip control characters. That is
      F5/F14 and it belongs to the fix pass, not to a task whose brief scoped it elsewhere.
      **Controller-verified gates: `be-check` exit 0, `be-sqlx-check` exit 0, 407 tests
      passed, 0 failed. `tests/job_queue.rs` 9/9 in 0.96s, and 0 rows left in the `jobs`
      table afterwards** — the suite cleans up after itself when nothing else is running.
- [x] **Task 6** — `queue-stats`. `QueueStats` + `stats()` in `job_repo.rs` (two scalar
      subselects, one round trip); `QUEUE_STATS` const, a third `Role::USAGE` line, a
      hand-matched `queue-stats` command and `run_queue_stats` in `lib.rs`, plus two unit
      tests (`queue_stats_is_not_a_process_role`, and the usage-line test extended); a
      tenth integration test; and the `## The job queue` section in `apps/api/CLAUDE.md`.
      Red first for the intended reason: `error[E0425]: cannot find function 'stats' in
      module 'job_repo'`.
      **The command was run by hand and the number that matters was made visible.** With a
      single `leased` row backdated 90 s and one `dead` row — and **nothing in `queued`**:
      ```
      oldest pending job: 101s
      dead jobs: 1
      ```
      That is AC5's whole point demonstrated rather than asserted: the pending figure is
      non-zero while `queued` is empty, which is exactly the wedged-worker case the
      corrected metric exists to catch and the literal `state = 'queued'` reading would
      have reported as a healthy queue. The bad-command path prints all three usage lines.
      **Three ledger findings were closed in prose here rather than deferred**: F1b (the
      `<kind-prefix>:<id>` convention and why the index alone does not enforce it), F10
      (an `effect_key` is server-derived, never from a request field, with the
      `media:<someone else's id>` suppression spelled out), and the Task 1 review's note
      that `payload`/`last_error` have no schema-level bound and `done` rows have no
      retention, so anything written there lives forever and travels into every backup.
      The `LEASE`/AM-359 decode-bound relationship is recorded in the same section, since
      the two numbers live in different modules and nothing connects them automatically.
      *Plan accuracy:* no corrections needed — the code matched the repository's real
      conventions, and only `cargo fmt` reflow differed.
      **Observed twice now, by two different implementers, in a file nobody in this plan
      touched:** `build_list_flow.rs::the_cursor_cannot_be_used_to_probe` fails
      intermittently under `cargo test --workspace`'s cross-binary concurrency against the
      shared development database, and passes in isolation. Not this plan's defect and not
      fixed here — recorded because a flake seen once is noise and a flake seen twice by
      independent runs is a report.

---

## Review findings ledger

Severity: `structural` (raise and fix now — a column, a constraint, or a public
signature) · `correctness` · `test-integrity` · `hygiene`.

---

## Fix pass — worked and verified

**All 25 findings closed or consciously deferred. Final gates: `make be-check` exit 0,
`make be-sqlx-check` exit 0, 412 tests passed, 0 failed, `tests/job_queue.rs` 12/12, and
0 rows left in the `jobs` table.**

**Three fixes were required to prove themselves red first**, because all three closed
findings whose whole substance was "this test passes for the wrong reason" — and fixing
that class without watching the new assertion fail just moves the defect up one layer:

| Finding | How it was falsified | The failure |
|---|---|---|
| **F14** | `trim_error` reverted to the unsanitised version | `left: "leased", right: "dead"` — the row never reaches `dead`, because the poisoned `UPDATE` is rejected |
| **F19** | `handle(job)` wrapped in a `select!` arm | panic `"async fn resumed after completion"` — downstream evidence the shutdown future resolved **mid-job** rather than between jobs |
| **F2** | `jobs_claimable_idx` recreated without its partial predicate | `the claim index must be partial over the live states, got: CREATE INDEX … USING btree (COALESCE(leased_until, run_at))` |

**F1 was done last and its precondition held.** The migration was amended in place, `make
db-drop` rebuilt from it, and the constraint was probed in a rolled-back transaction: an
empty-string `effect_key` is now rejected with `violates check constraint
"jobs_effect_key_shape"`, while `NULL` and a normal key still insert. After the F2
red-proof the schema was rebuilt again and re-verified — the partial predicate and both
constraints are live.

**The largest omission in this fix pass was the controller's own.** The consolidated list
below opens with "read this instead of the findings list", which made it authoritative —
and it silently dropped F2, F6, and F8 when the per-task review sections were merged into
it. The fix-pass implementer read the list as instructed, noticed the gap, **declined to
widen its own scope unasked, and reported back**. That is the only reason the three were
worked at all. F6 is the sharpest of them: its *recipe* was corrected in this plan's Task 2
code block and its *instance* in the test file was never touched — "fix the recipe, not
just the instance" run exactly backwards. All three were then written by the controller
inline, and F2 got the red-proof above.

**Deliberately not fixed, and each is a decision rather than an oversight:** F3
(`jobs_dead_idx` keyed on enqueue time rather than death time), F4
(`jobs_lease_matches_state` does not cover `leased_by`, because extending it would forbid
keeping `leased_by` on a `done` row as provenance — probably worth more), and F9 (`Job`'s
derived `Debug` — nothing logs a whole `Job` today, verified by grep three times across
three separate reviews, so the hand-written `Debug` is deferred rather than dropped).
**F18 was open and the owner decided it**: `mark_done` now clears `last_error`, because
`attempts > 1` already records that a job retried, while an uncleared field is an
unbounded-retention credential channel that Root A's sanitisation does not close — a
presigned URL survives control-character stripping intact.

Not this plan's defect and not fixed here: the
`build_list_flow.rs::the_cursor_cannot_be_used_to_probe` flake, reported independently by
two implementers in a file this plan never touched.

---

## Consolidated fix pass — read this instead of the findings list

Twenty-three findings from six reviews, none `structural`. Read one at a time they look
like a list of unrelated defects. Read together they are **four roots**, and the ordering
below is by what each costs if it escapes, not by severity label.

### Root A — `last_error` is a free-text channel into a long-retention column, and its contract lives in a doc comment rather than in code

**F5 · F7 · F14 · F18.** One pass closes all four, and it is first because F14 is the
worst outcome in the ledger.

`trim_error` truncates but does not sanitise. `mark_dead` is the **only** transition into
`dead`, and `settle_for` routes every `Permanent` failure there regardless of `attempts` —
so a message carrying `U+0000` makes `UPDATE … SET last_error = $3` fail, the row stays
`leased`, the lease expires five minutes later, and the job cycles **forever**, consuming
a worker slot with no dead-letter row for `queue-stats` to count. Task 5 built the first
production path that can carry one: `dispatch` interpolates `job.kind`, a `TEXT` column
constrained only by `length(btrim(kind)) > 0`.

1. **F5** — map control characters to spaces BEFORE the take, so truncation still counts
   500 visible characters: `.chars().map(|c| if c.is_control() { ' ' } else { c }).take(ERROR_MAX_CHARS)`.
2. **F14** — an integration test enqueuing a job whose settle message contains `'\0'`,
   asserting `mark_dead` returns `true` and the row reaches `dead`. Without it the fix is
   unpinned exactly the way the bug was.
3. **F7** — `JobFailure::message()`'s doc at `usecase/jobs.rs:63` still says "never a
   payload". It is the contract the AM-359 author actually reads — `job_repo.rs`'s
   paragraph is correct but sits where they will never look. Add "and never a credential",
   and name the mechanism: a `reqwest` error's `Display` includes the URL, so a presigned
   PUT that times out writes its `X-Amz-Signature` there.
4. **F18** — `mark_done` does not clear `last_error`, so a transient failure's text (and
   any credential in it) survives on a `done` row forever, and `done` rows have no
   retention. Decide and write one sentence either way; keeping it is defensible.

### Root B — the tests pin each property's happy path, not the property

**F13 · F15 · F16 · F19 · F20.** Five findings, one pattern: an assertion that passes for
the wrong reason, or a property whose only guard is a step a human ran once.

5. **F19** — the loop test's handler has **zero `.await`**, so it can never be cancelled,
   so a `select!` around `handle(job)` — the realistic defect, and the one AC1 explicitly
   rejects — passes green. `tokio::task::yield_now().await` after the send. Correct the
   comment to name the variant it actually pins.
6. **F20** — AC3 (`dispatch` → `Permanent`) has **no automated pin at all**; only Step 6's
   manual run catches it, and a manual step is not a gate. ~8 lines in `lib.rs`'s existing
   test module, no database.
7. **F13** — the lost-lease guard is pinned for `mark_done` only. Delete
   `AND leased_by = $2` from `reschedule` or `mark_dead` and nothing goes red — and the
   `reschedule` case is worse, because it returns a job to circulation while another
   worker is still running it. Six lines inside the existing test; both queries cached.
8. **F16** — the delay test discards `reschedule`'s return value, so it cannot distinguish
   "hidden because `run_at` is in the future" from "hidden because nothing happened".
   Assert the bool and `state_of == "queued"`.
9. **F15** — `AND state = 'leased'` is unpinned in all three settles; it only happens to be
   redundant today because every settle also nulls `leased_by`, and F4 records an intent to
   break exactly that. Two lines.
10. **F24** — the AC5 test pins the *predicate* but not the *expression*. **The predicate
   itself is genuinely pinned** — the Task 6 reviewer walked the fixture and confirmed that
   reverting to `state = 'queued'` reddens it, which was the most important thing that
   review could have found. But `is_some()` passes for any non-NULL number, so swapping
   `min(created_at)` for `min(run_at)` survives — a plausible edit, since the claim
   predicate next door is built on `COALESCE(leased_until, run_at)` and someone unifying
   the two would reach for it. A queue whose pending jobs are all mid-backoff then has
   `min(run_at) > now()`, so the age is **negative**: the operator reads a healthy queue
   while work piles up, which is the exact failure this whole task exists to close.
   *Fix*: `assert!((0.0..300.0).contains(&age))` — generous, and loud on either mutation.

**CORRECTED AFTER THE FIRST FIX PASS — this list was incomplete, and the omission is the
same class of defect it exists to catch.** F2, F6, and F8 are recorded in the per-task
review sections with fix-pass language attached, and were silently dropped when those
sections were consolidated into this one. Because this section opens with "read this
instead of the findings list", the fix-pass implementer read it as authoritative — which it
was — and all three went unworked. It reported the gap rather than quietly widening its own
scope, which is why they are here at all. **All three belong to this root**, which is what
makes the omission worse: they are more of the same pattern, not a separate concern.

11. **F6** — `truncation_never_splits_a_character` uses `"ø"`, which is **two** bytes, so
   byte 500 lands on an exact character boundary (500 / 2 = 250) and the byte-slicing
   implementation the test exists to forbid would not panic. `"€"` is three bytes and
   500 = 3·166 + 2, so the cut lands mid-character. **The recipe was corrected in this
   plan's own Task 2 code block and the instance in `tests/job_queue.rs` was never
   touched** — "fix the recipe, not just the instance" run exactly backwards.
12. **F2** — nothing pins the three structural properties Task 1 called its highest risk.
   Delete `WHERE state IN ('queued','leased')` from `jobs_claimable_idx`, or weaken
   `jobs_lease_matches_state` to a one-way implication, and the whole suite stays green
   while the queue silently degrades with total history instead of pending work. ~10 lines
   asserting on `pg_get_indexdef` and `pg_get_constraintdef` strings.
13. **F8** — `four_workers_claim_four_different_jobs`'s comment describes a mechanism that
   is backwards. Measured by the Task 3 reviewer: without `SKIP LOCKED` the second claimer
   blocks, then EvalPlanQual excludes the now-leased row and it takes the **next** one — so
   the test passes deterministically and the comment's stated tell ("claimers returning
   None") never happens. Correcting the comment matters as much as the assertion, because
   the next reader trusts it and never writes the real one.

### Root C — `effect_key` has no shape, and its provenance was only ever a convention

10. **F1** — `effect_key` accepts the empty string, which participates in uniqueness, so one
    bad `format!` turns the queue into a black hole: an unrelated job returns `INSERT 0 0`
    and is silently discarded. Add the CHECK `kind` already has. **This one amends the
    migration in place, so it goes LAST in the fix pass** — the four conditions in
    `apps/api/CLAUDE.md` require that nothing else is running against the database, and it
    must not survive to merge, because amend-in-place stops being available then.
    **F1b and F10 are already closed** — Task 6 wrote both into `apps/api/CLAUDE.md`.

### Root D — diagnostics that read as more precise than they are

11. **F23** — `dispatch`'s doc claims the kind is logged; nothing logs it. Add `kind` to
    the two settle warnings and make the sentence true.
12. **F21** — `run_worker`'s `pool.close()` is unbounded while `run_web`'s is wrapped in
    `shutdown::within`; on a database outage the process outlives its grace period and the
    real shutdown becomes `SIGKILL`. (`correctness`, listed here because the fix is one
    line in the same neighbourhood.)
13. **F22** — no backoff on the claim-error path: a worker deployed ahead of the web role
    logs `relation "jobs" does not exist` 86,400 times a day. `IDLE_POLL * 5`.
14. **F17** — `false` means three different things and the contract names one. One sentence.
15. **F11** — the controller's own `claim_this` rewrite dropped `{id}` from its message.
    `assert!` is not covered by `clippy::panic`, so it can be restored without a `panic!`.
16. **F12** — re-read `tests/job_queue.rs`'s module doc once F19 lands.
17. **F25** — the stats cost model and the trigger for a fourth index live **only in this
    plan file**. A plan is a historical artifact; `stats()`'s doc comment is what the author
    of a future exporter reads, and it currently names an exporter without saying what
    changes if one is added. *Fix*: one sentence on `stats` — the cost is O(p + d) and never
    O(n); if this is ever scraped on a timer rather than run by hand, add
    `CREATE INDEX … ON jobs (created_at) WHERE state IN ('queued','leased')`, which drops
    the `min` to O(log p). Measured by the reviewer with real `EXPLAIN`s: 0.22 ms at 500
    pending, 9.1 ms at 100,000 pending against a million `done` rows, and one behaviour the
    plan never mentioned — at ~80% pending the planner abandons the partial index for a
    parallel seq scan, which does not break the bound because p ≈ n by then.

### Recorded, deliberately not fixed

**F3** (`jobs_dead_idx` keyed on `created_at` rather than death time) and **F4**
(`jobs_lease_matches_state` does not cover `leased_by`) — both `hygiene`, neither worth a
migration on its own, and F4's asymmetry is deliberate. **The `build_list_flow.rs::the_cursor_cannot_be_used_to_probe`
flake** — reported by two independent implementers in a file this plan never touched; it is
a report, not this plan's defect.

---


Reviewers for Tasks 1 and 2 are dispatched and have not reported yet. Findings land here
the moment they arrive, and are worked in one pass after the final task — except a finding
against a column, a constraint, or a public contract, which is fixed immediately because
later tasks stand on it.

### Task 1 — reviewed by a dispatched `opus`. No `structural` finding.

All three flagged structural risks verified against the LIVE schema and probed in
rolled-back transactions, not inferred from the DDL: `state='done'` with a lease is
rejected; `state='leased'` without one is rejected; settling to `done` without clearing
the lease is rejected; and **two rows may share an effect key once one is `dead`**, which
is the re-upload path AM-359 depends on. The migration file is byte-identical to the
plan's Step 2 block.

**F1 · `correctness`, constraint-class · `20260822115146_jobs.up.sql:38`**
`effect_key` carries no shape constraint while `kind` does. An empty string is accepted
and participates in uniqueness, so with Task 3's `ON CONFLICT … DO NOTHING` an unrelated
job whose key is `''` returns `INSERT 0 0` and is **silently discarded** — the call site
reads that as "already on its way". Probed and confirmed by the reviewer. One bad
`format!` turns the queue into a black hole with no row, no log, and no counter.
Separately, an incompressible key over ~2704 bytes fails with a raw btree
`index row size … exceeds btree version 4 maximum 2704`, and a compressible 3000-char key
is accepted — so the failure is data-dependent, which is worse than a flat limit.
*Smallest fix*, mirroring what `kind` already has:
```sql
CONSTRAINT jobs_effect_key_shape
    CHECK (effect_key IS NULL OR length(btrim(effect_key)) BETWEEN 1 AND 200)
```
**TIMING — deliberate, not a deferral by default.** This is constraint-class, so the
carve-out applies by the letter of the rule. It is NOT being fixed the instant it was
reported for one reason: the environment card's point 5 permits amending a migration in
place only while "nothing else is running against your dev database", and Task 3 was
running against it when this landed. The carve-out's *purpose* — later tasks stand on the
constraint — does not apply either, because Task 3's `enqueue` is unaffected by it.
**Revised schedule:** deferred to the fix pass rather than fixed the moment Task 3
landed, because amending requires `make db-drop` and the environment card's point 5 needs
"nothing else running against that database" — which stayed false as Task 4 was dispatched
straight after. The fix pass is the first moment nothing else is running. Amend the
migration in place then, and `make db-drop`.
It must not survive to merge, because amend-in-place stops being available then.

**F1b · design note carried with F1.** The unique index is on `effect_key` alone, not
`(kind, effect_key)`, so two different job kinds using the same id space collide and the
second is silently dropped. Making the index composite would be strictly safer but is
NOT free: Task 3's `ON CONFLICT` inference would have to match it, and Task 3 was
mid-flight. Decision: keep the single-column index, and write the convention
`effect_key = <kind-prefix>:<id>` into `apps/api/CLAUDE.md` in Task 6, which already
writes that section. The composite index is the recorded upgrade path if a second kind
ever genuinely wants the same id space.

**F2 · `test-integrity` · the migration as a whole**
The three properties the plan calls its highest risk have **no test that pins them**. The
two tests named as this task's verification read filenames only — they go red if a
`.down.sql` is deleted or a version sorts out of order, and for nothing else. Delete
`WHERE state IN ('queued','leased')` from `jobs_claimable_idx`, or weaken
`jobs_lease_matches_state` to a one-way implication, and **nothing in this repository goes
red** while the queue silently degrades with total history instead of pending work.
*Smallest fix*: roughly ten lines in `tests/job_queue.rs` (Task 3 creates that file
anyway) asserting on `pg_get_indexdef` and `pg_get_constraintdef` strings. Fix pass.

**F3 · `hygiene` · `…up.sql:95-97`** `jobs_dead_idx` is keyed on `created_at` — enqueue
time. The question an operator asks a dead-letter queue is "what died recently", and
death time is `updated_at`. AC5's `count(*)` is still an index-only scan, so AC5 is met;
it is the next question that has no index. Not worth a migration on its own.

**F4 · `hygiene` · `…up.sql:70-71`** `jobs_lease_matches_state` covers `leased_until` but
not `leased_by`, so a released row keeps a stale worker uuid and the database accepts it.
Task 3's release clears it, so the application is right and the schema is merely silent.
Left as-is deliberately: extending the equivalence would also forbid keeping `leased_by`
on a `done` row as provenance, which is probably worth more.

### Task 2 — reviewed by a dispatched `opus`. No `structural` finding.

The file is byte-identical to the plan's code blocks apart from the four
`#[allow(dead_code)]` the plan asked for and a `ponytail:` comment (an established
convention here — `part_repo.rs:267/282/461`, `user_repo.rs:177`, `part_merge.rs:68/222`).
Three things the reviewer verified directly rather than accepting: the overflow clamp
(`i32::MAX` → shift 20 → 900s cap, no panic; `i32::MIN` takes the same branch as `-5`),
the off-by-one at the cap (pinned on **both** sides — `(7,8) → Retry(640s)` and
`(8,8) → Dead` — so it is a boundary, not a value comfortably inside one), and the
`#[allow]` minimality (compiled with the allows stripped: **exactly 7** warnings, so the
implementer's correction is right and the plan's "exactly those four" was wrong; the set
is nonetheless minimal, because rustc seeds an allowed item as a liveness root and the
three consts become reachable through it — do not "fix" this by adding three more).

**The three tests that passed against the stub each have a nameable red-making change**,
so none is an assertion that cannot fail: `>=` → `>` at the cap reddens the dead-letter
test; making the `Permanent` arm retry reddens the permanent test; making `message()`
return a constant reddens the message test. Worth stating plainly, though: the third never
went red during the cycle because Step 3 did not stub `message()`. It is a test-after
assertion on a one-line accessor wearing a TDD cycle's clothes — harmless, but the cycle
proved nothing about it.

**F5 · `correctness` · `usecase/jobs.rs:119-121`**
`trim_error` truncates but does not sanitise, so control characters reach `last_error`.
**A NUL byte kills the very statement that records the failure**: PostgreSQL `text` cannot
hold `U+0000` and rejects the parameter with `invalid byte sequence for encoding "UTF8":
0x00`, so `UPDATE … SET last_error = $n` fails, the row stays `leased` with no error
recorded, and it only returns five minutes later on lease expiry — reading as a mysterious
lease timeout rather than a bad message. The realistic source is AM-359's own territory: a
decoder or `from_utf8_lossy` echoing bytes of a malformed upload. Newlines and `\x1b[`
escapes separately forge log lines and drive an operator's terminal via `queue-stats`.
*Smallest fix*, at the choke point that already exists, mapping BEFORE the take so
truncation still counts 500 visible characters:
```rust
message.chars().map(|c| if c.is_control() { ' ' } else { c }).take(ERROR_MAX_CHARS).collect()
```
**Recipe corrected the same turn** — the plan's Global Constraints were silent on this,
which would have had Tasks 4 and 5 keep assuming the choke point was safe. See the
`last_error` rule above.

**F6 · `test-integrity` · `usecase/jobs.rs:190-198`, from plan line ~692**
`truncation_never_splits_a_character` never triggers the panic it exists to forbid. Its
comment claims "every character here is three bytes"; `ø` is U+00F8 — **two** bytes — so
byte 500 falls on an exact character boundary (500 / 2 = 250) and `&message[..500]` would
have returned 250 characters rather than panicking. The test still reddens against a wrong
implementation via the count assertion, so it was never an assertion that could not fail;
but the named regression went unexercised and the comment said otherwise.
*Smallest fix*: `"€"` (U+20AC, three bytes; 500 = 3·166 + 2). **Recipe corrected the same
turn** at the plan's own copy of this test.

**F7 · `hygiene` · `usecase/jobs.rs:58`**
The doc contract says `last_error` is "never a payload". It is one word short: Tasks 4 and
5 build the message from `format!("{err}")`, and a `reqwest` error's `Display` **includes
the URL** — so an R2 presigned PUT that times out writes its `X-Amz-Signature` there,
readable by anyone with database access or `queue-stats`. Should read "never a payload
**and never a credential**". That comment is the whole contract the AM-359 author reads.
Recipe corrected the same turn.

### Task 3 — reviewed by a dispatched `opus`. No `structural` finding.

`job_repo.rs:1-139` is byte-identical to the plan's Step 4 block; the test file matches
except the controller's `claim_this` rewrite and rustfmt wrapping. The claim's plan and the
`SKIP LOCKED` question were verified **empirically against the live database** with every
probe rolled back (`SELECT count(*) FROM jobs` = 0 afterwards), not inferred from DDL.

**F8 · `test-integrity` · `tests/job_queue.rs:159-165` (comment), `:194-203` (assertions)**
`four_workers_claim_four_different_jobs` **cannot** detect the removal of `SKIP LOCKED`,
and its own comment states the opposite mechanism. The plan guessed it "would most likely
still pass"; the reviewer measured it and it passes **deterministically**. Building the
claim with `FOR UPDATE` and no `SKIP LOCKED`, holding the head row's lock for two seconds:
the second claimer blocked 1.62 s, then Postgres's EvalPlanQual re-evaluated the
subquery's predicate against the now-`leased` tuple, found `COALESCE(leased_until, run_at)
<= now()` false at `now()+300s`, excluded it, and took the next row. A serialising claim
therefore hands out four DIFFERENT jobs — `unique.len()` holds, `claimed.len() == 4`
holds. The comment's stated tell ("claimers returning None") is backwards: being let
through is what makes the claimer move on and succeed. **Production symptom the test
cannot see: throughput of one worker regardless of replica count.**
What the test DOES catch and is worth keeping: deleting `FOR UPDATE` entirely — both
claimers then read the same head id and both lease it, firing the dedupe assertion.
*Smallest fix*, ~10 lines, one extra connection, no timing window: hold the head row's
lock in an explicit transaction, `SET lock_timeout = '250ms'` on a second connection, and
assert the claim returns another job rather than being cancelled. **Correcting the comment
matters as much as adding the test** — a wrong explanation of what a test proves is worse
than none, because the next reader trusts it and never writes the real assertion.

**F9 · `correctness` (privacy) · `adapter/postgres/job_repo.rs:28-39`**
`Job` derives `Debug` and carries `payload`, against this plan's own Global Constraint
"Never log a payload… A payload can carry a media id today and something private
tomorrow." `tracing::error!(?job)` or `format!("{job:?}")` is one keystroke from a full
payload dump — **in Task 5, the task that formats worker failures.** This repository
already solves this structurally rather than by discipline: `platform/config.rs:36`
hand-writes `Debug` for `Secret` to print `Secret(<redacted>)`, and `platform/logging.rs:19`
records why — "so a stray `?config` cannot spill one". `Job` got the derive instead.
*Smallest fix*: a hand-written `Debug` printing `id`, `kind`, `attempts`, `max_attempts`,
`effect_key`, and `payload: <redacted>` — ~10 lines, the shape already in the repo.
`effect_key` is a logical name, not private; leave it visible.
**Not fixed on the spot because Task 4 is editing `job_repo.rs` right now.** Task 5's
brief must carry it as an explicit prohibition, and the fix pass must land it.

**F10 · `hygiene` · `job_repo.rs:49-51`**
The `effect_key` doc contract does not say the key must be **server-derived**. Nothing is
reachable from HTTP yet, so this is not exploitable today — but this comment is what the
AM-359 author reads, and `POST /media/{id}/complete` IS an endpoint. If an `effect_key`
ever came from a request field, a caller sending `media:<someone-else's-id>` would suppress
that media's processing for the life of the live job, with `enqueue` returning `Ok(None)`
and nothing recorded anywhere. *Fix*: one sentence — "derived on the server from ids it
already trusts, never from a request field" — in the same paragraph as F1b's convention.

**F11 · `hygiene` · `tests/job_queue.rs:121-142` — against the CONTROLLER's own code.**
The `claim_this` rewrite dropped the job id from its failure message; the original carried
it in both arms. Behaviour is otherwise unchanged (connection handling and
release-on-the-way-past are identical), and collapsing "queue emptied" vs "16 claims went
by" is noise rather than loss — both mean the fixture never offered the job. But with
several jobs in flight the message no longer says which one. Restoring it needs no
`panic!`, because `assert!` is not covered by `clippy::panic`:
```rust
assert!(found.is_some(), "job {id} never came up: the queue emptied, or 16 claims went by");
found.expect("checked immediately above")
```

**F12 · `hygiene` · `tests/job_queue.rs:11-13`, `:20-22`**
The module doc promises "one of them runs the real worker loop" and describes that test's
handler. It arrives in Task 5. Written from the plan on purpose and self-healing — but
re-read it at the fix pass, because if Task 5's shape changes the paragraph quietly
becomes false.

**Recorded, not a finding.** Step 1b's open question — whether `&serde_json::Value` needed
a `sqlx::types::Json` wrapper — resolved to **no wrapper needed**, and Step 1b asked for
that to be written into `## Execution status`. It is now. Separately: `.sqlx/` was audited
entry by entry (89 cached, 89 accounted for, the six new ones exactly the six new queries,
no orphan from an edited query), and the guard-under-lock question was answered
empirically — the re-check that matters is EvalPlanQual on the subquery's `FOR UPDATE`,
not the outer `WHERE id = $x`, which is not a second guard and does not need to be.

### Task 4 — reviewed by a dispatched `opus`. No `structural` finding.

`job_repo.rs:141-243` is byte-identical to the plan's Step 3 block apart from ASCII `--`
for em dashes and the two declared `last_error` doc paragraphs. All six acceptance criteria
met; the `Tidak boleh ada` block was checked item by item and is clear. The `.sqlx` cache
was enumerated rather than sampled — twelve jobs-related entries, twelve distinct jobs
queries in source, exact one-to-one, no orphans. **The three settle functions are correct
and the implementer's deviation was the right call.** Every defect below is on the test
side, plus one escalation.

**F14 · `correctness` — escalates F5, and F5's own write-up did not name this**
`job_repo.rs:197`, `:233`. Before Task 4, nothing wrote `last_error`, so an unsanitised
control character was inert. Task 4 creates both statements that can be poisoned by one —
and the two failure modes are NOT equivalent:
- `reschedule` fails → the row stays `leased` and returns after 300 s. Slow and confusing;
  this is the symptom F5 predicted, and it self-heals.
- **`mark_dead` fails → the job can never reach `dead` at all.** `mark_dead` is the ONLY
  transition into `dead`, and `settle_for` routes every `Permanent` failure there
  regardless of `attempts`. So: claim (`attempts+1`) → permanent failure → `UPDATE … SET
  last_error = $3` rejected with `invalid byte sequence for encoding "UTF8": 0x00` → row
  stays `leased` → lease expires five minutes later → **repeat forever**. `attempts`
  climbs past `max_attempts` and changes nothing, because the cap's only exit is the
  statement that keeps failing. One poison message becomes a permanently cycling job
  consuming a worker slot every five minutes for the life of the deployment, with no
  dead-letter row for `queue-stats` to count.
The realistic source is AM-359's own territory, as F5 records: a decoder or
`from_utf8_lossy` echoing bytes of a malformed upload.
*Fix* is still F5's one-line map in `trim_error`. **Add to the fix pass: an integration
test** enqueuing a job whose settle message contains `'\0'`, asserting `mark_dead` returns
`true` and the row reaches `dead`. Without it the fix is unpinned exactly the way the bug
was. **This makes F5 the first item in the fix pass, not a hygiene-adjacent one.**

**F13 · `test-integrity` · `tests/job_queue.rs:403-435` against `job_repo.rs:198`, `:234`**
The lost-lease guard is pinned for `mark_done` **only** —
`a_lost_lease_cannot_be_settled_by_the_worker_that_lost_it` calls exactly one settle.
Delete `AND leased_by = $2` from `reschedule` or from `mark_dead` and **nothing in this
repository goes red**. The failure that ships is worse than the case the test does cover:
worker A stalls past its lease, B claims and starts, A's handler finally returns
`Transient`; without the clause A's `reschedule` sets `state='queued'` and nulls
`leased_by` **while B is still running the job**, so the queue hands the same work to a
third worker, and B's own `mark_done` then reports a lost lease it never lost. Task 4's own
AC1 claims this is proven — it is proven for one third of the surface.
**Recipe defect:** the plan's Step 1 block is what specified a single-settle test.
*Smallest fix*: six lines inside the existing test, asserting `reschedule` and `mark_dead`
both return `false` for the stalled worker; both queries are already cached, so no new
`.sqlx` entry. The existing `assert_eq!(state_of(...), "leased")` then proves all three
changed nothing.

**F15 · `test-integrity` · `job_repo.rs:161`, `:198`, `:234`**
`AND state = 'leased'` is unpinned in all three functions — removing it reddens nothing
**today**, because every settle also nulls `leased_by`, so `leased_by = $2` alone happens
to suffice. That is a coincidence, and ledger F4 records an intent to break it (keeping
`leased_by` on a `done` row as provenance). Verified empirically in a rolled-back
transaction: a row with `state='queued'` and a non-null `leased_by` is accepted by the
CHECK, `WHERE … AND state='leased' AND leased_by=$2` gives `UPDATE 0`, and dropping the
state clause gives `UPDATE 1` — the row goes `done`. *Fix*: two lines appended to
`a_dead_lettered_job_is_never_claimed_again`.

**F16 · `test-integrity` · `tests/job_queue.rs:381-398`**
`a_transient_failure_comes_back_and_a_delayed_one_does_not` pins more than "it eventually
returns" and less than it reads. **Caught:** a `reschedule` that ignored `delay` entirely,
and a fixed non-zero offset. **Not caught:** the second `reschedule`'s return value is
discarded, so if that call matched zero rows the job would still be `leased` from
`claim_this`'s 300-second lease and `assert_unclaimable` passes identically — the test
cannot distinguish "hidden because `run_at` is in the future" from "hidden because nothing
happened". Also not caught: a units error (seconds read as milliseconds) turning 60 s into
1 s still hides the job through 16 tight-loop claims. *Fix*: assert the returned bool and
`assert_eq!(state_of(&pool, id).await, "queued")` — queued AND unclaimable proves the delay
is what hides it. Free, no new query.

**F17 · `hygiene` · `job_repo.rs:141-146` and the plan's Task 4 Interfaces block**
`false` collapses three situations while the contract says it means one: another worker
holds the lease, the row is already terminal (a duplicate settle, which at-least-once makes
reachable), or the row is gone. The worker's response is identical in all three, so this is
not a correctness problem — but Task 5's log line will say "lost the lease" for a job that
was deleted or double-settled, and that is the line an operator reads while diagnosing.
*Fix*: one sentence on the shared comment and a matching correction to the plan's
Interfaces wording. **Recipe defect.** Related, for Task 5's reviewer rather than a finding
here: the returned `bool` is trivially ignorable — `mark_done(..).await?;` compiles clean —
so check that Task 5 branches on it.

**F18 · `hygiene` · `job_repo.rs:160`**
`mark_done` does not clear `last_error`, so a job that failed transiently on attempt 3 and
succeeded on attempt 4 keeps that failure text on a `done` row forever — and `done` rows
have no retention. If that message carried a presigned URL, the credential outlives the
success indefinitely. Defensible either way ("succeeded, but here is what went wrong on the
way" is real diagnostic value); it is currently neither documented nor decided. *Fix*: one
sentence on `mark_done`. The reviewer recommends keeping it and saying why.

**The deviation is correct and INSUFFICIENT, which is the sharpest thing in this review.**
The `last_error` paragraph the implementer added to `job_repo.rs` is well written — it
names the mechanism (`reqwest`'s `Display` includes the URL → a presigned PUT that times
out writes its `X-Amz-Signature`) and the consequence, which is what turns "never a
credential" from a slogan into an instruction. **But the AM-359 author never reads
`job_repo.rs`.** They implement a handler and return `JobFailure::Transient(format!("{err}"))`,
and the doc they read is `JobFailure::message()` at `usecase/jobs.rs:63` — which still says
only "never a payload". That is ledger **F7**, and it is the necessary other half of this
deviation. If F7 does not land, the contract sits where nobody who needs it will look.

### Task 5 — reviewed by a dispatched `opus`. No `structural` finding.

The loop's production code is correct and every design call holds. **Two of the eight ACs
are unpinned by any automated test, and one of them is the AC the TDD verdict was granted
for.** All five things the brief asked to be examined hardest were answered by
verification rather than by reading: the `Job`-never-logged prohibition was re-grepped
independently (`\?job|\{job:\?\}|%job|job = \?` → no hits, all six `tracing::*!` calls
enumerated and read); `settle` **does** branch on all three `bool` returns
(`jobs.rs:244-271`), closing F17's code side; and all four `#[allow(dead_code)]` are gone
with no new `#[allow]` anywhere.

**F19 (T5-1) · `test-integrity` · `tests/job_queue.rs:495-515`, comment at `:467-471`**
**The new test cannot detect the defect its own comment names.** The handler future
contains **zero `.await`** — the kind check, the `handled.push`, the `stop.send`, and
`Ok(())` are all synchronous — so it completes on its **first poll** and is never
suspended. A future that never suspends cannot be cancelled by `tokio::select!`. So the
exact change the comment claims would redden it — a `select!` with `handle(job)` in an arm
— **passes green**: whatever the poll order, `shutdown` is Pending on the first pass
(its sender has not fired, because the handler has not run) and the handler is Ready on
the second.

| Variant | Reddens? | Why |
|---|---|---|
| `select!` around **`run_one`** | **yes** | `settle` awaits after the signal fired, so it is cancelled; job 3 stays `leased` |
| `select!` around **`handle(job)`** only | **no** | the handler never suspends — nothing to cancel |
| top-of-loop check deleted | no | `claim_one` → `Ok(None)` → the idle `select!` breaks anyway |
| both checks deleted | not red — **hangs** | the suite hangs rather than failing |

**The unpinned variant is the realistic one.** AM-359's handler (image decode plus an R2
round trip) has many await points, and this plan's own AC1 says *"A `select!` whose arms
include `handle(job)` is a rejection"*. Nothing enforces it.
*Smallest fix*: `tokio::task::yield_now().await` in the test handler, after the send and
before `Ok(())`. The handler then suspends once; under the defect, `shutdown` is Ready at
that suspension, the handler is dropped, job 3 is never settled, and the `state_of`
assertion goes red. Costs nothing in the passing case. Correct the comment to name which
variant it actually pins. Secondary: the test proves **arrival, not ordering** — the
assertions are set membership, so it would pass identically if the claim returned jobs in
reverse order.

**F20 (T5-2) · `test-integrity` · `src/lib.rs:260-265`**
**AC3 — `dispatch` classifies an unknown kind as `Permanent` — has no automated pin.**
Changing `Permanent` to `Transient` reddens nothing in the 407-test suite. The only thing
that catches it is Step 6's manual `make be-worker` run, and the plan says so out loud.
**A manual step is not a gate**: it runs once, by the implementer, and never again. Concrete
failure: a later refactor flips the variant, every unknown kind then retries 8 times over
~21 minutes, and every real job queued behind it waits — discovered when AM-359 deploys
against a worker one release behind. *Smallest fix*: ~8 lines in the `#[cfg(test)] mod
tests` that already exists at `lib.rs:347`, constructing a `Job` and asserting
`matches!(failure, JobFailure::Permanent(_))`. **No database needed.**

**F21 (T5-3) · `correctness` · `src/lib.rs:241-249`**
**The worker's teardown is unbounded while the web role's is not.** `run_web` wraps
`pool.close()` in `shutdown::within(DRAIN_TIMEOUT, …)` at `:208-213` with a comment naming
the hazard; `run_worker:247` calls `pool.close().await` bare. This matters precisely
because the loop is *designed* to survive a database outage (`jobs.rs:148-153`): Postgres
becomes unreachable → the loop logs and keeps polling → the platform sends `SIGTERM` → the
loop breaks cleanly → `pool.close().await` waits on connections it cannot close gracefully
→ the process outlives the container's grace period and the real shutdown mechanism becomes
`SIGKILL`. `shutdown.rs:52-60` documents this exact failure as the reason `within` exists.
*Smallest fix*: mirror `run_web` — `shutdown::within(adapter::http::DRAIN_TIMEOUT, async move { pool.close().await }).await;`

**F22 (T5-4) · `hygiene` · `src/usecase/jobs.rs:184-190`**
**No backoff on the claim-error path — one `ERROR` line per second, forever.** Two regimes
and only one self-limits: with the connection down, `pool.acquire()` burns the 5 s
`acquire_timeout` first (~1 line per 6 s); with the connection fine and the *query* failing,
the error is instant → **86,400 lines/day per worker**. This task creates the first
realistic instance: `run_worker` deliberately does not migrate (`lib.rs:227-230`), so a
worker deployed ahead of the web role logs `relation "jobs" does not exist` once a second
until somebody notices. *Smallest fix*, no new constant: `IDLE_POLL * 5` in the `Err` arm.

**F23 (T5-5) · `hygiene` · `src/lib.rs:258`**
`dispatch`'s doc says *"The job's kind is logged; its payload never is."* The payload half
is true and verified; **the kind half is false** — no log line in this task carries `kind`.
The dead-letter line is `tracing::warn!(job = %id, attempts, "job dead-lettered")`; the
kind reaches `last_error` in the database, not a log. This is the comment the AM-359 author
reads, and it invites them to believe the kind is greppable. `kind` is explicitly not
private (F9's own reasoning keeps logical names visible) and is the one field that makes a
dead-letter line actionable without a `psql` session. *Fix*: add `kind = %kind` to the two
settle warnings and make the sentence true — better than correcting the sentence.

**What the manual Step 6 run does and does not demonstrate.** `dead | 1 | unknown job kind
``nope``` genuinely proves AC3 and the `Permanent` routing — attempt 1 rather than 8 is the
discriminating observation and could not come out that way under a transient
classification. But **no job was in flight when the signal arrived**, so the run says
nothing about whether the loop stops between jobs or abandons one. That is exactly the
property F19 shows the automated test also misses: **the manual check and the test cover
the same easy half.**

**Two open findings are now live rather than latent.** Task 5's `dispatch` builds the first
production message that can carry a control character into `last_error` — a `kind`
containing `U+0000` triggers F14's permanent five-minute cycle exactly as recorded — and
F7's `message()` doc is the contract the AM-359 author will read. Neither is a new finding;
both move up the fix pass.

**Two things the reviewer surfaced that are not findings but belong on the record.**
`payload jsonb` and `last_error` have no shape or size bound at the schema level — the
rule lives in a column comment and in `ERROR_MAX_CHARS`. With no `done`-row retention,
anything landing in `payload` lives forever and travels into backups. Acceptable while the
only planned payload is a media id, and the enforcement point is correctly the consumer
that defines the payload shape — but it should be a sentence in `apps/api/CLAUDE.md`, not
only a comment in a migration. And: the partial index means terminal rows leave **dead
index entries** until autovacuum reclaims them, so the index's physical size tracks queue
*throughput* rather than queue *depth*. The complexity class is unchanged; the migration
comment's "dead rows are not in this index at all" is exactly true for lookup and slightly
optimistic for file size.
