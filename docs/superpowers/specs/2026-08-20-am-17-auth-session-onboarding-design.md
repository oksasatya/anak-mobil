# Authentication, session foundation, app shell, and onboarding

**Tickets:** [AM-17](https://oksasatyaa.atlassian.net/browse/AM-17) · [AM-18](https://oksasatyaa.atlassian.net/browse/AM-18) · [AM-16](https://oksasatyaa.atlassian.net/browse/AM-16) · [AM-50](https://oksasatyaa.atlassian.net/browse/AM-50) (+ [AM-57](https://oksasatyaa.atlassian.net/browse/AM-57), [AM-59](https://oksasatyaa.atlassian.net/browse/AM-59)) · [AM-51](https://oksasatyaa.atlassian.net/browse/AM-51) (+ [AM-60](https://oksasatyaa.atlassian.net/browse/AM-60), [AM-61](https://oksasatyaa.atlassian.net/browse/AM-61)) · [AM-55](https://oksasatyaa.atlassian.net/browse/AM-55) · [AM-113](https://oksasatyaa.atlassian.net/browse/AM-113) · [AM-56](https://oksasatyaa.atlassian.net/browse/AM-56)

**Date:** 2026-08-20 · **Branch:** `feat/AM-17-auth-session-onboarding`

This is one design covering four sequential implementation plans. It exists as
one document because the four share a single contract — the session — and
splitting the contract across four specs is how the halves drift apart.

---

## What this is

A person can install the app, create an account, be carried into adding their
first car, and land somewhere real. Their session survives closing the app,
survives an expired access token, and ends completely when they sign out.

The backend already does most of the authentication work. What is missing is
everything on the phone, a username, and three defects in the session contract
that only became visible while designing against it.

---

## Findings that change the tickets

These were discovered by reading the code and by an adversarial cross-model
pass, not from the tickets. Each one changes what a ticket can honestly claim,
so each is recorded here rather than left for an implementer to trip over.

### 1. The backend already has authentication — the tickets read as if it does not

`POST /auth/register`, `/auth/login`, `/auth/refresh`, `/auth/logout` all ship
today (`apps/api/crates/runtime/src/adapter/http/auth.rs`), with argon2id,
opaque Redis session tokens, refresh rotation with reuse detection, per-IP and
per-account login rate limiting, and a deliberate anti-enumeration discipline
that pays one argon2 verification even for an unknown email
(`usecase/auth.rs::login`). AM-51's AC2 — identical messages for a wrong
password and an unknown account — is already true at the server.

What the phone has is nothing: no API client, no token storage, no auth state,
no route groups. One bare `fetch` to `/healthz`.

### 2. `users` has no `username` column, and AM-50 is named after one

The table is `id · email CITEXT UNIQUE · password_hash · platform_role ·
created_at · updated_at`. AM-50 is "Daftar akun dengan email dan username" and
its AC2 is entirely about username availability. The column, the constraint,
the validator, and the availability endpoint are all new work.

AM-55 additionally collects a display name, which has no column either.

### 3. `GET /vehicles/{id}/summary` is not the source for AM-56's counters

It returns a **service** summary — `service_count`, `total_cost`,
`cost_last_year`, `reminders`, `by_category`. AM-56 AC1 wants build,
known-issue, part, and community counts. No endpoint produces those, and
`docs/mobile-feature-breakdown.md` already flags this screen as one that
"cannot render honestly at launch". See the decision in §Aha screen.

### 4. The 429 carries no remaining time, so AM-61 cannot be built as written

`ApiError::too_many_requests()` takes no arguments. AM-61's definition of done
is a countdown, which requires the server to say how long. Small API change.

### 5. Logout can report success while the session survives — a real defect on `dev`

`Authenticated` carries only `user_id` (`adapter/http/auth.rs:23`). Because the
handler has no session id, logout **rotates the refresh token** to discover
which session to end. When that rotation returns `Reused` or `Invalid` the
handler still answers success and revokes nothing.

Combined with an in-flight refresh whose response has not yet been persisted,
a person can press sign-out, receive a success, and be authenticated again a
moment later. AM-51 AC4 asks for the session to actually end; today the server
can say it did without doing it.

### 6. One lost refresh response signs the account out of every device

`Rotation::Reused` calls `sessions.revoke_all(user_id, false)`
(`usecase/auth.rs:117`) — every session on every device. That is the correct
response to a stolen token. But a benign sequence reaches it too: the server
rotates, the app is force-closed before the new pair is written, and the next
launch presents the old refresh token. The server sees a replay and signs the
person out of their phone, their tablet, and everything else.

The revocation stays. What changes is that the client must never present a
token it is not sure it still holds — see §The session contract.

### 7. Nothing on the server can say whether onboarding finished

AM-55 AC2 says adding the first car cannot be skipped. There is no `GET /me`,
no completion flag, and no server-side gate, so a deep link into a protected
route would bypass the wizard entirely. Resolved by deriving the state rather
than storing a flag — see §Bootstrap and the onboarding gate.

### 8. An attack that failed, recorded so it is not re-litigated

Returning remaining seconds on the 429 was challenged as an account-existence
oracle. It is not: the limiter counts a digest of whatever email was submitted,
before any user lookup (`adapter/redis/rate_limit.rs`, called at
`adapter/http/auth.rs:201`), so an unregistered address is counted identically
to a registered one. The countdown ships. What must **not** ship is anything
distinguishing the per-IP limit from the per-account limit, or reporting
attempts remaining — those would be the oracle.

---

## Decisions

### The session contract

**Single-flight refresh is mandatory, not an optimisation.** The server detects
refresh-token reuse and answers it by revoking every session. Two concurrent
requests that each refresh with the same token are indistinguishable from a
stolen token, so the naive implementation logs the person out precisely when
the app is busiest. Exactly one refresh runs at a time; every other caller
awaits that same promise and retries its request once.

**`expires_in` is a hint, never the truth.** The server sends a duration rather
than a timestamp so a client need not trust its own clock. It may be used to
refresh proactively; it may not be used to decide a token is still valid. A 401
is the only authority.

**A refresh in progress is written down before it is attempted.** Secure storage
holds one record: the token pair plus a `refresh_pending` marker. The marker is
set before the request goes out and cleared when the new pair is stored. Finding
it set at launch means the previous refresh's outcome is unknown — the client
discards its credentials and asks for a password rather than replaying a token
the server may already have rotated. One device asks for a login; the account
does not lose every other device.

**Sign-out is a transaction with an epoch, not a sequence of cleanups.** An auth
epoch counter increments first. Then: in-flight requests are cancelled, the
in-memory query cache is cleared, the persisted cache is deleted and the delete
is awaited, client stores are reset, secure storage is wiped, and exactly one
redirect happens. Any response that resolves after the epoch changed is dropped
instead of written. Without the epoch, a request that was already in flight
writes fresh data into a cache that was just cleared — which is how the next
account sees the previous account's garage.

**Exactly one redirect.** Ten requests failing to refresh is one sign-out, not
ten. The epoch makes this fall out for free: the first failure increments it,
and the rest see a stale epoch and do nothing.

### Fixing the logout defect (§5, §6)

`Authenticated` gains `session_id`. This is cheaper than it sounds, and the
reason is worth writing down so nobody designs something larger. The store
already keys two hops:

```
at:{digest(access)}  ->  session_id
sess:{session_id}    ->  user_id
```

`SessionStore::authenticate` (`adapter/redis/session.rs:235`) already walks
both — it reads `session_id` at line 238 and `user_id` at line 241 — and then
**discards the session id**, returning `Option<Uuid>` of the user alone. The
change is to stop discarding it. No new round trip, no new key, no schema.

Logout then revokes that session directly and never rotates a refresh token to
find it.

**Corrected 2026-08-20, during planning.** An earlier draft of this paragraph
said "a logout against an already-dead session answers exactly as one against a
live session". That is unsatisfiable and was wrong: once the route is gated by
`Authenticated`, a revoked token cannot authenticate at all, so a dead-session
logout is a 401 and a live one is a 200 — the extractor answers before the
handler is ever reached. The property actually worth holding, and the one the
implementation must satisfy, is the weaker and achievable one:

> Nothing distinguishes a logout carrying a dead token from **any other
> request** carrying a dead token.

Logout is not special-cased into leaking whether that particular session once
existed, whether it was revoked a second ago or a week ago, or whether the
token was ever real. It is refused exactly as `GET /vehicles` would refuse it.

This touches an extractor used by roughly thirty routes, so it carries a
correspondingly careful review. It is a type-level addition — no route's
behaviour changes except logout's, and any handler that only wants `user_id`
keeps compiling unchanged.

### Username

**Username is a public namespace, and this document says so deliberately.** It
becomes the profile address (`/@username` is already planned in
`docs/mobile-feature-breakdown.md`), so "this one is taken" is not a leak the
way "this email has an account" would be. The distinction is stated here
because the rest of this codebase is fanatical about not confirming account
existence, and a reader who found an availability endpoint without this
paragraph would reasonably read it as an oversight.

The guard rails that make it defensible:

- The availability endpoint is rate-limited, the same discipline every other
  unauthenticated endpoint that pays real per-request cost carries: `/auth/login`
  throttles by IP and by account, and `/auth/register` throttles by IP —
  both because they run argon2 before anything else can refuse the request.
- Taken and reserved answer **identically**. `admin`, `api`, `about`, `support`,
  `help`, `login`, `register`, `settings`, `profile`, `new`, `edit`, and the
  platform's own names are unavailable, and nothing distinguishes them from a
  name somebody holds.

  **Twelve names, not thirteen — `me` is deliberately absent.** This paragraph
  originally listed it. It cannot be reserved, because `me` is two characters and
  the minimum length is three, so `canonicalise("me")` fails before the reserved
  check is ever reached — `GET /usernames/me/availability` answers
  `422 {"username":"Minimal 3 karakter."}`. Listing it would be dead weight, and
  it would break the domain crate's own invariant that every reserved name is
  itself a valid username. Nothing is lost: the length floor already makes `me`
  unclaimable, so `/@me` can never collide with an account.
- The endpoint never accepts, returns, or is correlated with an email or any
  account state.

**Rules:** `a-z`, `0-9`, `.` and `_`; 3–30 characters; no leading or trailing
dot or underscore; no consecutive dots; normalised to lowercase on the way in.
Stored `CITEXT` for consistency with `email`, but **`CITEXT` is not the
validator** — case-insensitive uniqueness is not the same as a character rule,
and the rule lives in one canonicalising function on the server that both
register and the availability check call.

**The migration adds the column nullable with a partial unique index**, because
`NOT NULL UNIQUE` cannot be added to a table that already has rows without a
backfill nobody has designed. Existing accounts (there are none in production,
but the migration must be honest) keep a null username until they claim one.

`display_name` lands in the same migration and is also nullable — AM-55
collects it during onboarding, which by definition happens after the row
exists. It is plain `TEXT`, is not unique, and is not an identifier: two people
may both be "Budi", and the thing that distinguishes them is the username. A
person who has not finished onboarding has a null display name, which is one of
the two facts `GET /me` reports.

**The `23505` mapping must stop assuming.** `usecase/auth.rs::register`
currently maps any unique violation to `EmailTaken` with a comment saying it
"can only be the email index". Adding a second unique index makes that comment
false, and a username collision would be reported as a taken email. The handler
matches on the constraint name and reports the field that actually collided.

**Register and login stop sharing a DTO.** `CredentialsRequest` is used by both
handlers; adding a username field to it would make `/auth/login` demand a
username. Register gets its own request type. This also keeps the shipped login
contract byte-identical.

### Bootstrap and the onboarding gate

A new `GET /me` returns the caller's identity and the two facts the gate needs:
whether a username and display name exist, and whether the account has at least
one vehicle. Onboarding completion is **derived, not stored** — a person who
has a car has finished onboarding, and a stored flag is a second source of
truth that can disagree with the first.

The app calls it once at launch, after restoring tokens, and routes on the
answer: no session → welcome; session but no profile → profile step; profile
but no vehicle → wizard; otherwise → the app shell. A deep link into a
protected route is held until this resolves, which is also what makes AM-55
AC2's "no skip" real rather than a missing button.

### Storage split

Tokens live in `expo-secure-store` and nowhere else. Server state lives in
TanStack Query, persisted to MMKV. Client state — session status, active
vehicle, form drafts — lives in zustand. This split is the repo's existing
decision (`docs/mobile-feature-breakdown.md` §1.1) and is followed rather than
re-opened. The one addition this design makes to it: **the persisted cache is
keyed per account**, so switching accounts cannot surface the previous one's
data even if a delete were to fail.

### Rate-limit feedback

The 429 gains `retry_after_seconds`: a single aggregate number, the larger of
whichever limiter refused. It never says which limiter that was, and never
reports attempts remaining. The client counts down and re-enables the button
when it reaches zero.

### The aha screen renders in AC2 mode only

AM-56 AC1 asks for build, known-issue, part, and community counts. Nothing
computes them, and the project's own rule is that nothing is seeded with fake
data. The screen ships as AC2 describes — the "be the first" state with one
concrete action — plus AC3's AI invitation and AC4's exit to the shell. AC1 is
deferred behind a seam: when an endpoint exists, the counter block drops in
without the screen being redesigned. This is not a shortcut; AM-56's own
technical note says "AC2 adalah inti story ini. Saat rilis, hampir semua
pengguna berada di kondisi itu."

### The app shell is honest about empty tabs

AM-16 wants five tabs — Home, Garage, Explore, Community, Profile. Explore and
Community belong to epics with no implementation, so their tabs render the
empty state the design system already provides, saying plainly what will live
there. AM-16's own out-of-scope line ("isi setiap tab") makes this the correct
reading, not a shortcut. The global add action offers only the entries whose
forms exist by then; the others are absent rather than present-and-broken.

---

## Architecture

```
apps/mobile/src/
  shared/
    api/
      client.ts          fetch wrapper: base URL, request id, auth header,
                         401 -> single-flight refresh -> one retry
      refresh.ts         the single-flight promise and the pending marker
      errors.ts          network vs validation vs server, mapped to messages
      queryClient.ts     TanStack Query + per-account MMKV persistence
    session/
      store.ts           zustand: status, user, epoch
      secure.ts          expo-secure-store read/write of {tokens, pending}
      signOut.ts         the epoch transaction
  features/
    auth/                schemas (zod), hooks, screens
    onboarding/          profile step, wizard steps, draft persistence
    vehicle/             catalog queries for the wizard
app/
  (auth)/                welcome, login, register
  (onboarding)/          profile, wizard, aha
  (app)/                 tabs
  _layout.tsx            providers + the bootstrap gate
```

The route groups are what make the gate declarative: a group's layout decides
whether its subtree may render at all, so no screen needs its own guard.

---

## Error taxonomy (AM-17 AC4)

Four kinds, four different things to say:

| Kind | Cause | What the person sees |
|---|---|---|
| Offline | no connectivity | "Tidak ada koneksi" + retry |
| Validation | 422 with field details | messages under the fields that failed |
| Rate limited | 429 | the countdown from `retry_after_seconds` |
| Server | 5xx, or a malformed response | "Ada gangguan di server" + retry, never a raw error |

The API's envelope (`meta` · `data` · `error` with a stable `code`) is what the
mapping keys on. Its messages are already Bahasa Indonesia by default with
`Accept-Language` support, so the client sends the header and prefers the
server's message where one exists rather than inventing a second copy.

---

## Testing

**Server:** the existing integration suite is the model. New cases: username
rules at the boundaries (2 and 3 characters, 30 and 31, leading dot, trailing
underscore, consecutive dots, uppercase normalisation), a username collision
reported as a username collision and not an email one, reserved names
answering identically to taken ones, `retry_after_seconds` appearing only on a
429, `GET /me` reflecting each onboarding stage, and — the important one —
**logout revoking the session without rotating the refresh token, and a logout
against an already-dead session answering identically to a live one.**

**Mobile:** there is no test runner in `apps/mobile` today and this work does
not add one. The pure logic that deserves tests — the username validator, the
error mapper, the single-flight refresh state machine — is written so it can be
tested when a runner arrives, and until then is verified by exercising the
flows on a simulator with the screens open. The one exception worth arguing
about is the refresh state machine; if a runner is ever added, it is the first
thing to cover.

---

## Tidak boleh ada

- No JWT. The session tokens are opaque on purpose so that logout revokes
  rather than waits.
- No token in MMKV, in the query cache, in a log, or in a URL.
- No second copy of the username rules on the client. The client may mirror the
  regex for instant feedback, but the server's canonicaliser is the authority
  and the client never decides a name is acceptable on its own.
- No invented counts on the aha screen, no seeded vehicles, no placeholder
  community numbers.
- No skip button in the first-car wizard.
- No distinguishing an unknown email from a wrong password, anywhere, ever —
  including in analytics, logs, or a "did you mean to register?" hint on the
  login screen.
- No second redirect on session expiry.
- No social login, no email verification, no password reset, no biometrics,
  no two-factor. Those are AM-52, AM-53, AM-54, AM-77 and stay theirs.
- No offline writes. AM-18 explicitly scopes to reading from cache.
- No tab content for Explore and Community beyond an honest empty state.

---

## Plan split

Four plans, executed in order, each its own branch and pull request into `dev`.

| Plan | Contents | Closes |
|---|---|---|
| **A — Session foundation** | *Backend:* `session_id` on `Authenticated`, logout without rotation, `GET /me`, `retry_after_seconds`, the username + display-name migration, the canonicaliser, the availability endpoint, the register DTO split, the constraint-name mapping. *Mobile:* API client, secure storage with the pending marker, single-flight refresh, the sign-out epoch transaction, error taxonomy, TanStack Query with per-account persistence, zustand stores, the bootstrap gate and route groups. | AM-17, AM-18 |
| **B — Auth screens** | Welcome, register (live validation, debounced availability, ToS consent), login (uniform error, rate-limit countdown), the already-registered path that carries the email across to login. | AM-50, AM-51, AM-57, AM-59, AM-60, AM-61 |
| **C — App shell** | Five tabs with preserved stacks, the global add action, the real home screen, honest empty states for Explore and Community. | AM-16 |
| **D — Onboarding** | Profile step, the six-step catalog wizard with draft persistence, the aha screen in AC2 mode, and the handoff into the shell. | AM-55, AM-113, AM-56 |

Plan A carries a backend half and a mobile half, in that order — the mobile
half is typed against contracts the backend half establishes, and writing them
concurrently is how a client ends up coded against a signature that does not
exist.

---

## Process record

**Cross-model adversarial pass (Codex, `gpt-5.6-terra`): run.** It refuted the
first draft of this design in four places, and every objection it raised was
independently verified against the code before being accepted: the logout
false-success (§5), the revoke-all-on-lost-response footgun (§6), the shared
register/login DTO, and the `23505` mapping. Its challenge to the 429 countdown
failed on inspection and is recorded in §8 so it is not raised again. Its
recommendation to move the onboarding gate into Plan A was accepted.

**`grill-with-docs`: recommended, not run.** The owner elected to proceed. Its
absence is recorded rather than glossed; the Codex pass covered the adversarial
role, and this repository has no `CONTEXT.md` or ADR set for the grill's
document-contradiction sweep to work against beyond `docs/design.md` and
`docs/mobile-feature-breakdown.md`, both of which were read directly while
writing this.

**Open decisions the owner still owes:** none blocking. Two things will need a
call later — whether the aha screen's counters get an endpoint before launch,
and whether existing accounts (none today) get a username claim flow or a
backfill.

## Corrections made after the four plans were written

Planning found four things this document got wrong or left open. They are fixed
above and listed here so the change is visible rather than silent.

1. **The logout-uniformity sentence was unsatisfiable** — corrected in place,
   with the achievable property stated instead.
2. **The frozen client contract had no way to *start* a session.** `signIn` was
   missing entirely: the contract could read a session and end one, and nothing
   could begin one, which both auth screens need. Added, owned by Plan A.
3. **A taken email or username was indistinguishable from a server fault.** The
   client error taxonomy had no path for a collision, so "email ini sudah
   terdaftar" would have surfaced as "Ada gangguan di server" and broken AM-50
   AC3. Collisions are 409 with `error.details` naming the field, mapped
   client-side to a validation error on that field.
4. **`refreshMe` was missing**, and its absence hides a trap worth recording:
   after `POST /vehicles` the cached `me.hasVehicles` is still `false`, so
   navigating before the refresh resolves bounces the person back into the
   wizard they just finished — while `hasVehicles` must be *read* before the
   refresh to decide between the aha screen and the garage.

Two further hazards were found in the database layer and belong here because
they are properties of this schema, not of any one plan: **`citext` overloads
`~` to be case-insensitive**, so a `CHECK` constraint written the obvious way
would accept `BUDI` as a valid lowercase username unless the column is cast
with `::text`; and **sqlx cannot map `citext` to `String`**, so the first query
that reads one needs an explicit cast or the macro fails on an unknown type.
