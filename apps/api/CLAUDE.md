# apps/api — backend instructions

Rust · axum · Postgres + pgvector · Redis. A modular monolith: one codebase, one database, one binary, two process roles.

Repository-wide rules are in [../../CLAUDE.md](../../CLAUDE.md). This file covers the backend.

## Structure

```
apps/api/
├── Cargo.toml              [workspace] members = ["crates/*"], resolver = "3"
├── clippy.toml             test-only exemptions for unwrap/expect/panic
├── deny.toml               licence and advisory policy
└── crates/
    ├── domain/             PURE — entities, value objects, errors, policy
    │   └── src/{identity,garage,build,knowledge,ai,waitlist}/
    └── runtime/
        ├── migrations/     sqlx resolves these relative to THIS crate
        └── src/
            ├── main.rs     launcher — starts a runtime, calls run()
            ├── lib.rs      role dispatch, startup and shutdown order
            ├── usecase/    application services — own the transaction
            ├── adapter/    HTTP, Postgres, Redis, S3 — translate only
            ├── platform/   config, logging, state, shutdown
            └── shared/     envelope, errors, error codes, request id
```

Three layers, three jobs: **domain decides**, **usecase orchestrates**, **adapter does I/O**.

## `runtime` is a library with a thin binary

`main.rs` starts a tokio runtime and calls `run()`. Everything else lives in `lib.rs` and below.

This is the `cmd/server/main.go` shape of the Go boilerplate, and in Rust it buys two things beyond tidiness. Integration tests under `tests/` can drive the real router. And a type written before its first caller is public API rather than dead code — which is what let the response envelope be built and tested before any endpoint used it, without a blanket `#[allow(dead_code)]` that would also have hidden the parts that genuinely were unused.

## The shared kit

`shared/` holds what every adapter needs and no layer owns: the response envelope, the failure-to-HTTP mapping, error codes with their messages, and the request id.

Only the parts with a consumer exist. `pagination`, `validation`, and `security` arrive with the first list endpoint, the first request DTO, and authentication respectively — an empty directory named after a future need is scaffolding, not structure.

**Error messages are Bahasa Indonesia by default**, unlike the Go boilerplate's English-first catalog. An error message is product text, and the repository rule puts product text in Indonesian. English is available through `Accept-Language`.

## Request id and language are task-local

Both are set once by the HTTP middleware and read wherever they are needed.

The obvious alternative — pass them as handler arguments — works for a successful response and fails for every other one. `?` returns an error value that has nowhere to carry a request id, and the error path is exactly where a support conversation starts. A task-local is safe here in a way a global is not: axum drives each request on its own task, and the middleware scopes the value to that task's future.

The request id is always minted here, never read from an inbound `X-Request-Id`. A caller-supplied value that lands in a log line is how log injection works.

## What a log line may contain

Method, matched route pattern, status, latency, request id. Nothing else.

Never the URI — a URI carries whatever the caller put in it, and a password-reset token in a path segment is the normal case, not the exotic one. A request that matches no route logs the constant `unmatched`, because that is precisely when the URI is most likely to hold something that should not be written down.

Never a request body, never a whole struct. A VIN cannot leak from a value that is never handed to a logging macro.

A cause is rendered at exactly one place: the 5xx branch of `ApiError::into_response`, into the log. `ApiError::source` is private with no accessor, so no handler can route a cause into a response body even by accident.

## The boundary is the dependency list

`crates/domain/Cargo.toml` contains `thiserror`, `uuid`, and `time`. No `axum`, no `sqlx`, no `reqwest`, no `redis`, no `tracing`, and no `serde`.

That absence *is* the architecture. `use axum::Json;` inside the domain crate produces `error[E0432]: unresolved import` — not a lint that can be allowed away. `make be-boundary` asserts it directly, and CI runs it on every push.

Before adding a dependency there, ask whether it is the language or infrastructure. `serde` is excluded deliberately: entities are never serialised, wire shapes are DTOs in `adapter`, and letting serde in lets the wire format start quietly dictating the domain model.

