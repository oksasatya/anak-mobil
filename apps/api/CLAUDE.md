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
            ├── main.rs     role dispatch on the first CLI argument
            ├── usecase/    application services — own the transaction
            ├── adapter/    HTTP, Postgres, Redis, S3 — translate only
            └── platform/   config, shutdown, later logging and pools
```

Three layers, three jobs: **domain decides**, **usecase orchestrates**, **adapter does I/O**.

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
pub fn derive_reminders(vehicle: &Vehicle, history: &[ServiceRecord], today: Date) -> Vec<Reminder>
```

Tested by constructing values and asserting. No database, no mock, no async runtime.

## Repositories take a connection, never a pool

```rust
// adapter/postgres/garage_repo.rs
pub async fn insert_service(tx: &mut PgConnection, rec: &ServiceRecord) -> Result<Uuid, sqlx::Error>
```

This is what lets the **use case** own the transaction boundary — open, authorise, write, re-read under `SELECT … FOR UPDATE`, call policy, write the result, commit. A repository holding its own pool turns every call into a separate transaction, and two concurrent requests then interleave into stale derived state.

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