The two crates do **not** prevent cross-context leaks *within* `domain` — `ai` can reach into `garage` today. Keep each module's public surface small and mark internals `pub(in crate::<module>)`, so the seam stays visible if a context ever has to be extracted.

## Use cases live in `runtime`, policy lives in `domain`

An earlier design put use cases in the pure crate. It does not compile: a use case needs a repository, repositories live in `runtime`, and `domain → runtime → domain` is a cycle Cargo rejects.

What stays pure is **policy** — no async, no I/O, just data in and a decision out:

```rust
// domain/src/garage/policy.rs
pub fn derive_reminders(history: &[LastService], odometer_km: Option<i32>, today: Date) -> Vec<Reminder>
```

Tested by constructing values and asserting. No database, no mock, no
async runtime. `today` is an argument rather than a call to the clock,
which is what lets a test put a car two years past its oil change without
waiting two years — and the same reason the rollup query takes its
twelve-month cutoff as a parameter instead of writing `CURRENT_DATE`.

`ServiceCategory` exists twice on purpose: once in `domain` and once in
`adapter/postgres` carrying the sqlx and serde derives the domain crate
has no dependencies for. The persistence model is not the domain model,
and the two `From` impls are non-exhaustive matches the moment either
side gains a variant — the compiler reports the drift.

## Repositories take a connection, never a pool

```rust
// adapter/postgres/garage_repo.rs
pub async fn insert_service(tx: &mut PgConnection, rec: &ServiceRecord) -> Result<Uuid, sqlx::Error>
```

This is what lets the **use case** own the transaction boundary — open, authorise, write, re-read under `SELECT … FOR UPDATE`, call policy, write the result, commit. A repository holding its own pool turns every call into a separate transaction, and two concurrent requests then interleave into stale derived state.

## Rollups are one query for every car, not one per car

A garage list that shows spend and overdue counts per row is the exact
shape that becomes a query per car without anybody noticing, because the
loop is over rendered rows rather than over `await`s. `summary_repo`
therefore answers for **every one of the owner's cars at once**, even
when the caller wants a single car: one aggregate scan for the totals,
one `DISTINCT ON` for the latest service of each kind.

The property is structural — `service_summary::for_list` makes exactly
two repository calls, neither inside a loop — and it is **not pinned by a
test**. Counting queries needs `pg_stat_statements`, which needs
`shared_preload_libraries` and a server restart, which a GitHub Actions
service container cannot set; such a test would pass locally and silently
skip in CI. What *is* tested is the failure batching actually produces:
`one_cars_totals_never_land_on_another_cars_row` catches totals grouped
by the wrong key or result sets zipped out of order — plausible numbers
on the wrong car. If you add a third rollup, keep it out of the loop and
say so here.

Summing money is left to the database. A rollup computed in Rust means
loading every service record a car has ever had to produce one number.

## Repositories do not get traits

A port becomes a trait only when one of these holds:

1. the adapter will genuinely be swapped,
2. orchestration needs an I/O seam to be tested without a network, or
3. two real implementations exist today.

Today only `LlmPort` and `EmbeddingPort` qualify — both are HTTP to external services, both need a fake in tests, and both will be swapped as cost and quality change. They arrive with AM-363 and AM-364.

Postgres repositories do not qualify. There is one implementation and there always will be, and `#[sqlx::test]` provides a transactional test database that also exercises the real SQL — more honest than a fake, and cheaper than fighting `Arc<dyn Trait + Send + Sync>` with `#[async_trait]`.

## sqlx

Compile-time macros (`query!`, `query_as!`). Never build SQL with `format!`.

**The trap:** when `DATABASE_URL` is set, the macros query the live database and silently ignore the committed `.sqlx` cache. A stale cache then passes locally and fails on any machine without a database. Builds therefore run with `SQLX_OFFLINE=true`, and CI runs `cargo sqlx prepare --check --workspace`. Re-run `cargo sqlx prepare` and commit the result whenever a query or migration changes.

Migrations live in `crates/runtime/migrations/`, not at the workspace root — `sqlx::migrate!()` and `#[sqlx::test]` resolve the path relative to `CARGO_MANIFEST_DIR`.

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
- **An `effect_key` is `<kind-prefix>:<id>`, by convention rather than by constraint.**
  The unique index is on `effect_key` alone, not on `(kind, effect_key)`. Two different
  job kinds that happened to share an id space would collide on that index, and the
  second enqueue would silently be dropped as a duplicate of the first. The kind prefix
  is the only thing keeping each kind's ids in its own namespace — nothing in the schema
  enforces it, so a new kind must pick a prefix nothing else uses.
- **An `effect_key` is derived on the server, from an id the server already trusts —
  never from a request field.** If one were built from client input, a caller sending
  `media:<someone else's id>` would suppress that media's processing for as long as the
  forged key stays live: `enqueue` returns `Ok(None)` ("already queued, do nothing") and
  nothing anywhere records that the suppression happened.
- **A validation failure never retries.** `JobFailure::Permanent` dead-letters on the
  first attempt; only `Transient` backs off. A malformed input fails identically every
  time, and eight attempts to discover that delay every other job.
- **`payload` and `last_error` have no shape or size bound at the schema level.** The
  rule lives in a column comment and in `ERROR_MAX_CHARS`, not in a `CHECK` constraint.
  There is no retention or cleanup of `done` rows, so anything written into `payload`
  lives in this table forever and travels into every backup from then on. The consumer
  that defines a job kind's payload shape is the only enforcement point there is.
- **`LEASE` must exceed the longest a job can take — it is 300 seconds, and that number
  is bound to AM-359's decode wall-clock limit specifically.** A job that runs longer
  has its own lease expire under it and gets handed to a second worker — the log line
  for that is `lost the lease before settling`, and it means the bound on the job is
  wrong, not that the worker is. The two numbers live in different modules and nothing
  connects them automatically; this sentence is the connection, the same way spec §9
  records the staging-TTL relationship for the same reason.
- **`attempts` increments on the claim, not on the failure**, so a worker killed mid-job
  still burns an attempt and a crash-looping payload still reaches the cap.

`anakmobil queue-stats` prints the age of the oldest job still owed work and how many
gave up. There is no metrics service and no probe port: a listener beside a job loop
answers `200` while the loop is deadlocked, whereas the oldest-pending age climbs.

## Authentication

Sessions live in Redis and tokens are opaque, never JWTs. A signed JWT cannot be revoked, only waited out, so logout would not be an act.

Rules that are easy to undo by accident:

- **Redis stores digests, never tokens.** `token_digest` is the only way a token becomes a key.
- **`sess:{id}` is the sole authority.** Authenticating requires both the token mapping and the session. Adding a shortcut that trusts the token mapping alone would make logout stop working, silently.
- **Rotation and session creation are Lua scripts.** Splitting either into separate commands reintroduces a race that mints two live token chains for one session. `tests/session_store.rs` fails against the non-atomic version — that is checked, not assumed.
- **A replayed refresh revokes every session but does not fence the account.** Fencing would let a stolen token permanently lock out its victim. Only account deletion fences.
- **Every credential failure returns the same status, code, and message.** Unknown email, wrong password, unparseable stored hash. Splitting them turns login into an account-enumeration oracle.
- **Login costs one argon2 verification whether or not the account exists.** Returning early on a missing user leaks the same thing through timing.
- **Never log a token, a digest, or an email on the auth path.** A user id is not a credential and is enough to investigate.

The integration tests need a real Postgres and a real Redis.

**They do NOT skip loudly, and this sentence used to claim they did.** Without `DATABASE_URL` and `REDIS_URL` the `app!` macro returns early and every test reports `ok` — and the word `SKIPPED` never reaches cargo's output, because cargo captures stderr for passing tests. Measured: 13 tests "pass" having executed nothing.

**Every one of those guards now fails loudly rather than skipping** — missing URLs, an unusable `DATABASE_URL`, a database that will not migrate, an unreachable Redis. Deliberately skipping is still possible and now has to be said out loud:

```bash
AM_SKIP_INTEGRATION=1 cargo test    # unit tests only, on purpose
make be-test                         # the normal path; loads .env
```

**The first fix for this was incomplete, and the incompleteness is the lesson.** Only the missing-URL guard was made loud; the three below it still returned. So when the Docker daemon died, the whole suite reported fifteen green boards with no database at all — the same false green, one guard over, and reported as closed. A partial fix to a silent-failure bug is worse than none, because it buys confidence the code has not earned.

## Migrations

**One migration per story, not one schema up front.** A table written before a query uses it is usually the wrong shape, and fixing it costs a migration anyway — so the work is not saved, only done when there is least information. Expand-and-contract exists so the schema can grow this way.

```bash
cd crates/runtime && sqlx migrate add -r <name>
```

Always `-r`. Writing the down migration is what forces you to notice a change that cannot be undone, while choosing a different one is still cheap.

Migrations run automatically at the start of the **web** role, before the listener binds. The worker deliberately does not run them — one role owns applying them, and a worker reading a schema the web role has not migrated yet is exactly the case expand-and-contract makes safe. `anakmobil migrate` applies them and exits; use it in CI, and for anything long enough that a health check would kill the process mid-migration.

**A migration that has been applied is never edited.** sqlx stores a checksum and refuses to continue when it changes, because two databases would otherwise carry the same version number and different schemas. Fix a mistake with a new migration.

**One boundary, and it is narrow enough to check.** That rule exists to prevent divergence *between databases*. A migration still on an unmerged branch, applied only to your own throwaway development database, has no other copy to diverge from — so it may be amended in place, followed by `make db-drop` to rebuild from the amended file. **Four** conditions, all required: **not merged**, **not pushed**, **nothing else is running against that database**, and **you reset it**. The moment it reaches `dev` — or a branch anybody else, including CI, may have migrated — the rule is absolute again.

The reset is self-enforcing rather than an honour system: sqlx stores a checksum in `_sqlx_migrations` and refuses to run against an amended file, so skipping it stops you at the tool rather than at your own discipline.

**The third condition was added after this rule failed in exactly the way it did not anticipate.** It assumed one person and one database. With two or three agents sharing this development database, amending a migration and resetting *your* copy leaves every concurrent process holding the old checksum — and `sqlx::migrate!` then fails for **every test file in the workspace**, not just the one you touched. It failed **silently**, because the `app!` macro swallows the message and cargo captures stderr for passing tests, so the whole suite reported green having executed nothing.

The recovery is `make db-drop`. The lesson is the condition: amend only when you are the only thing touching that database.

Use it for a mistake found hours after writing, where three corrective migrations against a table created the same afternoon would read as three errors rather than one correct schema. Do not use it to avoid writing a down migration, and never for a migration somebody else may have run.

### Expand, then contract

A deploy has both versions running at once, so a schema change must be readable by the old code:

1. Add the new column nullable, or with a default. Never rename in place — add, backfill, and drop in a **later** release.
2. Ship the code that writes both and reads the new one.
3. Drop the old column in a release after that.

### Conventions every migration follows

- **UUID primary keys**, generated by the application as v7. Time-sortable, so inserts land at the end of the index rather than scattering across it; safe in a URL, unlike a serial, which tells anyone who looks how many rows exist.
- **`TIMESTAMPTZ`, never `TIMESTAMP`.** The server is UTC and the users are in three Indonesian time zones.
- **`created_at` and `updated_at` on every table**, with `updated_at` maintained by the `set_updated_at()` trigger rather than by application code — including a manual `UPDATE` run during an incident.
- **Every foreign key gets an index.** PostgreSQL indexes primary keys and unique constraints; it never indexes a foreign key, and the missing one turns a parent delete into a sequential scan of the child.
- **Money is `NUMERIC`, never a float.** Rupiah has no subunit in practice, and that is not a reason to store money in a type that cannot represent it exactly.
- **Automotive specification is numeric.** "PCD 5x114.3" is a bolt count and a circle diameter — two columns. Offset is signed, because a deep-dish wheel has a negative ET and an unsigned column would reject exactly the wheels people fit. Stored as text, every comparison would parse first, and `5X114.3` and `5x114,3` would be different cars.
- **Closed sets are native enums.** `ALTER TYPE … ADD VALUE` does not break a running older version. Removing a value is genuinely hard, which is the honest cost of declaring the set closed.
- **Content tables carry a status the indexer can see.** Reported, hidden, and deleted content is never cited as evidence, and that requires a path that pulls it back out of the index.

## `part_merges` is append-only by discipline, not by constraint

The table's own comment says a merge is never deleted, because undo reads it
to know what the previous state was. The database does not enforce that:
verified, an `UPDATE` can rewrite `source_part_id` on a landed merge and a
`DELETE` can remove history outright.

Acceptable today — no admin surface, one application role, and no statement
anywhere that deletes from it. What would enforce it when there is one: a
`BEFORE UPDATE` trigger permitting only the `undone_at NULL → non-NULL`
transition, or `REVOKE DELETE` from the application role. The same trigger
would also force an undo to name its actor, which the one-way CHECK cannot.

Written here rather than in the migration because the migration is merged and
its checksum is frozen — and because a comment claiming a guarantee the schema
does not provide is the defect this ticket already burned a finding on.

`role_changes` is the version that does. Its migration ships a
`BEFORE UPDATE OR DELETE` trigger that raises, so the guarantee is in the
schema rather than in a comment, and both foreign keys are `ON DELETE
RESTRICT` rather than `SET NULL` — a referential action is an ordinary write
to the child row, and the trigger would reject it, taking the whole parent
`DELETE` down with it. `part_merges` predates that reasoning and its migration
is merged, so its checksum is frozen; the same trigger would fit it whenever a
new migration is worth writing.

## Async

Never block the runtime: no `std::thread::sleep`, no synchronous driver, no `reqwest::blocking`, no heavy CPU inside an `async fn`. CPU work goes to `spawn_blocking`, or to the worker role.

Never hold a `std::sync::Mutex` guard across `.await` — clippy's `await_holding_lock` is denied for this reason.

Bound fan-out with `tokio::sync::Semaphore` and collect with `JoinSet`. State the time and space complexity before writing a loop over a collection or a query in a hot path; feeds and Explore are the two paths most likely to become N+1.

## Errors

`thiserror` enums in `domain`, `anyhow` only at the binary boundary. Propagate with `?`; never `let _ = fallible();`.

Domain errors map to HTTP at exactly one choke point — a single `impl IntoResponse for ApiError` whose `match` on the error enum yields `(StatusCode, code)`. Adding a variant makes that match non-exhaustive, so the compiler refuses to let a mapping be forgotten. That lands with AM-351.

5xx responses are generic to the client; detail goes to logs. SQL, file paths, configuration values, and tokens never appear in a response body.

## Verification

```bash
make be-check      # fmt · clippy · tests · boundary assertion
```

Or individually:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace          # cargo nextest run once it is installed
cargo llvm-cov                  # ≥ 90% on new code
cargo audit && cargo deny check
```

Never lower a threshold, blanket-`#[allow]` a lint, or delete a failing test to go green. Fix the cause.

## Conventions

Config is a hand-written `Config::from_env() -> Result<Config>` plus `dotenvy` — no config library. It reports **every** problem in one pass, and secrets are wrapped in `Secret<T>` whose `Debug` prints `Secret(<redacted>)`.

Process role comes from the first CLI argument, matched manually: `anakmobil web`, `anakmobil worker`. Reach for `clap` when there are more subcommands than that, not before.

Deliberately absent, and not to be reintroduced without a decision: CQRS, event sourcing, layered aggregate roots, a separate `application` crate, domain events with no consumer, and repository-per-entity.
