# AM-355 — Two-role authorization, an audit trail, and cost filtering in the query

> **For agentic workers:** REQUIRED SUB-SKILL: use `executing-plans-hybrid` to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** give the platform a second kind of account — an admin — with a check that cannot be forgotten, an append-only record of every change to it, and one cost filter moved out of Rust and into SQL.

**Architecture:** a Postgres enum `platform_role` on `users`, read fresh on every admin request by a second `FromRequestParts` extractor beside `Authenticated`. One use case (`usecase::roles::set_role`) owns the transaction and the advisory lock, and is reached from two entrances: an HTTP `PATCH` and an operational CLI command. The AC3 work is a refactor with an invariant — the cost filter moves from `visible_cost` in Rust into the `modifications_for` query, and the tests that pinned it are rewritten rather than discarded.

**Tech Stack:** Rust 1.96 (edition 2024) · axum 0.8.9 · sqlx 0.9.0 (compile-time macros, offline cache) · Postgres 17 · Redis · tokio 1.53.

**Spec:** [`docs/superpowers/specs/2026-08-17-am-355-two-role-authorization-design.md`](../specs/2026-08-17-am-355-two-role-authorization-design.md) — read it alongside this plan. Its reasoning is load-bearing and several of its decisions reversed under review; the section headed **Decisions the plan may not quietly change** below lists the ones that will look wrong and are not.

---

## Global constraints

Copied verbatim from the spec and from `apps/api/CLAUDE.md`. Every task's requirements implicitly include this section.

- **Three unrelated things are called "role"** (`CONTEXT.md`). This ticket adds the **platform role** (`user` | `admin`), stored in `users.platform_role`. `runtime::Role` is the **process role** (`Web | Worker | Migrate`) and is not extended. Community membership will bring a third sense and gets its own column when it arrives. Never one column for two of them.
- **Product-facing text is Bahasa Indonesia.** Error messages, validation messages. Everything written for developers — code, comments, commit messages, docs, this plan — is English.
- **A log line may contain** method, matched route pattern, status, latency, request id, and a user id. **Never** an email, never a `reason`, never a URI, never a request body, never a whole struct. The AM-361 fix pass had to remove a caller-supplied `brand` from a log line; the same mistake is available here.
- **Private vehicle data** — plate, VIN, purchase price, service costs — never leaves the server for anyone who should not see it, **including an admin**.
- **The domain crate imports no framework.** Nothing in this ticket touches `crates/domain/`. There is deliberately **no pure-domain `PlatformRole`** — see the prohibitions below.
- **`unsafe_code` is forbidden**; `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `await_holding_lock` are **denied** on production paths and allowed in `#[cfg(test)]` (`apps/api/clippy.toml`). `too_many_arguments` is a warning and `-D warnings` makes it fatal.
- **Migrations are `-r` pairs**, live in `apps/api/crates/runtime/migrations/`, and an applied one is never edited.
- **Commits are English, small, and each one builds.** Do not commit or push unless asked; the branch is `feat/AM-355-two-role-authorization`.

---

## Environment card — Block A

**Paste this verbatim into every writer brief and every reviewer brief.** None of it is discoverable from the spec, and every line cost real time on AM-360 or AM-361.

```
ENVIRONMENT — AnakMobil backend, read before running anything

1. Every `make` target runs from the REPOSITORY ROOT, never from apps/api.
   `cd apps/api && make be-test` gives "No rule to make target".

2. Postgres is on 127.0.0.1:55432 (Docker Compose project `anakmobil`),
   NOT 5432. `make db-up` starts it. Redis is the machine's own on 6379;
   `make db-up-all` starts a containerised one behind the `redis` profile.

3. `.env` lives at the repository root and belongs to the BACKEND. The
   Makefile loads and exports it, which is why `make be-test` sees
   DATABASE_URL and REDIS_URL and a bare `cargo test` does not.

4. Run the gates through `make`, never a bare `cargo clippy` or `cargo test`.
   The Makefile sets `SQLX_OFFLINE ?= true`; without it the sqlx macros load
   .env themselves and check against the LIVE database, whose nullability
   inference can differ from the committed .sqlx cache — producing a compile
   error in a file nobody touched.

5. A bare `cargo test` PANICS loudly rather than skipping, as of PR #18.
   `AM_SKIP_INTEGRATION=1` is the deliberate opt-out. Never write a test
   harness that returns early and reports green.
   COPY THE HARNESS FROM `tests/build_list_flow.rs` (the `app!` macro that
   returns `(app, pool)`). DO NOT copy `tests/part_merge_flow.rs`'s `pool!`
   macro or `tests/session_store.rs` — both still return silently when their
   URL is unset, and copying one reintroduces the bug PR #18 closed.

6. `cargo sqlx prepare` CLEARS .sqlx before regenerating. A failed run leaves
   the cache empty and breaks the offline build for everybody. Use
   `make be-prepare`, which prepares against a throwaway `anakmobil_prepare`
   database — a populated dev database changes query plans and sqlx infers
   column nullability FROM the plan, so preparing against real data generates
   different Rust types. Verify with `make be-sqlx-check` (what CI runs).
   `make be-prepare` and `make be-sqlx-check` both DROP and CREATE the fixed
   database name `anakmobil_prepare`. Two of them running at once destroy
   each other's database.

7. `cargo sqlx prepare` while the crate does not compile writes an INCOMPLETE
   cache that then fails --check in CI. Fix compilation first.

8. Migrations live in apps/api/crates/runtime/migrations/, resolved relative
   to CARGO_MANIFEST_DIR — not at the workspace root. Create them with
   `cd apps/api/crates/runtime && sqlx migrate add -r <name>`. Always `-r`.

9. A MERGED migration is never edited — sqlx stores a checksum and refuses.
   apps/api/CLAUDE.md permits amending an unmerged one under FOUR conditions,
   all required: not merged, not pushed, nothing else running against that
   database, and you reset it with `make db-drop`. That rule already failed
   once against the person who wrote it. If in doubt, write a new migration.

10. `BigDecimal` renders trailing zeros when decoded from Postgres NUMERIC —
    114.3 comes back as "114.3000". `.normalized()` before `.to_string()`,
    or `.with_scale(2)` where a money scale is pinned.

11. Rust 1.96, edition 2024, axum 0.8.9, sqlx 0.9.0. The pool is
    max_connections(10) with a 5s acquire timeout
    (adapter/postgres/mod.rs). tokio features are
    rt-multi-thread, macros, signal, net, time — there is NO `io-std`,
    so `tokio::io::stdin()` does not exist here.

12. `ConnectInfo` must be inserted into a test request's extensions or every
    rate-limited route fails. The `send` helper in the flow tests does it.

13. Gate chain, in this order, from the repository root:
      make be-lint            # fmt is separate: cargo fmt --check
      make be-boundary
      make be-prepare && git add apps/api/.sqlx    # if a query or migration changed
      make be-sqlx-check
      make be-test
    Check EXIT CODES. Piped output is not evidence.
```

---

## Rust quality gate — Block B

**Paste this verbatim into every brief that writes Rust.** This repository does **not** run Sonar; clippy is the gate, and telling an implementer to invoke Sonar sends them after a tool that is not installed.

```
# Rust quality gate — write compliant from the first commit (NO Sonar; clippy is the gate)

- clippy::too_many_arguments — ≤7 params (aim ≤5); past that, a params struct.
  `shared::validation::DecimalSpec` is this repository's own example of the fix.
- clippy::cognitive_complexity — extract named helpers; early-return `?`;
  flatten with `let ... else`.
- NO `.unwrap()` / `.expect()` / `panic!` / `todo!()` on production paths —
  the workspace denies them (apps/api/Cargo.toml [workspace.lints.clippy]).
  Return `Result` + `?` / `ok_or` / `unwrap_or_default`. Tests are exempt
  (apps/api/clippy.toml sets allow-*-in-tests).
- Duplicated string literal 3+ times → a module-level `const`.
- `#![forbid(unsafe_code)]` is already workspace-wide. Do not add an exception.
- Errors: `thiserror` enums in the use case; `anyhow` only at the binary
  boundary. Never `let _ = fallible();`. Domain→HTTP mapping happens at exactly
  one choke point per module (`to_api_error`), and `ApiError::internal` is the
  only thing that carries a cause — which is logged, never serialised.
- Async: never hold a `std::sync::Mutex` guard across `.await`
  (clippy::await_holding_lock is denied); never block the runtime — no
  `std::thread::sleep`, no synchronous I/O inside an `async fn`. Blocking work
  goes to `tokio::task::spawn_blocking`.
- sqlx: `query!` / `query_as!` with bound parameters. NEVER build SQL with
  `format!`. Regenerate the offline cache with `make be-prepare` when a query
  or a migration changes.
- Verify before "done", from the repository root, checking exit codes:
    cd apps/api && cargo fmt --check
    make be-lint
    make be-boundary
    make be-prepare && git add apps/api/.sqlx      # only if SQL changed
    make be-sqlx-check
    make be-test

When fixing one instance of a rule, scan sibling files for the same shape and
fix forward. When reviewing, check the diff against this list BEFORE marking
compliant.
```

---

## Decisions the plan may not quietly change

Each reversed at least once under adversarial review. The reasoning is in the spec; the point of listing them here is that each one **reads as a mistake** to a fresh pair of eyes, and "tidying" any of them reintroduces a defect that nothing in the suite will catch.

| Decision | What it looks like | Why it is not that |
|---|---|---|
| `ON DELETE RESTRICT` on both foreign keys, plus a `BEFORE UPDATE OR DELETE` trigger | `SET NULL` reads more naturally for an audit trail | PostgreSQL performs a referential action as an ordinary `UPDATE` on the child. The trigger rejects it, the whole parent `DELETE` rolls back, and account deletion becomes impossible — the exact defect class AM-361 shipped once already |
| No denormalised name or email on `role_changes` | The trail "should survive" deletion | ADR-0001 retains deleted accounts, so the joined row is unconditionally present. A second, unerasable copy of an address buys nothing |
| No last-admin guard; zero admins is legitimate | Somebody can lock everyone out | `grant-admin` only succeeds at zero admins. If nothing may ever reach zero, `grant-admin` is dead code from day two and there is no recovery path at all |
| Two-argument lock `pg_advisory_xact_lock(hashtext('platform_role'), 0)` | The single-argument form is used elsewhere | A one-argument and a two-argument advisory lock occupy different keyspaces and cannot collide even on the same hash. `part_repo::lock_merges` uses the one-argument form; `usecase::parts::allowance_spent` uses the two-argument form |
| Both the actor's role **and** the target's role are re-read under the lock, on the HTTP path | The extractor already checked the actor | The extractor ran before the handler. An admin demoted in between would still complete the mutation they had already started |
| `reason` is read from **stdin**, never from `argv` | A `--reason` flag is more ergonomic | It lands in shell history and in every `ps` listing on the box |
| No pure-domain `PlatformRole` | `ServiceCategory` exists twice, so this should too | `ServiceCategory` is duplicated because a domain **policy function** consumes it. Nothing in `domain` consumes a role. Two variants and no policy function is a second place to keep in sync |
| A rejected admin request logs the user id and the route pattern, and nothing else | A log line with the email would be more useful | `apps/api/CLAUDE.md`: a user id is not a credential and is enough to investigate |
| `grant-admin` is **not** a fourth arm on `runtime::Role` | It is a fourth thing you can type after `anakmobil` | `Role::parse` reads `args().nth(1)` and returns one of three process roles. An email and a reason have nowhere to go, and `CONTEXT.md` defines the process role as a property of the deployment, not a command |
| `role_changes` has **no** `updated_at` and no `set_updated_at` trigger | Every other table in this schema has both | The row is never updated — the trigger refuses. A column recording when it last was would be a claim the schema contradicts |

---

## Tidak boleh ada

The spec's anti-goals, carried here verbatim in substance so a task cannot quietly grow past its brief. Anything on this list appearing in a diff is a finding, not an improvement.

- **No third platform role, and no per-feature permissions.** Two values, `user` and `admin`. The community-membership concept stays entirely separate and gets its own column when it arrives.
- **No RLS and no `SET LOCAL`.** This is not multi-tenant. Isolation is per-user through query predicates and use-case authorization.
- **No general `audit_log` table.** A shape that cannot record a role change — no `from_role`, no `to_role` — is not a foundation for AM-366; that ticket designs one from real needs.
- **No `part_merge` endpoints.** `usecase::part_merge::{merge, unmerge}` stays reachable by no route. They belong to AM-88, and pulling a destructive transitive operation into an authorization ticket mixes two acceptance surfaces. Do not add a route to silence anything — `runtime` is a library and nothing warns.
- **No backoffice session TTL.** AM-84, and it is a frontend concern.
- **No rate limiting on `/admin/*`.** AM-356 owns it. Recorded as a known gap: an authenticated non-admin can probe the route repeatedly, each attempt costing one indexed read and now leaving a log line.
- **No `deleted_at`, no partial unique index, no sign-in filter.** ADR-0001 decides the policy; AM-296 builds it. Nothing in this ticket implements any of it.
- **No pure-domain `PlatformRole`.** Until a domain policy function consumes one.
- **No last-admin guard**, in the schema, the use case, or the endpoint.
- **No read endpoint for `role_changes`.** The table is written here and read by AM-366. The index exists because a foreign key needs one, not because a query does.
- **No changes to `crates/domain/`.** `make be-boundary` is not the only reason; there is simply nothing in this ticket that belongs there.
- **No `clap`.** One more command than the three that exist is not the threshold `apps/api/CLAUDE.md` sets.
- **No service-cost changes.** Every service query and every summary rollup already carries `WHERE v.owner_id = $1`, so no stranger reads a service record at all and the cost never travels. **This was audited during the design and needs no work** — it is written down so the next reader does not check it again, and so nobody "completes" AC3 by touching a query that is already correct.

---

## File structure

| File | Responsibility | Task |
|---|---|---|
| `apps/api/crates/runtime/src/adapter/postgres/build_repo.rs` | `modifications_for` gains `viewer_id`; `BuildRow` loses two fields | 1 |
| `apps/api/crates/runtime/src/usecase/builds.rs` | two call sites pass a viewer | 1 |
| `apps/api/crates/runtime/src/adapter/http/builds.rs` | `visible_cost` and `ModificationResponse::filtered` deleted | 1 |
| `apps/api/crates/runtime/tests/build_list_flow.rs` | the rewritten cost matrix | 1 |
| `apps/api/crates/runtime/migrations/<ts>_platform_role.{up,down}.sql` | enum, column, `role_changes`, indexes, trigger | 2 |
| `apps/api/CLAUDE.md` | one paragraph: this table's append-only guarantee IS a constraint, unlike `part_merges` | 2 |
| `apps/api/crates/runtime/src/adapter/postgres/user_repo.rs` | `PlatformRole`, the role reads, the lock, the audit insert, the role write, the email lookup | 3, 4 |
| `apps/api/crates/runtime/src/adapter/http/auth.rs` | the `Admin` extractor | 3 |
| `apps/api/crates/runtime/src/adapter/http/request_id.rs` | `UNMATCHED` widened to `pub(super)` | 3 |
| `apps/api/crates/runtime/src/adapter/http/admin.rs` | **new** — both admin endpoints and their error mapping | 3, 5 |
| `apps/api/crates/runtime/src/adapter/http/mod.rs` | `pub mod admin;` and two routes | 3, 5 |
| `apps/api/crates/runtime/tests/admin_flow.rs` | **new** — every HTTP admin test | 3, 5 |
| `apps/api/crates/runtime/src/usecase/roles.rs` | **new** — `set_role`, the one transaction both entrances share | 4 |
| `apps/api/crates/runtime/src/usecase/mod.rs` | `pub mod roles;` | 4 |
| `apps/api/crates/runtime/tests/role_change_flow.rs` | **new** — the use case against a real database | 4 |
| `apps/api/crates/runtime/src/lib.rs` | the `grant-admin` command branch | 6 |
| `apps/api/README.md` | the endpoint table and the "Run it" section | 3, 5, 6 |

**Why the role queries live in `user_repo.rs` and not a new `role_repo.rs`.** All six of them answer one question — who is an admin, and how did that change. Splitting them by table would be splitting by technical layer rather than by responsibility, and `user_repo.rs` is 76 lines today.

---

## Task 1: AC3 — the cost filter moves into the query

The one query that actually leaks. `modifications_for` returns every row's `cost` unfiltered and relies on its caller to null it — its own doc comment admits this. The filter then runs in `visible_cost`, in Rust, after the number has been read out of the database and into the process.

This task is first because it is the only piece of AM-355 that needs no migration, and running it first proves the whole environment (`db-up` → `be-prepare` → `be-sqlx-check` → `be-test`) works before the schema starts changing. AM-361 lost most of a day to a migration checksum drift discovered halfway through a later task.

**Files:**
- Modify: `apps/api/crates/runtime/src/adapter/postgres/build_repo.rs` — `modifications_for` (≈L360-384), `BuildRow` (≈L389-404), `page_visible`'s `SELECT` (≈L455-472)
- Modify: `apps/api/crates/runtime/src/usecase/builds.rs:77` and `:116`
- Modify: `apps/api/crates/runtime/src/adapter/http/builds.rs` — delete `visible_cost` (L113-136), delete `ModificationResponse::filtered` (L71-93), change `BuildListItem::from_detail` (L190-214), change the `list` handler's call, rewrite the unit test at L649-691
- Modify: `apps/api/crates/runtime/tests/build_list_flow.rs` — add the rewritten matrix test
- Modify: `apps/api/README.md` — the `GET /builds` row now says the filter is in the query
- Regenerate: `apps/api/.sqlx`

**Interfaces:**
- Produces: `build_repo::modifications_for(conn: &mut PgConnection, build_ids: &[Uuid], viewer_id: Uuid) -> Result<Vec<Modification>, sqlx::Error>`
- Produces: `BuildRow` **without** `owner_id` and `cost_visibility`
- Produces: `BuildListItem::from_detail(detail: BuildDetail) -> Self` (one argument, not two)
- Consumes: nothing from a later task

**TDD: no** — and the reason is specific rather than a shrug. The behaviour does not change; only its location does. A red-first test is impossible because the existing code already produces the right answer, so any test written first would pass. What holds this task honest instead is that `tests/build_list_flow.rs::a_stranger_sees_no_modification_costs_when_the_car_keeps_them_private` and `tests/build_flow.rs:290` and `:684` must stay green **throughout** — those are the normal-path regression per `[[regression-safety-rule]]` — and the five assertions the deleted unit test carried are rewritten as integration assertions in the same task.

**Big O and access pattern.** `modifications_for` gains two joins, both to at most one row: `modifications.build_id → builds.id` (primary key) and `builds.vehicle_id → vehicles.id` (primary key, and `builds.vehicle_id` is UNIQUE). So the join multiplies nothing and the result set size is unchanged. `O(M log B)` for `M` modifications across the page, which is `O(M)` in practice; still **one query for any number of builds**, still no N+1. Removing two columns from `page_visible` changes nothing about its plan.

**Minimality check.** No new function, no new type, no new dependency. The change is net negative in lines: one `CASE` in SQL replaces a Rust function, an impl block, and a parameter that has to be threaded through two layers.

### Steps

- [ ] **Step 1: change the query**

In `build_repo.rs`, replace `modifications_for` entirely:

```rust
/// Every modification on these builds, with `cost` already filtered for this
/// viewer.
///
/// The filter is in the query, not in the caller. It used to be a Rust
/// function at the boundary — `visible_cost` in `adapter/http/builds.rs` —
/// which meant the number was read out of the database and into the process
/// before anybody decided whether the caller was allowed to see it. A cost
/// that reaches the wire cannot be recalled, and the shortest path to never
/// sending one is never selecting it.
///
/// The permitted values are listed rather than written `<> 'private'`, and
/// the difference is what happens when the enum grows: this repository
/// extends closed sets with `ALTER TYPE … ADD VALUE`, so `<>` would make
/// every cost on the platform visible under a new variant. Naming them fails
/// CLOSED. Note the cost of moving this into SQL: the Rust `match` this
/// replaces made the compiler report the drift, and no compiler will report
/// it here. Whoever adds a variant to `visibility` must come and read this.
///
/// The owner's own view is unconditional — a `private` setting never hides a
/// person's own numbers from the person who paid them.
///
/// # The caller still owns the build-level authorisation check
///
/// This takes whatever build ids it is given and its `WHERE` clause does not
/// reach `vehicles.owner_id`. What the viewer changes is the `cost` column,
/// not which rows come back. Pass only ids the caller may see.
///
/// Complexity: `O(M log B)` for `M` modifications — both joins are to at most
/// one row (`builds.id` is a primary key and `builds.vehicle_id` is UNIQUE),
/// so the join multiplies nothing. One query for any number of builds.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn modifications_for(
    conn: &mut PgConnection,
    build_ids: &[Uuid],
    viewer_id: Uuid,
) -> Result<Vec<Modification>, sqlx::Error> {
    sqlx::query_as!(
        Modification,
        r#"
        SELECT m.id, m.build_id, m.part_id, m.install_date, m.mileage_km,
               CASE WHEN v.owner_id = $2
                      OR v.cost_visibility IN ('community', 'public')
                    THEN m.cost
               END AS "cost?: BigDecimal",
               m.garage_name, m.notes, m.removed_at
        FROM modifications m
        JOIN builds b   ON b.id = m.build_id
        JOIN vehicles v ON v.id = b.vehicle_id
        WHERE m.build_id = ANY($1)
        ORDER BY m.build_id, m.install_date DESC NULLS LAST, m.id
        "#,
        build_ids,
        viewer_id,
    )
    .fetch_all(conn)
    .await
}
```

`BigDecimal` is already in scope in this file (`use sqlx::types::BigDecimal;` at the top). The `"cost?: BigDecimal"` form forces both the nullability and the type rather than relying on sqlx's inference for a `CASE` expression — if it compiles without the override, keep the override anyway so the type cannot drift when the expression changes.

- [ ] **Step 2: drop the two fields the filter used to need**

In `build_repo.rs`, `BuildRow` loses `owner_id` and `cost_visibility`, and `page_visible`'s `SELECT` loses the two lines that produced them:

```rust
pub struct BuildRow {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    pub notes: Option<String>,
    pub visibility: Visibility,
    pub variant_id: Option<Uuid>,
    pub nickname: Option<String>,
    pub described_as: Option<String>,
}
```

```sql
        SELECT
            b.id,
            b.vehicle_id,
            b.notes,
            b.visibility AS "visibility: Visibility",
            v.variant_id,
            v.nickname,
            v.described_as
        FROM builds b
        JOIN vehicles v ON v.id = b.vehicle_id
        WHERE (b.visibility <> 'private' OR v.owner_id = $1)
          AND ($2::uuid IS NULL OR b.id < $2)
        ORDER BY b.id DESC
        LIMIT $3
```

Note the `WHERE` clause still reads `v.owner_id` — that is the build-visibility filter and it stays. What goes is only the two selected columns.

**Do this rather than leaving them.** `BuildRow` is a `pub` struct in a library crate, so an unread field produces no compiler warning, and `cost_visibility`'s doc comment says in as many words that it exists "so the response mapping can null out a modification's `cost` without a second query" — a mechanism that will no longer exist. AM-361's ledger records three separate findings where a comment describing a mechanism that had moved sent the next reader to the wrong place.

- [ ] **Step 3: thread the viewer through the two call sites**

`usecase/builds.rs:77`, inside `for_vehicle`:

```rust
    let modifications = build_repo::modifications_for(&mut conn, &[build.id], owner_id).await?;
```

`usecase/builds.rs:116`, inside `page`:

```rust
    let modifications = build_repo::modifications_for(&mut conn, &ids, viewer_id).await?;
```

`for_vehicle` resolves the build with `find_build_for_vehicle(&mut conn, owner_id, vehicle_id)`, so its caller is always the owner and passing `owner_id` is what keeps the existing "an owner sees their own cost" behaviour exactly as it was.

- [ ] **Step 4: delete the Rust filter**

In `adapter/http/builds.rs`, delete `fn visible_cost` (L113-136) and the whole `impl ModificationResponse { fn filtered … }` block (L71-93). Then `BuildListItem::from_detail` loses its second parameter:

```rust
impl BuildListItem {
    /// The cost on each modification is already filtered — see
    /// [`crate::adapter::postgres::build_repo::modifications_for`], which
    /// nulls it in the query rather than here. This mapping used to take a
    /// `viewer_id` and apply the filter itself; a filter at the boundary is a
    /// filter somebody has to remember to call.
    fn from_detail(detail: BuildDetail) -> Self {
        let BuildDetail {
            build,
            modifications,
            photos,
        } = detail;

        Self {
            id: build.id,
            vehicle_id: build.vehicle_id,
            notes: build.notes,
            visibility: build.visibility,
            nickname: build.nickname,
            described_as: build.described_as,
            modifications: modifications.into_iter().map(Into::into).collect(),
            photos: photos.into_iter().map(Into::into).collect(),
        }
    }
}
```

Update the `list` handler's call to `BuildListItem::from_detail(detail)` — the `viewer_id` it used to pass is now consumed one layer down, by `usecase::builds::page`, which already had it.

Fix the imports that go unused (`sqlx::types::BigDecimal` is still needed by `ModificationRequest::check`; `Visibility` is still needed by `BuildResponse`). Let `cargo fmt --check` and `make be-lint` tell you rather than guessing.

- [ ] **Step 5: rewrite the unit test as an integration test**

Delete `a_stranger_never_sees_a_private_cost_and_the_owner_always_sees_their_own` from `adapter/http/builds.rs` (L649-691) — the function it tested no longer exists. Its five assertions become one `#[tokio::test]` in `tests/build_list_flow.rs`, appended after the existing cost test:

```rust
#[tokio::test]
async fn the_cost_matrix_holds_for_every_setting_and_both_viewers() {
    // The replacement for the unit test that pinned `visible_cost`. Five
    // mutations of that function once survived the whole suite, including
    // deleting the filter from the call path outright, which is why it
    // existed. The filter now lives in `modifications_for`'s CASE expression,
    // so the same matrix has to be asserted through a real query.
    let (app, _pool) = app!();
    let owner = a_signed_in_person(&app).await;
    let stranger = a_signed_in_person(&app).await;

    // One car per cost_visibility setting, each with a priced modification
    // and a community-visible build so the stranger can see the build at all.
    let mut cars = Vec::new();
    for setting in ["private", "community", "public"] {
        let car = a_car(&app, &owner).await;

        let updated = send(
            &app,
            "PUT",
            &format!("/vehicles/{car}"),
            Some(json!({ "cost_visibility": setting })),
            Some(&owner),
        )
        .await;
        assert_eq!(updated.status(), StatusCode::NO_CONTENT, "setting {setting}");

        let saved = send(
            &app,
            "PUT",
            &format!("/vehicles/{car}/build"),
            Some(json!({ "visibility": "community" })),
            Some(&owner),
        )
        .await;
        assert_eq!(saved.status(), StatusCode::NO_CONTENT);

        let part = an_existing_part(&app, &owner, &format!("Matrix {}", Uuid::now_v7())).await;
        let added = send(
            &app,
            "POST",
            &format!("/vehicles/{car}/build/modifications"),
            Some(json!({ "part_id": part, "cost": "1200000" })),
            Some(&owner),
        )
        .await;
        assert_eq!(added.status(), StatusCode::CREATED);

        cars.push((setting, car));
    }

    // The owner check comes first and unconditionally: a private setting must
    // never hide a person's own numbers from themselves.
    let owner_items = all_visible_builds(&app, &owner).await;
    for (setting, car) in &cars {
        let build = owner_items
            .iter()
            .find(|item| item["vehicle_id"] == *car)
            .unwrap_or_else(|| panic!("the owner's own {setting} build is missing"));
        assert_eq!(
            build["modifications"][0]["cost"], "1200000.00",
            "cost_visibility={setting} hid an owner's own cost from themselves"
        );
    }

    let stranger_items = all_visible_builds(&app, &stranger).await;
    for (setting, car) in &cars {
        let build = stranger_items
            .iter()
            .find(|item| item["vehicle_id"] == *car)
            .unwrap_or_else(|| panic!("the community {setting} build is missing"));
        let mods = build["modifications"].as_array().expect("a list of modifications");
        assert_eq!(
            mods.len(), 1,
            "{setting}: the modification itself must survive — only its cost is hidden"
        );
        let cost = &mods[0]["cost"];
        if *setting == "private" {
            assert!(cost.is_null(), "a private cost reached a stranger");
        } else {
            assert_eq!(
                *cost, "1200000.00",
                "{setting} must be visible to any signed-in caller today"
            );
        }
    }
}

#[tokio::test]
async fn a_modification_with_no_cost_stays_null_rather_than_becoming_a_zero() {
    // The sixth assertion the deleted unit test carried. "No cost recorded"
    // and "cost hidden from you" are both null on the wire, and that is
    // deliberate — but a filter that turned an absent cost into "0.00" would
    // be a number the owner never typed.
    let (app, _pool) = app!();
    let owner = a_signed_in_person(&app).await;
    let car = a_car(&app, &owner).await;
    let part = an_existing_part(&app, &owner, &format!("Unpriced {}", Uuid::now_v7())).await;

    let added = send(
        &app,
        "POST",
        &format!("/vehicles/{car}/build/modifications"),
        Some(json!({ "part_id": part })),
        Some(&owner),
    )
    .await;
    assert_eq!(added.status(), StatusCode::CREATED);

    let items = all_visible_builds(&app, &owner).await;
    let build = items
        .iter()
        .find(|item| item["vehicle_id"] == car)
        .expect("the owner's own build is missing");
    assert!(build["modifications"][0]["cost"].is_null());
}
```

Check the helper names against the top of `tests/build_list_flow.rs` before writing — `a_car`, `an_existing_part`, `all_visible_builds`, `a_signed_in_person`, `send`, `json` all exist there, but confirm each signature rather than assuming. If `a_car` does not accept a `cost_visibility`, set it with the `PUT /vehicles/{id}` shown above; `tests/build_flow.rs:867` (`cost_visibility_persists_from_creation`) shows the field name on the wire.

- [ ] **Step 6: regenerate the offline cache and run the gate**

```bash
make db-up
make be-prepare && git add apps/api/.sqlx
make be-sqlx-check
cd apps/api && cargo fmt --check
```
then from the repository root:
```bash
make be-lint
make be-boundary
make be-test
```

Expected: every command `EXIT=0`. `make be-test` runs 11 suites; report the counts, not a summary.

- [ ] **Step 7: prove the guards can fail**

Three sabotages, each run and then reverted. Record which test reddened for each.

| Sabotage | Test that must die, and only it |
|---|---|
| Change the `CASE` to `THEN m.cost` with no condition (always visible) | `the_cost_matrix_holds_for_every_setting_and_both_viewers` (the private/stranger branch) **and** `a_stranger_sees_no_modification_costs_when_the_car_keeps_them_private` |
| Change `v.owner_id = $2` to `FALSE` (owner loses their own private cost) | `the_cost_matrix_holds_for_every_setting_and_both_viewers` (the owner loop) **and** `build_flow.rs:684` |
| Change `IN ('community', 'public')` to `IN ('public')` | `the_cost_matrix_holds_for_every_setting_and_both_viewers` (the `community` case) |

If any sabotage leaves the suite green, the assertion it was meant to break cannot fail and must be strengthened before this task is done.

- [ ] **Step 8: update the README row**

In `apps/api/README.md`, the `GET /builds` row currently reads "cost hidden per `cost_visibility` unless the caller owns the car". Change the tail to say the filter is applied in the query rather than at the boundary:

```
| `GET /builds` | `200` | own builds plus `community`/`public` ones, cursor paged; cost is nulled **in the query** per `cost_visibility` unless the caller owns the car |
```

### Acceptance criteria

1. `build_repo::modifications_for` takes a `viewer_id` and no caller of it applies a further cost filter. `grep -rn visible_cost apps/api/` returns nothing.
2. All three sabotages in Step 7 redden exactly the named tests and nothing else, and the suite is green after each revert.
3. `tests/build_list_flow.rs::a_stranger_sees_no_modification_costs_when_the_car_keeps_them_private`, `tests/build_flow.rs:290`, and `tests/build_flow.rs:684` are green and unmodified — the normal-path regression.
4. `make be-sqlx-check` is `EXIT=0` against a freshly prepared cache, and `apps/api/.sqlx` is staged in the same commit as the query change.
5. `BuildRow` has no `owner_id` and no `cost_visibility` field, and `page_visible` no longer selects them.

**Block B applies to this task.**

---

## Task 2: the migration — `platform_role`, the column, and `role_changes`

**Files:**
- Create: `apps/api/crates/runtime/migrations/<timestamp>_platform_role.up.sql`
- Create: `apps/api/crates/runtime/migrations/<timestamp>_platform_role.down.sql`
- Modify: `apps/api/CLAUDE.md` — one paragraph beside the existing `part_merges` section

**Interfaces:**
- Produces: the Postgres type `platform_role` with values `'user'` and `'admin'`; `users.platform_role`; the table `role_changes`; the trigger function `role_changes_append_only()`
- Consumes: nothing

**TDD: no** — a migration has no unit to test first. Verified by running it: apply → revert → re-apply, plus the SQL-level assertions in Step 3, each of which is executed and its output recorded.

**Big O and access pattern.** `role_changes` is read by `target_user_id` — `role_changes_target_idx (target_user_id, created_at DESC, id)` serves "this person's role history, newest first" at `O(log n + k)` with no sort. **Nothing in AM-355 reads it**; the read endpoint belongs to AM-366. The index earns its place anyway as this repository's mandatory foreign-key index — PostgreSQL indexes primary keys and unique constraints but never a foreign key, and with `ON DELETE RESTRICT` a parent delete does a real lookup here. The trailing `created_at DESC, id` are the speculative part: two extra columns on an index that has to exist regardless, versus a migration later. `role_changes_actor_idx` is partial because bootstrap rows have no actor and nothing queries for them.

**Minimality check.** One migration pair, no `updated_at`, no status column, no view, no seed. The enum and the column ship together because they cannot be used apart.

### Steps

- [ ] **Step 1: create the pair**

```bash
cd apps/api/crates/runtime && sqlx migrate add -r platform_role
```

Always `-r`. Writing the down migration is what forces you to notice that the type cannot be dropped before the column that uses it.

- [ ] **Step 2: write the up migration**

```sql
-- Two platform roles, and an append-only record of every change between them.
--
-- "Platform role" is one of three unrelated senses of the word "role" in this
-- codebase and they are never stored in the same column — see CONTEXT.md.
-- `runtime::Role` is the PROCESS role (web | worker | migrate), a property of
-- the deployment rather than of any person. Community membership will bring a
-- third sense, confined to one community, which grants nothing platform-wide.

CREATE TYPE platform_role AS ENUM ('user', 'admin');

-- Defaults to `user`, and no sign-up path writes this column, so the admin
-- role cannot be reached through registration.
ALTER TABLE users
    ADD COLUMN platform_role platform_role NOT NULL DEFAULT 'user';

COMMENT ON COLUMN users.platform_role IS
    'What this account may do across the whole platform. Read fresh on every admin request; never cached in a session.';

-- Every promotion, demotion, and bootstrap.
--
-- Append-only, and enforced by the trigger below rather than by a comment.
-- `part_merges` claims append-only in a comment while an UPDATE was verified
-- to rewrite its history and a DELETE to remove it outright. A comment is not
-- a constraint. Here the guarantee is in the schema from the first migration.
CREATE TABLE role_changes (
    id             UUID PRIMARY KEY,

    -- NULL for a bootstrap: `anakmobil grant-admin` has no signed-in human
    -- behind it.
    --
    -- RESTRICT rather than SET NULL, and the difference is the whole reason
    -- this table can carry foreign keys at all. PostgreSQL performs a
    -- referential action as an ordinary UPDATE or DELETE on the CHILD row,
    -- which the append-only trigger below rejects — so SET NULL would make
    -- every parent DELETE fail and account deletion impossible. That is the
    -- exact defect class this project has already shipped once. RESTRICT never
    -- writes to the child, so the trigger never sees it.
    --
    -- The usual convention for an audit table is to drop the keys entirely.
    -- What makes RESTRICT strictly better here is ADR-0001: deleted accounts
    -- are retained rather than erased, so the row a key points at is
    -- unconditionally present. Identity is therefore a join, and this table
    -- stores no copy of anybody's email. A future hard-erasure path meets a
    -- refusal here and has to make a conscious decision instead of silently
    -- orphaning the trail.
    actor_id       UUID          REFERENCES users (id) ON DELETE RESTRICT,
    target_user_id UUID NOT NULL REFERENCES users (id) ON DELETE RESTRICT,

    from_role      platform_role NOT NULL,
    to_role        platform_role NOT NULL,

    -- Why, in the admin's or operator's own words. Never logged, never
    -- returned in a response — the response answers what changed, not who
    -- anybody is or what they were thinking.
    reason         TEXT          NOT NULL,

    created_at     TIMESTAMPTZ   NOT NULL DEFAULT now(),

    -- A row claiming a change that did not happen is a lie about the past.
    -- The use case has an explicit no-op branch so this never surfaces to a
    -- client as a 500; the constraint is the backstop for a path that forgets.
    CONSTRAINT role_changes_real_change CHECK (from_role <> to_role)
);

-- No `updated_at` and no `set_updated_at` trigger, deliberately, against the
-- convention every other table here follows. A row is never updated — the
-- trigger below refuses — so a column recording when it last was would be a
-- claim the schema contradicts.

-- The only query that will matter: this person's role history, newest first.
-- It is also the mandatory foreign-key index — PostgreSQL indexes primary keys
-- and unique constraints and never a foreign key, and RESTRICT turns every
-- parent delete into a real lookup here.
CREATE INDEX role_changes_target_idx
    ON role_changes (target_user_id, created_at DESC, id);

-- Partial: bootstrap rows have no actor and there is no query for them.
CREATE INDEX role_changes_actor_idx
    ON role_changes (actor_id)
    WHERE actor_id IS NOT NULL;

CREATE OR REPLACE FUNCTION role_changes_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'role_changes is append-only: % is not permitted', TG_OP;
END;
$$ LANGUAGE plpgsql;

-- BEFORE, not AFTER: the row must never be written at all, and an AFTER
-- trigger raising would still have done the work first. UPDATE and DELETE
-- together, because removing history and rewriting it are the same defect.
CREATE TRIGGER role_changes_append_only
    BEFORE UPDATE OR DELETE ON role_changes
    FOR EACH ROW EXECUTE FUNCTION role_changes_append_only();

COMMENT ON TABLE role_changes IS
    'Append-only, enforced by the role_changes_append_only trigger rather than by this comment. Identity is a join to users; no name or email is copied here.';
```

- [ ] **Step 3: write the down migration**

```sql
-- The table before the function: dropping the table drops its trigger, and
-- dropping the function first would fail while the trigger still references it.
DROP TABLE IF EXISTS role_changes;
DROP FUNCTION IF EXISTS role_changes_append_only();

-- The column before the type. `DROP TYPE platform_role` while `users` still
-- has a column of that type fails with "cannot drop type platform_role because
-- other objects depend on it", which leaves the revert half-applied.
ALTER TABLE users DROP COLUMN IF EXISTS platform_role;
DROP TYPE IF EXISTS platform_role;
```

- [ ] **Step 4: apply, revert, re-apply**

```bash
make db-up
make be-migrate
cd apps/api/crates/runtime && sqlx migrate revert
cd /Volumes/Project/anak-mobil && make be-migrate
```

Expected: all three succeed. After the revert, prove no orphan type is left:

```bash
docker compose exec -T postgres psql -U postgres -d anakmobil -c \
  "SELECT typname FROM pg_type WHERE typname = 'platform_role'"
```
must return zero rows **while reverted**. Re-apply before continuing.

- [ ] **Step 5: prove the trigger and the foreign keys, in both directions**

Run each of these and record the actual output. A constraint proven only in the rejecting direction can be a constraint that rejects everything.

```sql
-- Fixtures.
INSERT INTO users (id, email, password_hash)
VALUES ('00000000-0000-7000-8000-000000000001', 'trigger-actor@example.com', 'x'),
       ('00000000-0000-7000-8000-000000000002', 'trigger-target@example.com', 'x'),
       ('00000000-0000-7000-8000-000000000003', 'no-history@example.com', 'x');

-- 1. An ordinary insert is ACCEPTED.
INSERT INTO role_changes (id, actor_id, target_user_id, from_role, to_role, reason)
VALUES ('00000000-0000-7000-8000-0000000000aa',
        '00000000-0000-7000-8000-000000000001',
        '00000000-0000-7000-8000-000000000002',
        'user', 'admin', 'proving the happy path');

-- 2. A bootstrap row with NO actor is ACCEPTED.
INSERT INTO role_changes (id, actor_id, target_user_id, from_role, to_role, reason)
VALUES ('00000000-0000-7000-8000-0000000000bb', NULL,
        '00000000-0000-7000-8000-000000000002',
        'admin', 'user', 'proving actor_id is nullable');

-- 3. from_role = to_role is REFUSED — expect 23514 role_changes_real_change.
INSERT INTO role_changes (id, actor_id, target_user_id, from_role, to_role, reason)
VALUES ('00000000-0000-7000-8000-0000000000cc', NULL,
        '00000000-0000-7000-8000-000000000002',
        'user', 'user', 'should be refused');

-- 4. UPDATE is REFUSED by the trigger — expect the append-only message.
UPDATE role_changes SET reason = 'rewritten'
WHERE id = '00000000-0000-7000-8000-0000000000aa';

-- 5. DELETE is REFUSED by the trigger — expect the append-only message.
DELETE FROM role_changes WHERE id = '00000000-0000-7000-8000-0000000000aa';

-- 6. Deleting a user WITH history is REFUSED by the FOREIGN KEY, not by the
--    trigger — expect 23503 foreign_key_violation. This is the assertion that
--    distinguishes RESTRICT from SET NULL: the child row is never touched, so
--    the message must name the constraint and not the trigger.
DELETE FROM users WHERE id = '00000000-0000-7000-8000-000000000002';

-- 7. Deleting the ACTOR of a row is REFUSED the same way — expect 23503.
DELETE FROM users WHERE id = '00000000-0000-7000-8000-000000000001';

-- 8. Deleting a user with NO history SUCCEEDS. Without this, 6 and 7 are also
--    satisfied by a schema that refuses every delete.
DELETE FROM users WHERE id = '00000000-0000-7000-8000-000000000003';
```

Then clean up: `make db-drop` (it drops, recreates, and re-migrates), because rows 1 and 2 cannot be deleted — which is the whole point of the table.

Record the SQLSTATE of each refusal. **6 and 7 must be `23503`, not the trigger's message.** If either reports the append-only exception, the foreign key is `SET NULL` and account deletion is broken; stop and raise it as a `structural` finding before any later task builds on the schema.

- [ ] **Step 6: record the difference from `part_merges` in `apps/api/CLAUDE.md`**

The file has a section headed "`part_merges` is append-only by discipline, not by constraint" which names a `BEFORE UPDATE` trigger as what would enforce it. Add a short paragraph immediately after it:

```markdown
`role_changes` is the version that does. Its migration ships a
`BEFORE UPDATE OR DELETE` trigger that raises, so the guarantee is in the
schema rather than in a comment, and both foreign keys are `ON DELETE
RESTRICT` rather than `SET NULL` — a referential action is an ordinary write
to the child row, and the trigger would reject it, taking the whole parent
`DELETE` down with it. `part_merges` predates that reasoning and its migration
is merged, so its checksum is frozen; the same trigger would fit it whenever a
new migration is worth writing.
```

- [ ] **Step 7: run the gate**

The migration adds no Rust, but it moves the schema, and the sqlx cache is generated from the schema:

```bash
make be-prepare && git add apps/api/.sqlx
make be-sqlx-check
make be-test
```

Expected: `EXIT=0` on all three. `make be-prepare` may produce no diff at all if no query changed — that is correct, and `be-sqlx-check` passing is the evidence, not the diff.

### Acceptance criteria

1. Apply → revert → re-apply all succeed, and `pg_type` holds no `platform_role` while reverted.
2. Assertions 1 and 2 in Step 5 are accepted; 3, 4 and 5 are refused; **6 and 7 fail with SQLSTATE `23503`** and not with the trigger's message; 8 succeeds.
3. `role_changes` has no `updated_at` column and no `set_updated_at` trigger.
4. `\d role_changes` shows both indexes, with `role_changes_actor_idx` partial.
5. `make be-sqlx-check` and `make be-test` are `EXIT=0`.

**Block B applies to this task** (its verification chain, not its Rust rules).

---

## Task 3: `PlatformRole`, the role reads, the `Admin` extractor, and the admin vehicle read

The extractor and its first route ship together, and that is deliberate: an extractor with no route cannot be tested through anything, and "the check is in the type" is a claim about routing that only a routed handler can prove.

**Files:**
- Modify: `apps/api/crates/runtime/src/adapter/postgres/user_repo.rs` — `PlatformRole`, `platform_role_of`
- Modify: `apps/api/crates/runtime/src/adapter/http/auth.rs` — the `Admin` extractor
- Modify: `apps/api/crates/runtime/src/adapter/http/request_id.rs` — `UNMATCHED` widened to `pub(super)`
- Create: `apps/api/crates/runtime/src/adapter/http/admin.rs`
- Modify: `apps/api/crates/runtime/src/adapter/http/mod.rs` — `pub mod admin;` and one route
- Create: `apps/api/crates/runtime/tests/admin_flow.rs`
- Modify: `apps/api/README.md` — one endpoint row
- Regenerate: `apps/api/.sqlx`

**Interfaces:**
- Consumes: Task 2's `platform_role` type and `users.platform_role` column
- Produces: `user_repo::PlatformRole` — `#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]`, `#[sqlx(type_name = "platform_role", rename_all = "snake_case")]`, `#[serde(rename_all = "snake_case")]`, variants `User` and `Admin`
- Produces: `user_repo::platform_role_of(conn: &mut PgConnection, id: Uuid) -> Result<Option<PlatformRole>, sqlx::Error>`
- Produces: `adapter::http::auth::Admin { pub user_id: Uuid }` with `impl FromRequestParts<AppState>`
- Produces: `adapter::http::admin::to_api_error(err: GarageError) -> ApiError`
- Produces: the route `GET /admin/users/{id}/vehicles`

**TDD: no** — verified by running it. The extractor's behaviour is a routing property and the endpoint's is a serialisation property; both are pinned by integration tests written immediately after, including the AC4 test the spec singles out and a normal-path regression that an ordinary `Authenticated` route is unaffected.

**Big O and access pattern.** `platform_role_of` is a primary-key lookup: `O(log U)`, one row, one extra round trip **on admin routes only**. Ordinary routes keep `Authenticated`, which touches Redis alone and pays nothing. The admin vehicle read reuses `usecase::garage::list`, which is one query with four `LEFT JOIN`s to catalog tables — `O(V log C)` for `V` cars, and it is already the list endpoint's own query, so there is no new plan to reason about.

**Minimality check.** No new repository, no new use case, no new response type. `usecase::garage::list(pool, owner_id)` already answers "every car this person owns" and takes the owner as a parameter, so the admin endpoint calls it with the target's id and maps through the existing `VehicleResponse`. The endpoint's whole body is four lines.

### Steps

- [ ] **Step 1: the enum and the read**

Append to `adapter/postgres/user_repo.rs`:

```rust
/// What an account may do across the whole platform.
///
/// Two values, and `CONTEXT.md` records why the name is this long: `Role` in
/// this crate is the PROCESS role (`web` | `worker` | `migrate`), and
/// community membership will bring a third sense of the word that grants
/// nothing platform-wide. Three unrelated concepts, three names.
///
/// There is deliberately no copy of this in the domain crate. `ServiceCategory`
/// exists twice because a domain policy function consumes it; nothing in
/// `domain` consumes a role, and two variants with no policy function is not a
/// domain model but a second place to keep in sync. When a policy function
/// earns it, the split is additive.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize,
)]
#[sqlx(type_name = "platform_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PlatformRole {
    User,
    Admin,
}

/// This account's platform role, read from the source.
///
/// `None` means there is no such account — which happens when a Redis session
/// outlives the row it points at. Every caller treats that as a refusal; see
/// [`crate::adapter::http::auth::Admin`].
///
/// Nothing caches this. That is what satisfies AC1: a revoked role is refused
/// on the next admin request without waiting for a new token, because the
/// token never carried the role in the first place.
///
/// Complexity: `O(log n)` — a primary-key lookup, one row.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails. A failure is never turned
/// into a default role by any caller.
pub async fn platform_role_of(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<PlatformRole>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT platform_role AS "platform_role: PlatformRole" FROM users WHERE id = $1"#,
        id
    )
    .fetch_optional(conn)
    .await
}
```

- [ ] **Step 2: widen `UNMATCHED`**

In `adapter/http/request_id.rs`, change one word:

```rust
/// What a request that matched no route is called in the log.
pub(super) const UNMATCHED: &str = "unmatched";
```

One name for one concept: the extractor logs a route label for the same reason the middleware does, and a second `"unmatched"` literal in another file is the drift this avoids.

- [ ] **Step 3: the extractor**

Append to `adapter/http/auth.rs`, after `Authenticated`:

```rust
/// A caller proven to be a platform admin.
///
/// The same claim [`Authenticated`] makes, one level up: a handler that takes
/// this parameter cannot be routed without an admin behind it, so the check is
/// in the type rather than in a line of code somebody has to remember to write
/// first. With one admin endpoint today that looks like ceremony; with AM-366's
/// curation queues it is the only thing that scales, and adding it later means
/// auditing every route written in between.
///
/// The role is read from Postgres on **every** admin request. Nothing caches
/// it, so a demoted admin's next admin request is refused without waiting for
/// a new token — while their ordinary requests are unaffected, because a
/// demoted admin is still a user and their garage still belongs to them.
///
/// **Failure is closed.** A database error is a 500; it is never "assume
/// `user`" and never "assume `admin`". A missing account — a session that
/// outlived its row — is a refusal, not an assumption.
///
/// **403, not 404.** AM-84's AC2 asks for a rejection the person can
/// understand, and hiding an endpoint's existence from an already
/// authenticated account buys nothing.
#[derive(Debug, Clone, Copy)]
pub struct Admin {
    pub user_id: Uuid,
}

impl FromRequestParts<AppState> for Admin {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let Authenticated { user_id } = Authenticated::from_request_parts(parts, state).await?;

        let mut conn = state
            .pool
            .acquire()
            .await
            .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?;

        let role = user_repo::platform_role_of(&mut conn, user_id)
            .await
            .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?;

        match role {
            Some(PlatformRole::Admin) => Ok(Self { user_id }),
            // Enumerated rather than `_`, so a third variant is a compile
            // error somebody has to decide about. It would fail closed either
            // way; what the enumeration buys is that the decision is visible.
            Some(PlatformRole::User) | None => {
                // Without this line, probing for admin endpoints is
                // indistinguishable from silence on precisely the routes an
                // attacker would probe. The user id and the matched route
                // pattern, and nothing else — no email, no path, no body.
                // A user id is not a credential and is enough to investigate.
                tracing::warn!(
                    %user_id,
                    route = parts
                        .extensions
                        .get::<MatchedPath>()
                        .map_or(super::request_id::UNMATCHED, MatchedPath::as_str),
                    "admin route refused"
                );
                Err(ApiError::forbidden())
            }
        }
    }
}
```

Add to the imports at the top of `auth.rs`:

```rust
use axum::extract::MatchedPath;

use crate::adapter::postgres::user_repo::{self, PlatformRole};
```

`MatchedPath` is populated by the router during routing, so it is always present by the time an extractor on a registered route runs; the fallback cannot fire and exists so the expression has no `unwrap`.

- [ ] **Step 4: the admin module and the vehicle read**

Create `adapter/http/admin.rs`:

```rust
//! The admin surface.
//!
//! Every handler here takes [`Admin`] rather than [`Authenticated`], which is
//! what makes the authorisation check unforgettable: a route wired to a
//! handler that forgot it does not compile into this module in the first
//! place.
//!
//! # Why an admin's view of a car is the ordinary view
//!
//! [`VehicleResponse`] has no field for a plate, a VIN, or a purchase price —
//! not a skipped field, not an `Option` that happens to be `None`, no field.
//! So redaction here is structural rather than a filter somebody remembered to
//! apply, and the reason `find_private` is never called from this module is
//! that there is nowhere to put what it returns.
//!
//! The summary is left absent for the same reason. `ListSummaryResponse`
//! carries `total_cost`, which is service spend, which `CONTEXT.md` names as
//! private vehicle data — "never leaves the server for anyone who should not
//! see it, including an admin". `GET /vehicles` fills it in because that
//! endpoint answers to the owner. This one does not.

use axum::extract::{Path, State};
use uuid::Uuid;

use crate::adapter::http::auth::Admin;
use crate::adapter::http::vehicles::VehicleResponse;
use crate::platform::state::AppState;
use crate::shared::errors::ApiError;
use crate::shared::response::ApiResponse;
use crate::usecase::garage::{self, GarageError};

/// `GET /admin/users/{id}/vehicles`
///
/// An admin listing one person's cars. No plate, no VIN, no purchase price,
/// and no spend.
///
/// An unknown `id` answers with an empty list rather than a `404`: `users` is
/// not queried, and an admin distinguishing "no cars" from "no such person"
/// through this endpoint would make it a user-id oracle for no gain. The
/// non-admin case never reaches here at all — the extractor refuses first.
///
/// # Errors
///
/// A storage failure.
pub async fn vehicles(
    State(state): State<AppState>,
    // Unused by name and load-bearing by position: constructing it IS the
    // authorisation check, and it runs before this body does.
    _caller: Admin,
    Path(id): Path<Uuid>,
) -> Result<ApiResponse<Vec<VehicleResponse>>, ApiError> {
    let vehicles = garage::list(&state.pool, id).await.map_err(to_api_error)?;

    Ok(ApiResponse::ok(
        vehicles.into_iter().map(VehicleResponse::from).collect(),
    ))
}

/// The single failure-to-response mapping for this module.
///
/// Exhaustive with no `_` arm, so a new [`GarageError`] variant makes this
/// match non-exhaustive and the build fails until somebody decides what it
/// means here. Three of the four arms are unreachable from `garage::list` —
/// it neither resolves an id nor touches the catalog — and they are mapped
/// anyway rather than collapsed into a catch-all, because that is what keeps
/// the compiler reporting the drift.
fn to_api_error(err: GarageError) -> ApiError {
    match err {
        GarageError::NotFound | GarageError::ReorderMismatch => ApiError::not_found(),
        GarageError::UnknownVariant => ApiError::validation(serde_json::json!({
            "variant_id": "Varian tidak ada di katalog."
        })),
        GarageError::Database(inner) => ApiError::internal(anyhow::anyhow!(inner)),
    }
}
```

That is `vehicles.rs:358` copied, with the comment rewritten for this module's reachability. Do not simplify it to the one arm that can actually fire.

- [ ] **Step 5: register the module and the route**

In `adapter/http/mod.rs`, add `pub mod admin;` to the module list (alphabetically first), and one route inside `router`:

```rust
        .route("/admin/users/{id}/vehicles", get(admin::vehicles))
```

Place it with the other `get` routes. There is no literal-versus-parameter conflict here — `{id}` sits between two literals.

- [ ] **Step 6: the test harness and the tests**

Create `tests/admin_flow.rs`. **Copy the `app!` macro, `a_peer`, `send`, and `json` from `tests/build_list_flow.rs` verbatim** — the version that returns `(app, pool)` and asserts on `AM_SKIP_INTEGRATION`. Do not copy from `tests/part_merge_flow.rs`; its `pool!` macro still returns silently when `DATABASE_URL` is unset, which is the bug PR #18 closed everywhere else.

Then the helpers this file needs:

```rust
/// Registers a fresh person, returning their access token and their user id.
///
/// The id comes from the database rather than from the response, because
/// registration answers with tokens and says nothing about who the account is.
async fn a_person(app: &axum::Router, pool: &sqlx::PgPool) -> (String, Uuid) {
    let email = format!("admin-flow-{}@example.com", Uuid::now_v7());
    let response = send(
        app,
        "POST",
        "/auth/register",
        Some(json!({ "email": email, "password": "kata sandi panjang" })),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let token = json(response).await["data"]["access_token"]
        .as_str()
        .expect("an access token")
        .to_owned();

    let id = sqlx::query_scalar!("SELECT id FROM users WHERE email = $1::citext", email)
        .fetch_one(pool)
        .await
        .expect("the account we just registered");

    (token, id)
}

/// Promotes somebody directly in the database.
///
/// Deliberately SQL rather than the endpoint: this file's first tests exist
/// before `PATCH /admin/users/{id}/role` does, and a test that can only make
/// an admin by using the feature under test cannot fail independently of it.
async fn promote(pool: &sqlx::PgPool, id: Uuid) {
    sqlx::query!("UPDATE users SET platform_role = 'admin' WHERE id = $1", id)
        .execute(pool)
        .await
        .expect("promoting");
}
```

The `sqlx::query!` calls in a test file are compiled against the same offline cache, so `make be-prepare` must run after this file is written.

Then the tests:

```rust
#[tokio::test]
async fn an_ordinary_account_is_refused_from_an_admin_route() {
    // 403 and not 404: AM-84 AC2 asks for a rejection the person can
    // understand, and hiding the route from an authenticated account buys
    // nothing.
    let (app, pool) = app!();
    let (token, _) = a_person(&app, &pool).await;
    let (_, target) = a_person(&app, &pool).await;

    let response = send(
        &app,
        "GET",
        &format!("/admin/users/{target}/vehicles"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn an_unauthenticated_caller_is_refused_before_any_role_is_read() {
    let (app, pool) = app!();
    let (_, target) = a_person(&app, &pool).await;

    let response = send(
        &app,
        "GET",
        &format!("/admin/users/{target}/vehicles"),
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_demoted_admin_is_refused_on_their_very_next_admin_request() {
    // AC1, and the reason the role is not in the session: the token is never
    // reissued and never revoked, and the refusal still takes effect
    // immediately, because nothing cached the role.
    let (app, pool) = app!();
    let (token, admin_id) = a_person(&app, &pool).await;
    let (_, target) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;

    let before = send(
        &app,
        "GET",
        &format!("/admin/users/{target}/vehicles"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(before.status(), StatusCode::OK);

    sqlx::query!(
        "UPDATE users SET platform_role = 'user' WHERE id = $1",
        admin_id
    )
    .execute(&pool)
    .await
    .expect("demoting");

    let after = send(
        &app,
        "GET",
        &format!("/admin/users/{target}/vehicles"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(after.status(), StatusCode::FORBIDDEN, "the role was cached");
}

#[tokio::test]
async fn a_demoted_admins_ordinary_requests_are_unaffected() {
    // The other half of AC1, and the half a literal reading gets wrong. A
    // demoted admin is still a user, and their garage still belongs to them.
    // Also the normal-path regression: adding the Admin extractor must not
    // change what an Authenticated route does.
    let (app, pool) = app!();
    let (token, admin_id) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;

    sqlx::query!(
        "UPDATE users SET platform_role = 'user' WHERE id = $1",
        admin_id
    )
    .execute(&pool)
    .await
    .expect("demoting");

    let response = send(&app, "GET", "/vehicles", None, Some(&token)).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn an_admin_reading_somebody_elses_cars_gets_no_plate_no_vin_and_no_price() {
    // AC4, and the reason the endpoint was pulled into this ticket at all.
    // The status assertion is not decoration: without an admin read endpoint
    // this test would receive a 404 and pass while asserting nothing, which
    // is the defect class this project's reviewers caught nine times in one
    // ticket.
    let (app, pool) = app!();
    let (admin_token, admin_id) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;
    let (owner_token, owner_id) = a_person(&app, &pool).await;

    let created = send(
        &app,
        "POST",
        "/vehicles",
        Some(json!({
            "described_as": "Avanza 2019",
            "plate": "B 1234 XYZ",
            "vin": "MHFXW42G8K0123456",
            "purchase_price": "185000000",
        })),
        Some(&owner_token),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);

    let response = send(
        &app,
        "GET",
        &format!("/admin/users/{owner_id}/vehicles"),
        None,
        Some(&admin_token),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "a 404 would make every assertion below vacuous"
    );

    let body = json(response).await;
    assert_eq!(
        body["data"].as_array().map(Vec::len),
        Some(1),
        "the admin did not see the car at all"
    );

    // Asserted on the whole serialised body, not field by field: a field-by-
    // field check passes on a response that renamed the field, and the values
    // are what must never travel.
    let text = body.to_string();
    for secret in ["B 1234 XYZ", "MHFXW42G8K0123456", "185000000"] {
        assert!(
            !text.contains(secret),
            "private vehicle data reached an admin: {secret} in {text}"
        );
    }
    assert!(
        body["data"][0].get("summary").is_none(),
        "the admin response carried a spend summary, which is service cost"
    );
}
```

Check `POST /vehicles`'s request field names against `tests/garage_flow.rs` before writing — `plate`, `vin`, and `purchase_price` are the names `PrivateResponse` uses, and `garage_flow.rs` already exercises the create-with-private path. If the create endpoint does not accept them inline, set them through whichever call `garage_flow.rs` uses and keep the assertion identical.

- [ ] **Step 7: regenerate, gate, and sabotage**

```bash
make be-prepare && git add apps/api/.sqlx
make be-sqlx-check
cd apps/api && cargo fmt --check
```
then from the root: `make be-lint`, `make be-boundary`, `make be-test`.

Sabotages, each reverted:

| Sabotage | Test that must die |
|---|---|
| Change `Some(PlatformRole::Admin) => Ok(...)` to accept `Some(PlatformRole::User)` too | `an_ordinary_account_is_refused_from_an_admin_route` and `a_demoted_admin_is_refused_on_their_very_next_admin_request` |
| Give the handler `_caller: Authenticated` instead of `Admin` | `an_ordinary_account_is_refused_from_an_admin_route` |
| Populate `summary` on the admin response by calling `service_summary::for_list` | `an_admin_reading_somebody_elses_cars_gets_no_plate_no_vin_and_no_price` |

- [ ] **Step 8: README row**

Add to the endpoint table in `apps/api/README.md`, after the `/vehicles/…` rows:

```
| `GET /admin/users/{id}/vehicles` | `200` | platform admin — that person's cars, with no plate, VIN, price, or spend |
```

### Acceptance criteria

1. All five tests pass, and each sabotage in Step 7 reddens the named test and nothing else.
2. `Admin`'s `match` enumerates `Some(PlatformRole::User) | None` explicitly — there is no `_` arm, so adding a variant to `PlatformRole` is a compile error.
3. Neither a database error nor a missing row can produce a role: `platform_role_of`'s `Result` is propagated with `?` into `ApiError::internal`, and there is no `unwrap_or`, no `unwrap_or_default`, and no `unwrap_or_else` anywhere on the path. **This one is a reviewer check, not a test** — simulating a dead pool inside the suite costs more than it proves, and pretending a test covers it would be worse than saying so.
4. A rejected admin request emits exactly one `WARN` carrying `user_id` and `route`, and nothing else. Verify by reading the captured output of `an_ordinary_account_is_refused_from_an_admin_route`, not by reading the code.
5. `make be-sqlx-check` and the full gate are `EXIT=0`.

**Block B applies to this task.**

---

## Task 4: `usecase::roles::set_role`

The one transaction both entrances share. Everything that makes this ticket correct rather than merely present is in this function: the lock, the order of the reads, the audit row preceding the update, and the no-op branch that makes a retry safe.

**Files:**
- Create: `apps/api/crates/runtime/src/usecase/roles.rs`
- Modify: `apps/api/crates/runtime/src/usecase/mod.rs` — `pub mod roles;`
- Modify: `apps/api/crates/runtime/src/adapter/postgres/user_repo.rs` — `lock_platform_role`, `admin_count`, `insert_role_change`, `set_platform_role`, `find_id_by_email`
- Create: `apps/api/crates/runtime/tests/role_change_flow.rs`
- Regenerate: `apps/api/.sqlx`

**Interfaces:**
- Consumes: Task 2's schema; Task 3's `PlatformRole` and `platform_role_of`
- Produces:
  ```rust
  pub enum Actor { Admin(Uuid), Bootstrap }
  pub struct RoleChange { pub target_user_id: Uuid, pub from_role: PlatformRole,
                          pub to_role: PlatformRole, pub created_at: OffsetDateTime }
  pub enum RoleError { NotFound, NotAdmin, AdminExists, InvalidReason(String), Database(sqlx::Error) }
  pub async fn set_role(pool: &PgPool, actor: Actor, target_user_id: Uuid,
                        to_role: PlatformRole, reason: &str)
      -> Result<Option<RoleChange>, RoleError>
  ```
  `Ok(None)` is the no-op — already in that role, nothing written.
- Produces in `user_repo`: `lock_platform_role`, `admin_count`, `insert_role_change(conn, RoleChangeRow<'_>) -> Result<OffsetDateTime, sqlx::Error>`, `set_platform_role`, `find_id_by_email`

**TDD: yes** — the spec's own verdict, and it is right. Three branches (no-op, actor no longer an admin, real change), an ordering requirement inside a transaction, and a concurrency property. The failing test comes first. Because `set_role` owns a transaction it cannot be unit tested; the red test is an integration test in `tests/role_change_flow.rs` calling the use case directly, exactly as `tests/part_merge_flow.rs` calls `part_merge::merge` — but with the loud harness, not that file's.

**Big O and access pattern.** `platform_role_of` is a primary-key lookup, run twice per HTTP call (`O(log U)` each). `admin_count` is `SELECT count(*) FROM users WHERE platform_role = 'admin'` — a sequential scan, `O(U)`, with no supporting index, and that is correct: it runs once per `grant-admin` invocation, which is a hand-typed operational command run perhaps twice in the platform's life. Mark it with a `ponytail:` comment naming `CREATE INDEX … ON users (platform_role) WHERE platform_role = 'admin'` as the upgrade if it ever reaches a hot path. The insert and the update are single-row writes against indexed keys. The advisory lock serialises the whole thing platform-wide, which is acceptable because a role change is a human action measured in units per year.

**Minimality check.** One file, one public function, five repository functions with no wrapper types beyond the one that keeps `insert_role_change` under the parameter ceiling. No `RoleChangeRepo` trait — this repository's rule is that a port becomes a trait only when the adapter will be swapped, orchestration needs an I/O seam, or two implementations exist; none holds. No status column, no soft-delete, no read endpoint.

### Steps

- [ ] **Step 1: write the failing tests first**

Create `tests/role_change_flow.rs`. Copy the harness from `tests/build_list_flow.rs` — the `app!` macro that returns `(app, pool)` — and use only the `pool` half, discarding the router with `let (_app, pool) = app!();`. **Do not** copy `tests/part_merge_flow.rs`'s `pool!` macro; a database-only harness that returns silently is exactly the shape PR #18 removed, and this file would reintroduce it.

```rust
//! `usecase::roles::set_role` against a real database.
//!
//! These call the use case directly rather than through HTTP. The endpoint
//! arrives in the next task; the properties below are about a transaction and
//! a lock, and putting a router in front of them would only add ways for the
//! test to be wrong.

#![expect(
    clippy::expect_used,
    reason = "test helpers abort rather than propagate"
)]

use anakmobil_runtime::adapter::postgres::user_repo::{self, PlatformRole};
use anakmobil_runtime::usecase::roles::{self, Actor, RoleError};
use sqlx::PgPool;
use uuid::Uuid;

// … app! macro copied from tests/build_list_flow.rs …

async fn a_user(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query!(
        "INSERT INTO users (id, email, password_hash) VALUES ($1, $2, 'x')",
        id,
        format!("roles-{id}@example.com"),
    )
    .execute(pool)
    .await
    .expect("creating a user");
    id
}

async fn an_admin(pool: &PgPool) -> Uuid {
    let id = a_user(pool).await;
    sqlx::query!(
        "UPDATE users SET platform_role = 'admin' WHERE id = $1",
        id
    )
    .execute(pool)
    .await
    .expect("promoting");
    id
}

#[tokio::test]
async fn promoting_writes_the_audit_row_and_the_column_together() {
    let (_app, pool) = app!();
    let actor = an_admin(&pool).await;
    let target = a_user(&pool).await;

    let change = roles::set_role(
        &pool,
        Actor::Admin(actor),
        target,
        PlatformRole::Admin,
        "catalog curation",
    )
    .await
    .expect("the promotion should succeed")
    .expect("a real change returns Some");

    assert_eq!(change.from_role, PlatformRole::User);
    assert_eq!(change.to_role, PlatformRole::Admin);
    assert_eq!(change.target_user_id, target);

    let mut conn = pool.acquire().await.expect("a connection");
    assert_eq!(
        user_repo::platform_role_of(&mut conn, target)
            .await
            .expect("reading the role"),
        Some(PlatformRole::Admin)
    );

    let row = sqlx::query!(
        r#"
        SELECT actor_id,
               from_role AS "from_role: PlatformRole",
               to_role   AS "to_role: PlatformRole",
               reason
        FROM role_changes WHERE target_user_id = $1
        "#,
        target
    )
    .fetch_one(&pool)
    .await
    .expect("exactly one audit row");
    assert_eq!(row.actor_id, Some(actor));
    assert_eq!(row.from_role, PlatformRole::User);
    assert_eq!(row.to_role, PlatformRole::Admin);
    assert_eq!(row.reason, "catalog curation");
}

#[tokio::test]
async fn setting_the_role_somebody_already_has_writes_nothing_and_is_not_an_error() {
    // 204 upstream, and it is what makes a retry safe: a dropped connection
    // after a successful PATCH leaves the client unsure, and retrying lands
    // here. Without this branch the CHECK constraint fires and surfaces as a
    // generic 500 — a correct database rejecting a reasonable client.
    let (_app, pool) = app!();
    let actor = an_admin(&pool).await;
    let target = an_admin(&pool).await;

    let result = roles::set_role(
        &pool,
        Actor::Admin(actor),
        target,
        PlatformRole::Admin,
        "already there",
    )
    .await
    .expect("a no-op is not an error");
    assert!(result.is_none());

    let count = sqlx::query_scalar!(
        "SELECT count(*) FROM role_changes WHERE target_user_id = $1",
        target
    )
    .fetch_one(&pool)
    .await
    .expect("counting");
    assert_eq!(count, Some(0), "a no-op wrote an audit row");
}

#[tokio::test]
async fn an_actor_demoted_since_the_extractor_ran_is_refused_under_the_lock() {
    // The finding a checklist and a second model each found half of. The
    // extractor checked the actor's role before the handler ran; re-reading it
    // inside the transaction is what stops an admin demoted in between from
    // completing the mutation they had already started.
    let (_app, pool) = app!();
    let actor = a_user(&pool).await; // never an admin — the same state as demoted
    let target = a_user(&pool).await;

    let err = roles::set_role(
        &pool,
        Actor::Admin(actor),
        target,
        PlatformRole::Admin,
        "should be refused",
    )
    .await
    .expect_err("a non-admin actor must be refused");
    assert!(matches!(err, RoleError::NotAdmin));

    let count = sqlx::query_scalar!("SELECT count(*) FROM role_changes WHERE target_user_id = $1", target)
        .fetch_one(&pool)
        .await
        .expect("counting");
    assert_eq!(count, Some(0));
}

#[tokio::test]
async fn a_target_that_does_not_exist_is_not_found() {
    let (_app, pool) = app!();
    let actor = an_admin(&pool).await;

    let err = roles::set_role(
        &pool,
        Actor::Admin(actor),
        Uuid::now_v7(),
        PlatformRole::Admin,
        "nobody",
    )
    .await
    .expect_err("an unknown target");
    assert!(matches!(err, RoleError::NotFound));
}

#[tokio::test]
async fn an_admin_may_demote_themselves_and_the_platform_may_reach_zero_admins() {
    // There is deliberately no last-admin guard. The alternative collapses on
    // contact with the bootstrap rule: if `grant-admin` only succeeds at zero
    // admins and nothing may ever reach zero, `grant-admin` is dead code from
    // the second day and there is no recovery path at all.
    let (_app, pool) = app!();
    let actor = an_admin(&pool).await;

    let change = roles::set_role(
        &pool,
        Actor::Admin(actor),
        actor,
        PlatformRole::User,
        "stepping down",
    )
    .await
    .expect("self-demotion is allowed")
    .expect("a real change");
    assert_eq!(change.from_role, PlatformRole::Admin);
    assert_eq!(change.to_role, PlatformRole::User);
}

#[tokio::test]
async fn a_bootstrap_is_refused_once_the_platform_has_an_admin() {
    let (_app, pool) = app!();
    let _existing = an_admin(&pool).await;
    let target = a_user(&pool).await;

    let err = roles::set_role(
        &pool,
        Actor::Bootstrap,
        target,
        PlatformRole::Admin,
        "should be refused",
    )
    .await
    .expect_err("the platform already has an admin");
    assert!(matches!(err, RoleError::AdminExists));
}

#[tokio::test]
async fn an_empty_or_whitespace_reason_is_refused_on_both_entrances() {
    // `reason TEXT NOT NULL` accepts the empty string, which would make the
    // trail useless while looking complete. The guard lives in the use case
    // rather than at the HTTP boundary, because the CLI is a second entrance
    // and a guard on one door of two is the defect AM-361's ledger records.
    let (_app, pool) = app!();
    let actor = an_admin(&pool).await;
    let target = a_user(&pool).await;

    for blank in ["", "   ", "\n\t"] {
        let err = roles::set_role(&pool, Actor::Admin(actor), target, PlatformRole::Admin, blank)
            .await
            .expect_err("a blank reason");
        assert!(matches!(err, RoleError::InvalidReason(_)), "accepted {blank:?}");
    }

    let err = roles::set_role(
        &pool,
        Actor::Admin(actor),
        target,
        PlatformRole::Admin,
        &"x".repeat(1_001),
    )
    .await
    .expect_err("an unbounded reason");
    assert!(matches!(err, RoleError::InvalidReason(_)));
}

#[tokio::test]
async fn two_concurrent_promotions_of_one_person_produce_exactly_one_audit_row() {
    // The lock's job: without it both callers read `from_role = user`, both
    // write a row claiming `user → admin`, and one of those rows is a lie
    // about a change that did not happen. It is NOT protecting a last admin —
    // there is no last-admin rule.
    //
    // Twenty rounds, because a race that fails one time in ten passes a single
    // run and reports itself fixed.
    let (_app, pool) = app!();

    for round in 0..20 {
        let actor_a = an_admin(&pool).await;
        let actor_b = an_admin(&pool).await;
        let target = a_user(&pool).await;

        let (first, second) = tokio::join!(
            roles::set_role(&pool, Actor::Admin(actor_a), target, PlatformRole::Admin, "a"),
            roles::set_role(&pool, Actor::Admin(actor_b), target, PlatformRole::Admin, "b"),
        );

        let changed = [&first, &second]
            .iter()
            .filter(|r| matches!(r, Ok(Some(_))))
            .count();
        assert_eq!(changed, 1, "round {round}: {changed} callers claimed the change");
        assert!(first.is_ok() && second.is_ok(), "round {round}: {first:?} {second:?}");

        let rows = sqlx::query_scalar!(
            "SELECT count(*) FROM role_changes WHERE target_user_id = $1",
            target
        )
        .fetch_one(&pool)
        .await
        .expect("counting");
        assert_eq!(rows, Some(1), "round {round}: the audit trail is not true");
    }
}
```

- [ ] **Step 2: run them and confirm they fail for the intended reason**

```bash
make be-test 2>&1 | tail -40
```

Expected: compilation errors naming `usecase::roles` as an unresolved module — `E0432` / `E0433`. **Not** a test-body typo, and **not** a green run. A green run here means `app!` returned early; check `DATABASE_URL` and `REDIS_URL` are exported (`make be-test` does it; a bare `cargo test` does not, and now panics rather than passing).

- [ ] **Step 3: the repository functions**

Append to `adapter/postgres/user_repo.rs`:

```rust
/// Serialise every platform-role change against every other, for this
/// transaction.
///
/// Released on commit or rollback — there is no unlock to forget.
///
/// **Two arguments, deliberately.** `pg_advisory_xact_lock(key, 0)` occupies a
/// different keyspace from the single-argument locks already in use —
/// `hashtext('part_merge')` in `part_repo::lock_merges` — so the two cannot
/// collide even on the same hash. The other two-argument lock in this codebase
/// keys on a person (`usecase::parts::allowance_spent`); this one keys on the
/// platform, because a role change is rare and serialising all of them costs
/// nothing.
///
/// **Only useful inside a transaction.** On a pooled autocommit connection
/// every statement is its own transaction, so the lock would be taken and
/// released before the next read — a guard that looks right and does nothing.
/// That bug was shipped and fixed once already; see `usecase::parts::suggest`.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn lock_platform_role(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query!(r#"SELECT pg_advisory_xact_lock(hashtext('platform_role'), 0)"#)
        .execute(conn)
        .await
        .map(drop)
}

/// How many accounts are admins right now.
///
/// The bootstrap precondition, and it is read **inside** the same lock and the
/// same transaction as the write. Counting and then inserting across two
/// statements is check-then-act: two operators running `grant-admin`
/// concurrently both see zero and both succeed. That is the defect the AM-361
/// fix pass closed twice and it does not get to appear a third time.
///
/// Complexity: `O(U)` — a sequential scan over `users`, no index.
/// ponytail: correct today. `grant-admin` is a hand-typed operational command
/// run perhaps twice in the platform's life, and an index maintained on every
/// account write to serve it would cost more than it saves. If a count of
/// admins ever appears on a request path, add
/// `CREATE INDEX users_admins_idx ON users (platform_role) WHERE platform_role = 'admin'`.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn admin_count(conn: &mut PgConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT count(*) FROM users WHERE platform_role = 'admin'"#)
        .fetch_one(conn)
        .await
        .map(|count| count.unwrap_or(0))
}

/// One row of the audit trail, on its way to being written.
///
/// A struct rather than seven positional arguments: `clippy::too_many_arguments`
/// is a denied warning at eight, and two adjacent `PlatformRole` values and two
/// adjacent `Uuid`s are exactly the shape that gets swapped at a call site.
/// `shared::validation::DecimalSpec` is the same fix for the same reason.
pub struct RoleChangeRow<'a> {
    pub id: Uuid,
    /// `None` for a bootstrap — an operational command has no signed-in human
    /// behind it.
    pub actor_id: Option<Uuid>,
    pub target_user_id: Uuid,
    pub from_role: PlatformRole,
    pub to_role: PlatformRole,
    pub reason: &'a str,
}

/// Record a role change. Returns when it was recorded.
///
/// Called **before** the column is updated, and its failure fails the whole
/// change: a privilege that exists with no record of how it was granted is
/// worse than a privilege that failed to be granted.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails, including a `CHECK` violation
/// on `role_changes_real_change` — which the use case's no-op branch means no
/// client should ever be able to reach.
pub async fn insert_role_change(
    conn: &mut PgConnection,
    row: RoleChangeRow<'_>,
) -> Result<time::OffsetDateTime, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        INSERT INTO role_changes
            (id, actor_id, target_user_id, from_role, to_role, reason)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING created_at
        "#,
        row.id,
        row.actor_id,
        row.target_user_id,
        row.from_role as PlatformRole,
        row.to_role as PlatformRole,
        row.reason,
    )
    .fetch_one(conn)
    .await
}

/// Write the new platform role.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn set_platform_role(
    conn: &mut PgConnection,
    id: Uuid,
    role: PlatformRole,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE users SET platform_role = $2 WHERE id = $1"#,
        id,
        role as PlatformRole,
    )
    .execute(conn)
    .await
    .map(drop)
}

/// Find an account by email, for the operational command that takes one.
///
/// Email is `CITEXT`, so the comparison is case-insensitive in the database
/// rather than in whichever caller remembered to lowercase.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn find_id_by_email(
    conn: &mut PgConnection,
    email: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT id FROM users WHERE email = $1::citext"#, email)
        .fetch_optional(conn)
        .await
}
```

The `$n as PlatformRole` casts are how sqlx binds a custom enum type to a query parameter. If the macro rejects the form, the alternative is `$4::platform_role` in the SQL with the plain binding — try the cast first, since `build_repo` binds `Visibility` the same way and that is the shape already proven here.

- [ ] **Step 4: the use case**

Create `usecase/roles.rs`:

```rust
//! Changing a platform role, from either entrance.
//!
//! Two write paths, one use case, one transaction:
//!
//! ```text
//! anakmobil grant-admin <email>      → actor_id NULL, only when the admin count is zero
//! PATCH /admin/users/{id}/role       → actor_id is the calling admin
//!                     ↓ both
//!         usecase::roles::set_role()
//! ```
//!
//! A guard that lives on one of two entrances is a guard on neither, which is
//! the finding AM-361's ledger records against the parts queue. So the reason
//! check, the lock, the re-reads, and the audit write are all here, and both
//! callers are thin.

use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::adapter::postgres::user_repo::{self, PlatformRole, RoleChangeRow};

/// The longest reason worth storing.
///
/// Bounded because `role_changes` is append-only: nothing can ever trim a row
/// that turned out to hold a paragraph, or a paste. A thousand characters is
/// far more than a sentence explaining a promotion and far less than a way to
/// use an audit table as storage.
const MAX_REASON: usize = 1_000;

/// Who is asking, and what that entitles them to.
#[derive(Debug, Clone, Copy)]
pub enum Actor {
    /// A signed-in admin. Their role is re-read under the lock — the extractor
    /// checked it before the handler ran, and an admin demoted in between
    /// would otherwise still complete the mutation they had already started.
    Admin(Uuid),
    /// The operational command. There is no actor to re-read; what is re-read
    /// instead is the admin count, which is this path's own precondition.
    Bootstrap,
}

/// What changed. Deliberately carries no email and no reason — it answers what
/// changed, not who anybody is.
#[derive(Debug, Clone, Copy)]
pub struct RoleChange {
    pub target_user_id: Uuid,
    pub from_role: PlatformRole,
    pub to_role: PlatformRole,
    pub created_at: OffsetDateTime,
}

/// Why a role change did not happen.
#[derive(Debug, thiserror::Error)]
pub enum RoleError {
    #[error("no such account")]
    NotFound,
    /// The caller is not an admin — either they never were, or they were
    /// demoted between the extractor and this transaction. The same answer
    /// either way, because they are the same situation.
    #[error("the caller is not a platform admin")]
    NotAdmin,
    /// The bootstrap precondition failed: somebody is already an admin, so the
    /// operational command is not the way in.
    #[error("the platform already has an admin")]
    AdminExists,
    /// The message names the problem in Bahasa Indonesia and reaches the
    /// client, so it carries only what the caller supplied.
    #[error("{0}")]
    InvalidReason(String),
    #[error("the database could not be reached")]
    Database(#[from] sqlx::Error),
}

/// Change somebody's platform role, or report that there was nothing to change.
///
/// `Ok(None)` means the account already had that role: nothing was written,
/// and the caller answers `204`. That branch is what makes a retry safe — a
/// dropped connection after a successful call leaves the client unsure, and
/// retrying lands here rather than on the `CHECK` constraint, which would
/// surface as a generic 500.
///
/// # The order inside the transaction is the design
///
/// 1. take the platform-role advisory lock
/// 2. re-read the actor's role (HTTP) or the admin count (CLI)
/// 3. re-read the target's role, and decide
/// 4. insert the audit row
/// 5. update the column
///
/// The lock's job is to make the audit row **true**. Without it, two admins
/// promoting the same person concurrently both read `from_role = user`, both
/// write a row claiming `user → admin`, and one of those rows is a lie about a
/// change that did not happen. It is not protecting a last admin — there is no
/// last-admin rule, and zero admins is a legitimate state that `grant-admin`
/// exists to recover from.
///
/// The audit insert precedes the update and its failure fails the change: a
/// privilege that exists with no record of how it was granted is worse than a
/// privilege that failed to be granted.
///
/// Complexity: two primary-key lookups (`O(log U)`) on the HTTP path, or one
/// lookup plus an `O(U)` admin count on the bootstrap path, then two
/// single-row writes.
///
/// # Errors
///
/// [`RoleError::InvalidReason`] for a blank or over-long reason,
/// [`RoleError::NotAdmin`] when the actor is not an admin,
/// [`RoleError::AdminExists`] when a bootstrap runs on a platform that already
/// has one, [`RoleError::NotFound`] when the target does not exist.
pub async fn set_role(
    pool: &PgPool,
    actor: Actor,
    target_user_id: Uuid,
    to_role: PlatformRole,
    reason: &str,
) -> Result<Option<RoleChange>, RoleError> {
    let reason = check_reason(reason)?;

    // A transaction, not a pooled connection: `pg_advisory_xact_lock` releases
    // at the end of its transaction, and on an autocommit connection every
    // statement IS its own transaction — so the lock would be gone before the
    // first read. That exact bug shipped once in `usecase::parts::suggest`.
    let mut tx = pool.begin().await?;
    user_repo::lock_platform_role(&mut tx).await?;

    let actor_id = match actor {
        Actor::Admin(id) => {
            if user_repo::platform_role_of(&mut tx, id).await? != Some(PlatformRole::Admin) {
                return Err(RoleError::NotAdmin);
            }
            Some(id)
        }
        Actor::Bootstrap => {
            // Inside the lock, not before the call. Counting and then
            // inserting across two statements is check-then-act, and two
            // operators running the command at once would both see zero.
            if user_repo::admin_count(&mut tx).await? > 0 {
                return Err(RoleError::AdminExists);
            }
            None
        }
    };

    let from_role = user_repo::platform_role_of(&mut tx, target_user_id)
        .await?
        .ok_or(RoleError::NotFound)?;

    if from_role == to_role {
        // Nothing written. The transaction is dropped without a commit, which
        // rolls it back and releases the lock.
        return Ok(None);
    }

    let created_at = user_repo::insert_role_change(
        &mut tx,
        RoleChangeRow {
            id: Uuid::now_v7(),
            actor_id,
            target_user_id,
            from_role,
            to_role,
            reason: &reason,
        },
    )
    .await?;

    user_repo::set_platform_role(&mut tx, target_user_id, to_role).await?;
    tx.commit().await?;

    // Ids and roles only. Never the reason, never an email — the repository
    // rule is method, route, status, latency, request id, and a user id is not
    // a credential. The AM-361 fix pass had to remove a caller-supplied value
    // from a log line for exactly this reason.
    tracing::info!(
        %target_user_id,
        actor_id = ?actor_id,
        from_role = ?from_role,
        to_role = ?to_role,
        "platform role changed"
    );

    Ok(Some(RoleChange {
        target_user_id,
        from_role,
        to_role,
        created_at,
    }))
}

/// Trim the reason and refuse the two shapes an append-only column cannot
/// recover from: nothing at all, and more than anybody meant to type.
///
/// Here rather than at the HTTP boundary, because there are two entrances and
/// the CLI is not one of them. Messages are Bahasa Indonesia — they are
/// product text, and the repository rule puts product text in Indonesian.
fn check_reason(reason: &str) -> Result<String, RoleError> {
    let trimmed = reason.trim();
    if trimmed.is_empty() {
        return Err(RoleError::InvalidReason("Alasan wajib diisi.".to_owned()));
    }
    if trimmed.chars().count() > MAX_REASON {
        return Err(RoleError::InvalidReason(format!(
            "Alasan maksimal {MAX_REASON} karakter."
        )));
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_reason_is_refused() {
        for blank in ["", "   ", "\n\t "] {
            assert!(matches!(
                check_reason(blank),
                Err(RoleError::InvalidReason(_))
            ));
        }
    }

    #[test]
    fn a_reason_is_stored_trimmed() {
        assert_eq!(
            check_reason("  kurasi katalog  ").expect("accepted"),
            "kurasi katalog"
        );
    }

    #[test]
    fn the_length_bound_is_counted_in_characters_not_bytes() {
        // A thousand emoji is a thousand characters and four thousand bytes.
        // Counting bytes would refuse a reason written in a script this
        // platform's users actually type.
        assert!(check_reason(&"é".repeat(MAX_REASON)).is_ok());
        assert!(check_reason(&"é".repeat(MAX_REASON + 1)).is_err());
    }
}
```

Add `pub mod roles;` to `usecase/mod.rs`, in alphabetical position.

- [ ] **Step 5: green, then the gate**

```bash
make be-prepare && git add apps/api/.sqlx
make be-sqlx-check
cd apps/api && cargo fmt --check
```
then from the root: `make be-lint`, `make be-boundary`, `make be-test`.

Every test from Step 1 must now pass. `two_concurrent_promotions_of_one_person_produce_exactly_one_audit_row` runs twenty rounds; if it fails on any round, the lock is not doing its job — check that `set_role` opens a transaction rather than acquiring a connection.

- [ ] **Step 6: prove each guard can fail**

| Sabotage | Test that must die, and only it |
|---|---|
| Delete the `lock_platform_role` call | `two_concurrent_promotions_of_one_person_produce_exactly_one_audit_row` |
| Take the lock on `pool.acquire()` instead of `pool.begin()` | the same one — this is the shipped-once bug, and if it does not redden, the concurrency test is decoration |
| Delete the `Actor::Admin` re-read (accept any actor) | `an_actor_demoted_since_the_extractor_ran_is_refused_under_the_lock` |
| Move the `admin_count` check before `pool.begin()` | `a_bootstrap_is_refused_once_the_platform_has_an_admin` will still pass — say so, and record that this specific ordering is pinned by review rather than by a test, because a two-process race is not reproducible in-suite |
| Delete the `from_role == to_role` branch | **two** tests, not one: `setting_the_role_somebody_already_has_writes_nothing_and_is_not_an_error` (it becomes a `Database` error from the `CHECK`) **and** `two_concurrent_promotions_of_one_person_produce_exactly_one_audit_row` — the concurrency test's losing racer legitimately lands on the no-op path under the lock, so removing the branch breaks it too. Verified by the implementer; this table originally named only the first. |
| Swap the audit insert and the column update | nothing dies — **and that is the finding.** Both are in one transaction, so no test can distinguish the order. Record it: the ordering is a review property, not a tested one |

The last two rows are deliberate. A sabotage table that only lists survivable mutations is a table somebody wrote to look thorough; naming the two that no test can catch is what tells the reviewer where to actually look.

### Acceptance criteria

1. All eight integration tests and three unit tests pass, run through `make be-test`.
2. The four sabotages in Step 6 that name a test redden exactly that test.
3. The two that name no test are reported as review properties in the completion note, not quietly omitted.
4. `set_role` takes five parameters and `insert_role_change` takes two — `make be-lint` is `EXIT=0` with `too_many_arguments` denied through `-D warnings`.
5. No log line in this module carries a reason or an email. Verified by reading the module, and by `grep -n 'reason' src/usecase/roles.rs` showing it only in the signature, the guard, and the `RoleChangeRow` field.

**Block B applies to this task.**

---

## Task 5: `PATCH /admin/users/{id}/role`

**Files:**
- Modify: `apps/api/crates/runtime/src/adapter/http/admin.rs` — the request and response types, the handler, the error mapping
- Modify: `apps/api/crates/runtime/src/adapter/http/mod.rs` — one route, and `patch` in the `axum::routing` import
- Modify: `apps/api/crates/runtime/tests/admin_flow.rs` — the response-contract tests
- Modify: `apps/api/README.md` — one endpoint row

**Interfaces:**
- Consumes: Task 3's `Admin` extractor and `admin.rs`; Task 4's `roles::{set_role, Actor, RoleChange, RoleError}`
- Produces: the route `PATCH /admin/users/{id}/role`

**TDD: no** — the use case's contract was driven out by tests in Task 4. What is left here is a mapping from that contract to statuses, verified by integration tests written immediately after, including the retry-safety case.

**Big O and access pattern.** Nothing new; the handler is a call and a match.

**Minimality check.** No new use case, no new repository call, no `RoleChangeResponse` field beyond the four the spec names. The 204 branch reuses the existing `NoContent`.

### Steps

- [ ] **Step 1: the request and response types**

In `adapter/http/admin.rs`:

```rust
#[derive(Debug, Deserialize)]
pub struct RoleRequest {
    pub role: PlatformRole,
    /// Why. Stored on the audit row, never returned and never logged.
    pub reason: String,
}

/// What changed.
///
/// No email and no reason: this answers what changed, not who anybody is.
#[derive(Debug, Serialize)]
pub struct RoleChangeResponse {
    pub target_user_id: Uuid,
    pub from_role: PlatformRole,
    pub to_role: PlatformRole,
    pub created_at: String,
}

impl From<RoleChange> for RoleChangeResponse {
    fn from(change: RoleChange) -> Self {
        Self {
            target_user_id: change.target_user_id,
            from_role: change.from_role,
            to_role: change.to_role,
            created_at: change.created_at.format(&Rfc3339).unwrap_or_default(),
        }
    }
}
```

`Rfc3339` comes from `time::format_description::well_known::Rfc3339`; `adapter/http/builds.rs:66` uses the same `.unwrap_or_default()` shape and the same reasoning applies — a formatting quirk must not become a 500.

- [ ] **Step 2: the handler**

```rust
/// `PATCH /admin/users/{id}/role`
///
/// | Situation | Response |
/// |---|---|
/// | Role changed | `200` with the change |
/// | Already in that role | `204`, nothing written |
/// | Target does not exist | `404` |
/// | Caller is not an admin | `403`, before any lookup |
/// | Caller was demoted between the extractor and the lock | `403`, same code and message |
/// | Caller is not signed in | `401` |
///
/// **`204` rather than an error is what makes a retry safe.** A dropped
/// connection after a successful call leaves the client unsure; retrying hits
/// the no-op branch and succeeds.
///
/// **`403` before any lookup is what stops this being a user-id oracle**, and
/// it is automatic: the extractor runs before this body, so a non-admin never
/// reaches a query. An admin receiving `404` for a missing id is not a leak —
/// they are authorised to see the user list.
///
/// # Errors
///
/// See the table above.
pub async fn set_role(
    State(state): State<AppState>,
    caller: Admin,
    Path(id): Path<Uuid>,
    Json(body): Json<RoleRequest>,
) -> Result<Response, ApiError> {
    let change = roles::set_role(
        &state.pool,
        Actor::Admin(caller.user_id),
        id,
        body.role,
        &body.reason,
    )
    .await
    .map_err(to_role_error)?;

    Ok(match change {
        Some(change) => ApiResponse::ok(RoleChangeResponse::from(change)).into_response(),
        None => NoContent.into_response(),
    })
}

/// The single failure-to-response mapping for role changes.
///
/// Exhaustive on purpose: a new [`RoleError`] variant makes this match
/// non-exhaustive and the build fails until somebody decides what it means
/// over HTTP.
fn to_role_error(err: RoleError) -> ApiError {
    match err {
        RoleError::NotFound => ApiError::not_found(),
        // The same code and the same message the extractor would have given.
        // Two different answers for "you are not an admin" would tell a caller
        // which check they tripped.
        RoleError::NotAdmin => ApiError::forbidden(),
        RoleError::AdminExists => ApiError::conflict(),
        RoleError::InvalidReason(message) => {
            ApiError::validation(serde_json::json!({ "reason": message }))
        }
        RoleError::Database(inner) => ApiError::internal(anyhow::anyhow!(inner)),
    }
}
```

`RoleError::AdminExists` is unreachable from this handler — only `Actor::Bootstrap` produces it — and it is mapped anyway because the match must be exhaustive. Say so in a comment rather than reaching for `unreachable!()`, which is a `panic` and is denied.

Add the imports this needs: `axum::Json`, `axum::response::{IntoResponse, Response}`, `serde::{Deserialize, Serialize}`, `time::format_description::well_known::Rfc3339`, `crate::adapter::postgres::user_repo::PlatformRole`, `crate::shared::response::NoContent`, `crate::usecase::roles::{self, Actor, RoleChange, RoleError}`.

- [ ] **Step 3: the route**

In `adapter/http/mod.rs`, add `patch` to the routing import and one route:

```rust
use axum::routing::{get, patch, post, put};
```
```rust
        .route("/admin/users/{id}/role", patch(admin::set_role))
```

- [ ] **Step 4: the response-contract tests**

Append to `tests/admin_flow.rs`:

```rust
#[tokio::test]
async fn an_admin_promotes_somebody_and_the_response_names_the_change() {
    let (app, pool) = app!();
    let (token, admin_id) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;
    let (_, target) = a_person(&app, &pool).await;

    let response = send(
        &app,
        "PATCH",
        &format!("/admin/users/{target}/role"),
        Some(json!({ "role": "admin", "reason": "kurasi katalog" })),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = json(response).await;
    assert_eq!(body["data"]["target_user_id"], target.to_string());
    assert_eq!(body["data"]["from_role"], "user");
    assert_eq!(body["data"]["to_role"], "admin");
    assert!(body["data"]["created_at"].is_string());

    // The response answers what changed, not who anybody is.
    let text = body.to_string();
    assert!(!text.contains("kurasi katalog"), "the reason reached the client");
    assert!(!text.contains("@example.com"), "an email reached the client");
}

#[tokio::test]
async fn repeating_a_successful_promotion_answers_204_so_a_retry_is_safe() {
    let (app, pool) = app!();
    let (token, admin_id) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;
    let (_, target) = a_person(&app, &pool).await;

    let body = json!({ "role": "admin", "reason": "kurasi katalog" });

    let first = send(
        &app,
        "PATCH",
        &format!("/admin/users/{target}/role"),
        Some(body.clone()),
        Some(&token),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let second = send(
        &app,
        "PATCH",
        &format!("/admin/users/{target}/role"),
        Some(body),
        Some(&token),
    )
    .await;
    assert_eq!(
        second.status(),
        StatusCode::NO_CONTENT,
        "a retry must not become a 409 or a 500"
    );

    let rows = sqlx::query_scalar!(
        "SELECT count(*) FROM role_changes WHERE target_user_id = $1",
        target
    )
    .fetch_one(&pool)
    .await
    .expect("counting");
    assert_eq!(rows, Some(1), "the retry wrote a second audit row");
}

#[tokio::test]
async fn a_non_admin_is_refused_before_any_lookup_happens() {
    // The endpoint must not be a user-id oracle: a non-admin gets the same
    // 403 whether the id is real or invented.
    let (app, pool) = app!();
    let (token, _) = a_person(&app, &pool).await;
    let (_, real) = a_person(&app, &pool).await;
    let invented = Uuid::now_v7();

    for target in [real, invented] {
        let response = send(
            &app,
            "PATCH",
            &format!("/admin/users/{target}/role"),
            Some(json!({ "role": "admin", "reason": "should be refused" })),
            Some(&token),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}

#[tokio::test]
async fn an_admin_naming_a_target_that_does_not_exist_gets_404() {
    let (app, pool) = app!();
    let (token, admin_id) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;

    let response = send(
        &app,
        "PATCH",
        &format!("/admin/users/{}/role", Uuid::now_v7()),
        Some(json!({ "role": "admin", "reason": "nobody" })),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_blank_reason_is_a_422_naming_the_field() {
    let (app, pool) = app!();
    let (token, admin_id) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;
    let (_, target) = a_person(&app, &pool).await;

    let response = send(
        &app,
        "PATCH",
        &format!("/admin/users/{target}/role"),
        Some(json!({ "role": "admin", "reason": "   " })),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body = json(response).await;
    assert_eq!(body["error"]["code"], "validation_failed");
    assert!(body["error"]["details"]["reason"].is_string());
}

#[tokio::test]
async fn an_admin_may_demote_themselves() {
    // Allowed explicitly. With no last-admin rule there is nothing it can
    // break, and stepping down is an ordinary thing for a person to do.
    let (app, pool) = app!();
    let (token, admin_id) = a_person(&app, &pool).await;
    promote(&pool, admin_id).await;

    let response = send(
        &app,
        "PATCH",
        &format!("/admin/users/{admin_id}/role"),
        Some(json!({ "role": "user", "reason": "mundur" })),
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    // And the very next admin request is refused, because nothing cached it.
    let after = send(
        &app,
        "GET",
        &format!("/admin/users/{admin_id}/vehicles"),
        None,
        Some(&token),
    )
    .await;
    assert_eq!(after.status(), StatusCode::FORBIDDEN);
}
```

- [ ] **Step 5: gate and sabotage**

Run the full chain from Block A. Then:

| Sabotage | Test that must die |
|---|---|
| Return `ApiError::conflict()` for the `Ok(None)` case instead of `NoContent` | `repeating_a_successful_promotion_answers_204_so_a_retry_is_safe` |
| Map `RoleError::NotAdmin` to `ApiError::not_found()` | `a_non_admin_is_refused_before_any_lookup_happens` — **check this carefully**: the extractor refuses first, so this arm may be unreachable from HTTP and the sabotage may survive. If it survives, say so and record that the `NotAdmin` arm is exercised only by Task 4's use-case test |
| Add `reason` to `RoleChangeResponse` | `an_admin_promotes_somebody_and_the_response_names_the_change` |
| Change the handler's extractor to `Authenticated` | `a_non_admin_is_refused_before_any_lookup_happens` |

- [ ] **Step 6: README row**

```
| `PATCH /admin/users/{id}/role` | `200` / `204` | platform admin, re-checked inside the transaction; `204` when the account already has that role, so a retry is safe |
```

### Acceptance criteria

1. Every status in the spec's response-contract table is asserted by a test that would fail if the status changed.
2. Neither the reason nor any email appears in the response body — asserted on the serialised body, not field by field.
3. A retry writes exactly one audit row.
4. `to_role_error` is an exhaustive match with no `_` arm and no `unreachable!()`.
5. The full gate is `EXIT=0`.

**Block B applies to this task.**

---

## Task 6: `anakmobil grant-admin <email>`

The recovery path, and the only reason zero admins is a safe state to allow.

**Files:**
- Modify: `apps/api/crates/runtime/src/lib.rs` — the command branch, `run_grant_admin`, `Role::USAGE`
- Modify: `apps/api/README.md` — the "Run it" section

**Interfaces:**
- Consumes: Task 4's `roles::{set_role, Actor}`; `user_repo::find_id_by_email`
- Produces: the command `anakmobil grant-admin <email>`

**TDD: no** — the branch is argument dispatch and terminal I/O. The behaviour worth testing is `set_role`'s and it was driven out in Task 4. What is testable here is the argument surface, and Step 4 adds unit tests for exactly that, including the one that pins the spec's prohibition.

**Big O and access pattern.** One email lookup (`users_email_key`, `O(log U)`), then `set_role`'s `O(U)` admin count. Run by hand, twice in the platform's life.

**Minimality check.** No `clap`. `apps/api/CLAUDE.md` says to reach for it "when there are more subcommands than that, not before", and this is one more. No config file, no interactive confirmation, no `--dry-run`.

### Steps

- [ ] **Step 1: the argument surface**

In `lib.rs`, change `Role::USAGE` and add the command constant:

```rust
impl Role {
    const USAGE: &'static str = "usage: anakmobil <web|worker|migrate>\n       anakmobil grant-admin <email>";
```

`grant-admin` is in the usage line because a person typing `anakmobil` sees one argument surface, and a command they cannot discover is a recovery path that does not exist. It is **not** a fourth `Role` variant: `Role` models the process role — a property of the deployment, per `CONTEXT.md` — and `Role::parse` reads one argument with no room for an email or a reason.

- [ ] **Step 2: dispatch before `Role::parse`**

Replace the top of `run()`:

```rust
pub async fn run() -> anyhow::Result<()> {
    // A missing .env is normal — production supplies real environment
    // variables and has no file to load.
    let _ = dotenvy::dotenv();

    let mut args = std::env::args().skip(1);
    let command = args.next();

    // Matched before `Role::parse`, because this is not a process role. It
    // takes an email, which a role does not, and the reason it needs comes
    // from stdin rather than from another argument — see `run_grant_admin`.
    if command.as_deref() == Some(GRANT_ADMIN) {
        let email = args
            .next()
            .ok_or_else(|| anyhow::Error::msg(Role::USAGE))?;
        let config = Config::from_env()?;
        logging::init(config.app_env, &config.log_level)?;
        return run_grant_admin(&config, &email).await;
    }

    let role = Role::parse(command.as_deref()).map_err(anyhow::Error::msg)?;
    // … the rest of run() is unchanged …
```

and, beside `Role`:

```rust
/// The one command that is not a process role.
const GRANT_ADMIN: &str = "grant-admin";
```

- [ ] **Step 3: the command**

```rust
/// Grant the first platform admin, when the platform has none.
///
/// The way back in when there are zero admins — which is a legitimate state,
/// because there is no last-admin guard. Requiring shell access to the server
/// is a higher authority than any admin session, which is what makes it a
/// recovery path rather than a back door.
///
/// The reason is read from **stdin**, never from `argv`. An operational
/// reason is not a secret, but `--reason "granting Budi admin for catalog
/// curation"` lands in shell history and in every `ps` listing on the box.
/// Reading it from the terminal costs nothing and leaks nothing.
///
/// The zero-admin precondition is checked inside `set_role`'s transaction and
/// its lock, not here. Checking it here and then calling would be
/// check-then-act: two operators running this at once would both see zero.
async fn run_grant_admin(config: &Config, email: &str) -> anyhow::Result<()> {
    let pool = adapter::postgres::connect(config.database_url.expose())?;

    let reason = read_reason().await?;

    let mut conn = pool.acquire().await?;
    let target = adapter::postgres::user_repo::find_id_by_email(&mut conn, email)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no account is registered with that address"))?;
    drop(conn);

    let change = usecase::roles::set_role(
        &pool,
        usecase::roles::Actor::Bootstrap,
        target,
        adapter::postgres::user_repo::PlatformRole::Admin,
        &reason,
    )
    .await?;

    match change {
        Some(_) => println!("granted: {target} is now a platform admin"),
        None => println!("no change: {target} is already a platform admin"),
    }

    pool.close().await;
    Ok(())
}

/// Read one line of reason from the terminal.
///
/// `spawn_blocking` rather than a direct read: `apps/api/CLAUDE.md` forbids
/// blocking the runtime, and tokio is built here without the `io-std` feature
/// so there is no async stdin to reach for. One line, and the feature stays
/// out of the dependency list.
async fn read_reason() -> anyhow::Result<String> {
    eprint!("reason: ");
    let line = tokio::task::spawn_blocking(|| {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        Ok::<_, std::io::Error>(line)
    })
    .await??;
    Ok(line)
}
```

The double `?` is deliberate and not a typo: the outer one unwraps `JoinError` from the blocking task, the inner one unwraps the `io::Error` from the read.

`set_role`'s own `check_reason` trims the line and refuses a blank, so there is no second guard here — that is the whole point of the guard living in the use case.

The prompt goes to **stderr** so that piping the command's output somewhere does not capture it, and so `echo "reason" | anakmobil grant-admin budi@example.com` works unattended.

`RoleError` needs to reach `anyhow` at this boundary. It already will: `thiserror`'s derive gives it `std::error::Error`, and `?` into an `anyhow::Result` converts. Confirm rather than assume — if it does not, add `.map_err(anyhow::Error::new)`.

- [ ] **Step 4: the unit tests that pin the argument surface**

Extend the existing `mod tests` in `lib.rs`:

```rust
    #[test]
    fn grant_admin_is_not_a_process_role() {
        // The prohibition, pinned. `Role` models the process a binary IS —
        // web, worker, migrate — and `CONTEXT.md` calls that a property of the
        // deployment rather than of any person. A fourth arm here would have
        // nowhere to put an email or a reason.
        let err = Role::parse(Some(GRANT_ADMIN)).unwrap_err();
        assert!(err.contains("unknown role"));
    }

    #[test]
    fn the_usage_line_names_every_way_to_start_this_binary() {
        // A recovery path nobody can discover is a recovery path that does not
        // exist.
        let err = Role::parse(Some("webb")).unwrap_err();
        assert!(err.contains("web|worker|migrate"));
        assert!(err.contains(GRANT_ADMIN));
    }
```

The three existing `Role::parse` tests must stay green unchanged — that is this task's normal-path regression.

- [ ] **Step 5: run it for real**

```bash
make db-up
make be-migrate
# register somebody through the API first, or insert one directly:
docker compose exec -T postgres psql -U postgres -d anakmobil -c \
  "INSERT INTO users (id, email, password_hash) VALUES (gen_random_uuid(), 'bootstrap@example.com', 'x')"

cd apps/api
echo "bootstrapping the first admin" | cargo run --bin anakmobil -- grant-admin bootstrap@example.com
```

Expected: `granted: <uuid> is now a platform admin`. Then verify each of these and record the output:

1. Running it a **second time** for the same address prints the `no change` line and exits 0 — `set_role` returns `Ok(None)` because the account already has the role. **This is what the fix pass corrected: originally the bootstrap precondition (`admin_count > 0`) was checked before the no-op, and the just-promoted target counted toward its own total, so the second run returned `AdminExists` and exited non-zero.** The precondition now sits after the no-op check and fires only on a real change. Confirm no second audit row: `SELECT count(*) FROM role_changes`.
2. Running it for a **different** address now fails with "the platform already has an admin" — the zero-admin precondition.
3. Demote the admin through `PATCH /admin/users/{id}/role` or directly in SQL, then `grant-admin` a different address succeeds. **This is the recovery path, and it is the whole justification for allowing zero admins** — if it does not work, the no-last-admin-guard decision is unsound and that is a `structural` finding.
4. `anakmobil grant-admin` with no email prints the usage line and exits non-zero.
5. `anakmobil bogus` prints the usage line naming all four invocations.
6. The `reason` does **not** appear in the process's log output. Check by piping stdout and stderr to a file and grepping for the reason text.

- [ ] **Step 6: README**

In `apps/api/README.md`'s "Run it" section, add the command beside `web`, `worker`, and `migrate`:

```markdown
```bash
anakmobil grant-admin <email>    # grant the first platform admin, when there is none
```

Reads the reason from stdin rather than from an argument, because an argument
lands in shell history and in every `ps` listing on the box. Succeeds only when
the platform has zero admins — which is a legitimate state, and this is the way
back from it.
```

### Acceptance criteria

1. `Role::parse(Some("grant-admin"))` is an **error**. The prohibition is pinned by a test, not by a comment.
2. The usage line names all four invocations, and the three existing `Role::parse` tests are green and unmodified.
3. Manual verification 3 in Step 5 — demote, then bootstrap a different address — succeeds. Without it, `grant-admin` is dead code from the second day.
4. `grep -rn "reason" apps/api/crates/runtime/src/lib.rs` shows it only in `run_grant_admin` and `read_reason`, never in a `tracing::` macro.
5. There is no `clap` in `Cargo.toml`.
6. The full gate is `EXIT=0`, and `cargo run -- web` still starts — the dispatch change sits in front of the path every process role takes.

**Block B applies to this task.**

---

## Execution mode

### 1. What runs in parallel, and what is serialised on what

**Every task in this plan is serialised, and the constraint is a shared artifact rather than the task graph.** Say this before dispatching, because six tasks look parallelisable and none of them are.

Three artifacts are shared and none can be held by two writers at once:

| Artifact | Why it serialises | Which tasks touch it |
|---|---|---|
| The local Postgres at `127.0.0.1:55432` — one database, one `_sqlx_migrations` table | Two writers running `sqlx migrate run` produce an interleaved history, and an amended migration leaves every concurrent process holding the wrong checksum. That failed silently across the whole workspace on AM-361 | 1, 2, 3, 4, 5, 6 |
| `apps/api/.sqlx`, regenerated **wholesale** by `cargo sqlx prepare --workspace` | Two concurrent preparations produce a cache reflecting neither branch, and a failed run leaves it empty, breaking the offline build for everybody | 1, 2, 3, 4 |
| The scratch database `anakmobil_prepare`, a **fixed name** that `make be-prepare` and `make be-sqlx-check` both `DROP` and `CREATE` | Two of them running at once destroy each other's database mid-preparation | 1, 2, 3, 4 |

**The AC3 question, asked properly.** Task 1 is the one candidate for genuine parallelism: it needs no migration, and its files (`build_repo.rs`, `usecase/builds.rs`, `http/builds.rs`, `tests/build_list_flow.rs`) do not intersect the role work's files (`user_repo.rs`, `http/auth.rs`, `http/admin.rs`, `usecase/roles.rs`, `lib.rs`, `tests/admin_flow.rs`) at any point. On the file graph it is independent, and it would be wrong to call it serial without saying that.

It is still serialised, and the reason is the third row of that table. Task 1 must run `make be-prepare` and `make be-sqlx-check`; so must Task 2. Both commands `DROP DATABASE anakmobil_prepare` and rebuild it from the migrations on disk. Two of them overlapping is not a merge conflict, it is one process dropping the database the other is mid-way through migrating — and the symptom is a corrupt or empty `.sqlx` cache, which is precisely the failure `make be-prepare` was written to prevent. Fixing that properly means a per-writer scratch database name, which is a change to the `Makefile` and out of this ticket's scope.

So: **serial, by analysis, and the analysis names what would have to change.** If a future plan genuinely needs two SQL writers at once, parameterise `PREPARE_URL` with a per-writer suffix; that is a two-line `Makefile` change and it is the only thing standing between this plan and a parallel Task 1.

**Serialised, and on what:**

- **1 → 2**: `.sqlx` and the scratch database. No code dependency at all; this edge is purely the shared artifact.
- **2 → 3**: Task 3's `PlatformRole` decodes a Postgres type that Task 2 creates. `platform_role_of` does not compile until the column exists.
- **3 → 4**: Task 4's `set_role` calls `platform_role_of` and reuses `PlatformRole`. Both tasks also write `user_repo.rs`.
- **4 → 5**: Task 5 calls `roles::set_role` and matches on `RoleError`. Both tasks also write `http/admin.rs` and `tests/admin_flow.rs`.
- **4 → 6**: Task 6 calls `roles::set_role` with `Actor::Bootstrap`. It does **not** depend on Task 5 — but it shares `.sqlx` with it, so it runs after.

**Files two tasks both write**, which is where a concurrent dispatch would collide even if the artifacts were solved: `src/adapter/postgres/user_repo.rs` (3, 4) · `src/adapter/http/admin.rs` (3, 5) · `src/adapter/http/mod.rs` (3, 5) · `tests/admin_flow.rs` (3, 5) · `apps/api/README.md` (1, 3, 5, 6) · `apps/api/.sqlx/` (1, 2, 3, 4).

**Task 1 goes first despite having no dependents.** It is the only task with no migration, so running it first exercises the whole environment — `db-up`, `be-prepare`, `be-sqlx-check`, `be-test` — before the schema starts moving. AM-361 discovered a migration-checksum drift halfway through its ninth task and the whole suite had been silently green for hours.

### 2. What the writers cannot discover for themselves

**Block A**, verbatim, in every brief. **Block B**, verbatim, in every brief that writes Rust. The five lines in Block A that cost the most if rediscovered rather than told:

1. **`make` runs from the repository root**, and Postgres is on **55432**.
2. **Copy the test harness from `tests/build_list_flow.rs`, not from `tests/part_merge_flow.rs`.** The latter still returns silently when `DATABASE_URL` is unset. A writer told to "follow the existing dialect" and pointed at the merge tests will reintroduce the bug PR #18 closed.
3. **`make be-prepare`, never a bare `cargo sqlx prepare`** — the bare command clears the cache first and prepares against the populated dev database, whose query plans change sqlx's nullability inference.
4. **An applied migration is never edited**, and the four-condition exception has already failed once against the person who wrote it.
5. **`pg_advisory_xact_lock` on a pooled autocommit connection is released before the next statement.** `pool.begin()`, never `pool.acquire()`. That bug shipped once in `usecase::parts::suggest` and its second fix is what made the first one real.

Verified before planning, so no task should re-check it:

- `tests/build_list_flow.rs`'s `app!` returns `(app, pool)`; `tests/garage_flow.rs`'s returns the router alone. Task 3 and 4 need the pool.
- `ApiError` already has `forbidden()`, `not_found()`, `conflict()`, `validation()`, and `internal()`, and `ErrorCode::Forbidden` already renders *"Kamu tidak punya akses ke sini."* — no new error code is needed.
- `shared::response::NoContent` already exists and returns a bodyless 204.
- `usecase::garage::list(pool, owner_id)` already answers "every car this person owns" through the `Vehicle` projection, which has no private field. The admin read needs no new query.
- `request_id::route_label` is private but `MatchedPath` is readable from `parts.extensions` directly; only `UNMATCHED` needs widening.
- Postgres is 17 and the pool is `max_connections(10)`; the concurrency test in Task 4 uses two connections per round and is well inside it.

### 3. Where the risk concentrates

**First — Task 2, the migration.** A column, two constraints, and a trigger, all of which later tasks build on. The specific failure to hunt: if the foreign keys end up `ON DELETE SET NULL`, the trigger converts every account deletion into a hard failure, and **nothing in the Rust suite would catch it** because nothing in this ticket deletes an account. That is why Step 5's assertions 6 and 7 pin the SQLSTATE rather than merely asserting a refusal — a `SET NULL` schema also refuses, with a different error, for the opposite reason. This project has shipped that defect class once.

**Second — Task 4, `set_role`.** An authorization boundary and a concurrency guard in one function. Two of its properties cannot be pinned by any test in this suite and are named in the plan rather than papered over: the ordering of the audit insert against the column update (both inside one transaction, so no observer can distinguish), and the bootstrap count's position inside the lock (a two-process race that is not reproducible in-suite). Those are the reviewer's job. The concurrency test runs twenty rounds because a race that fails one time in ten passes a single run and reports itself fixed.

**Third — Task 3, the `Admin` extractor.** A public contract and the platform's only authorization check. The failure to hunt is not "does it refuse a non-admin" — that is tested — but whether any path can produce a role from something other than the database: an `unwrap_or`, a default, a cached value on `AppState`, or a `_` match arm that a future third variant would fall into. `make be-lint` catches none of those.

**Fourth, and not in the ticket's framing — Task 1's `CASE` expression.** It is the only change in this plan that alters an already-shipped response contract, and it moves a filter from a place the compiler was policing into a place no compiler polices. `visible_cost`'s Rust `match` made adding a `visibility` variant a compile error; the SQL `IN` list does not. The comment in Step 1 says so out loud, because a silent downgrade of a compile-time guarantee to a documented convention is the kind of thing that is only ever noticed by the person it later bites.

### 4. What this plan found by reading the code, that the spec does not state

Said plainly rather than improvised around.

- **`shared/validation.rs` holds only a decimal guard.** The brief that produced this plan said to reuse it for `reason` and not to write a second validator. There is nothing there to reuse: the file contains `DecimalSpec`, `decimal()`, and `OUT_OF_RANGE`, and the text-length pattern lives privately in `adapter/http/parts.rs` as `MAX_BRAND`, `MAX_PRODUCT_NAME`, and `too_long()`. Extracting a shared `validation::text` helper was considered and rejected: it accumulates into a `serde_json::Map` for an `ApiError`, and the CLI entrance has no `ApiError`. So `reason`'s guard lives in **`usecase::roles::set_role`**, which is the only place both entrances pass through — the same lesson as AM-361's finding that the parts queue had two entrances and a limit on one. This is a better answer than the brief's, and it is written down so nobody "corrects" it back.

- **Two integration test harnesses still skip silently, and PR #18's completion note says all four guards are loud.** `tests/part_merge_flow.rs`'s `pool!` macro returns when `DATABASE_URL` is unset, and `tests/session_store.rs` returns when `REDIS_URL` is unset or unreachable — no `AM_SKIP_INTEGRATION` assertion in either. `apps/api/CLAUDE.md` says "All four are loud now", which is true of the four *branches* inside `app!` and false of these two *files*. **No task in this plan fixes them** — two unrelated harnesses is scope this ticket did not ask for — but Block A names them so no writer copies from either, and this belongs in a follow-up. It is roughly a six-line change per file.

- **`BuildRow.owner_id` and `BuildRow.cost_visibility` become unread when the cost filter moves into SQL**, and neither produces a compiler warning, because `BuildRow` is a `pub` struct in a library crate. Task 1 removes both. The spec does not mention them.

- **The admin vehicle read must not populate `summary`.** `ListSummaryResponse` carries `total_cost`, which is service spend, which `CONTEXT.md` names as private vehicle data that never leaves the server "for anyone who should not see it, including an admin". `GET /vehicles` fills it in because that endpoint answers to the owner. The spec's AC4 section names plate, VIN, and purchase price and does not mention spend; the test in Task 3 asserts the field is absent.

- **The AC4 assertion is written against the whole serialised body, not field by field.** A field-by-field check passes on a response that renamed the field, and it is the *values* that must never travel. The spec's phrasing ("carries none of `plate`, `vin`, `purchase_price`") reads as a field check.

- **`RoleError::AdminExists` is unreachable from the HTTP handler** — only `Actor::Bootstrap` produces it. It is mapped anyway, because the match must stay exhaustive so a new variant is a compile error. `unreachable!()` is not available: `clippy::panic` is denied workspace-wide.

- **`grant-admin` run twice for the same address must hit the no-op branch, not the bootstrap precondition** — and in the original implementation it did NOT, which the fix pass corrected. `set_role` checked `admin_count > 0` inside the actor match, before reading the target's role, and the target counts toward its own total; so the second run returned `AdminExists` and exited non-zero instead of printing "no change" and exiting 0. The fix moves the bootstrap precondition to after the no-op check and gates it on a real change. This bullet originally claimed the correct behaviour as if it were the shipped behaviour — it was the intended behaviour, and the gap was found only because Task 6 exercised a path no test covered. Pinned now by `re_granting_the_sole_admin_is_a_safe_no_op_not_a_failure`.

- **The `Ok(None)` no-op path leaves the transaction uncommitted.** Dropping a `Transaction` rolls it back and releases the advisory lock, which is correct and is why the plan does not call `tx.rollback()` explicitly — but it is the kind of implicit behaviour a reviewer should confirm rather than assume, especially since the lock's release depends on it.

- **A 500 from the `Admin` extractor cannot be integration-tested cheaply.** Simulating an unreachable pool inside the suite costs more than it proves. Task 3's acceptance criterion 3 states this as a reviewer check rather than pretending a test covers it, because a criterion that claims coverage it does not have is worse than an admitted gap.

---

## Execution status

### Ready-queue map, built before task 1 and not re-derived

Two edges only: **produces → consumes**, and **shared artifact**. Everything with no edge between it runs together — which here is nothing.

```
1 ─(.sqlx, scratch db)─> 2 ─(schema)─> 3 ─(PlatformRole, user_repo.rs)─> 4 ─┬─(RoleError, admin.rs)─> 5
                                                                            └─(Actor::Bootstrap)────> 6
```

Tasks 5 and 6 have no code edge between them — 6 does not call anything 5 produces — but they share `apps/api/.sqlx` and `apps/api/README.md`, so 6 follows 5.

**The chain is real, and it is not the task graph that causes most of it.** Task 1 has no code dependency on anything; it is serialised purely by the shared `.sqlx` directory and the fixed-name scratch database. Task 6 is serialised behind 5 for the same reason. The genuine code edges are 2→3→4→{5,6}.

| Task | Status | Notes |
|---|---|---|
| 1 AC3 — the cost filter moves into the query | not started | |
| 2 The migration — `platform_role`, the column, `role_changes` | not started | |
| 3 `PlatformRole`, the `Admin` extractor, the admin vehicle read | done | All 5 tests pass; all three sabotages reddened only their named test. `PrivateResponse`'s fields nest under a `private` object — the plan's Step 6 test body put `plate`/`vin`/`purchase_price` at the top level of the `POST /vehicles` request, which `VehicleRequest` does not accept; corrected against `tests/garage_flow.rs`'s `a_car_with_a_plate`. AC criterion 3 (no non-database path to a role) verified by reading `Admin::from_request_parts`, not by a test — no `unwrap_or`, no default, no cache, exhaustive match with no `_` arm. AC criterion 4 (log line) verified by running `an_ordinary_account_is_refused_from_an_admin_route` with a subscriber attached: exactly one `WARN`, `admin route refused user_id=... route="/admin/users/{id}/vehicles"`, nothing else. |
| 4 `usecase::roles::set_role` | not started | |
| 5 `PATCH /admin/users/{id}/role` | not started | |
| 6 `anakmobil grant-admin <email>` | not started | |
| Fix pass | not started | |

### Finishing, and the two steps most often dropped

Beyond the usual — consolidate the ledger, fix pass, final gates, Artifact, show the owner, commit, push and watch CI to green:

1. **Tell AM-366 and AM-89 that the admin vehicle read landed here.** `GET /admin/users/{id}/vehicles` belongs to AM-366 (backoffice API) serving AM-89 (user management, E13-6), and it was pulled into this ticket so AC4 had something that could actually fail. **If nobody tells them, it gets built twice** — the failure AM-361 already paid for with AM-88. Comment on both, naming the route and this plan.
2. **The three open threads carry forward** and belong in the pull request description rather than being closed by it: retention has no stated bound (AM-296), `/admin/*` has no rate limit (AM-356), and `apps/api/CLAUDE.md`'s claim that all four loud-skip guards are closed is true of the four branches in `app!` and false of `tests/part_merge_flow.rs` and `tests/session_store.rs`.
3. **Refresh the knowledge graph** — `graphify update .` — unless a commit hook already did it. This plan adds three files and a module; a later session orienting from a stale map is orienting from a codebase that no longer exists.

---

## Review findings ledger

| Task | Severity | Where | What breaks | Smallest fix | Status |
|---|---|---|---|---|---|
| 1 | `test-integrity` | `tests/build_list_flow.rs:920`, `:958`, and pre-existing at `:502` — **and the plan's own Step 5 block, which is where it came from** | `serde_json`'s `Index<usize>` returns `Value::Null` for an *empty array*, so `build["modifications"][0]["cost"]` is `Null` when there are no modifications at all. The private branch therefore cannot tell "the cost was nulled" from "the modification vanished". The mutation this admits is a natural-looking simplification of the very expression this task just wrote: move the `CASE` condition into the `WHERE`, and a stranger stops seeing a private-cost car's modifications **entirely** rather than seeing them without prices. Every assertion still passes. The community silently loses the ability to see which parts are fitted — the exact thing "the build is visible, the money is not" exists to preserve. | Assert the row survives before asserting its cost is null: `let mods = build["modifications"].as_array()...; assert_eq!(mods.len(), 1, ...)`. Two lines, twice. | **closed — fix pass.** All three sites (`:502`, `:920`, `:958`) now assert `mods.as_array().len() == 1` before asserting the cost is null. |
| 1 | `hygiene` | `adapter/postgres/build_repo.rs:405-407` | The doc comment on `BuildRow` still promises "who owns it … and both visibility settings"; both fields were removed in this task. A comment describing a mechanism that has moved is the shape AM-361 logged three findings against — it sends the next reader to the wrong place, and the wrong place is where a profiler stops looking. | Rewrite the sentence to name only what the struct still carries. | **closed — fix pass.** Comment now says the row carries variant + name, and that ownership/cost-visibility moved to the query. |
| 1 | `hygiene` | Plan acceptance criterion AC1 | Written as "`grep -rn visible_cost apps/api/` returns nothing", which is unmeetable: two hits remain and the plan's own Step 1 and Step 5 prescribe both, verbatim, as historical comments. A criterion that cannot be met reads as a failure forever. | Reword AC1 wording. | **noted; not done.** Pure plan-text cosmetics (the acceptance criterion's grep phrasing) — verified separately that `modifications_for` has exactly two call sites and neither post-filters, so the property holds. Not worth a deep plan edit; recorded so it is not mistaken for unaddressed. |
| 1 | `hygiene` | Plan Step 4 (line ~401) and Step 5 (line ~429) | Two plan statements the writer had to deviate from, correctly, and neither was recorded: `sqlx::types::BigDecimal` is **not** still needed in `adapter/http/builds.rs` once `visible_cost` is gone, and the `PUT /vehicles/{car}` body must repeat `described_as` or `VehicleRequest::check()` returns 422 on every setting. | Correct the `BigDecimal`/`described_as` notes in the plan. | **noted; not done.** Plan-text cosmetics — the code is correct (the deviations were the right call and shipped); only the plan's step prose still describes the pre-deviation shape. Low value; recorded honestly rather than marked closed. |
| 1 | `hygiene` — pre-existing, out of scope | `adapter/postgres/build_repo.rs`, `BuildRow.variant_id` | Now the only unread field on the struct, for the same reason the other two were removed. Not introduced by this task. | Remove it, or say why it stays. | noted; not this ticket |
| 2 | `test-integrity` | The suite as a whole — no file pins it | **Nothing in `make be-test` proves `ON DELETE RESTRICT`.** Assertions 6 and 7 were executed once, by hand, and their proof now lives only in this ledger. Two migrations earlier, `part_merges.merged_by` is `ON DELETE SET NULL` *with a comment arguing for it*, so a later reader harmonising `role_changes` to match reads as tidying — and all four gates stay EXIT=0. Account deletion then fails in production the first time the deleted user has role history. This is the defect class the project shipped once. | One `#[sqlx::test]`: insert a `role_changes` row, `DELETE FROM users`, assert the SQLSTATE is `23503` and **not** the append-only `P0001`. ~12 lines. Belongs in Task 5's test file — the migration can no longer be amended, because `apps/api/CLAUDE.md`'s four conditions require nothing else running against the database, and Task 3 was. | fix pass |
| 2 | `hygiene` | `migrations/20260817133711_platform_role.up.sql:74` | `role_changes_target_idx` is `(target_user_id, created_at DESC, id)` — `id` ascending. AM-366's natural keyset cursor for "newest first" is `ORDER BY created_at DESC, id DESC`, which this index can serve in neither scan direction, so AM-366 inherits an unbudgeted sort node. Nearly decorative today: `id` is UUIDv7 and same-microsecond writes for one user are serialised behind the advisory lock anyway. | `id DESC`. Costs nothing now, costs a migration later. | **deferred — next migration.** Amending the applied migration on the shared dev DB would trip the four-condition rule that already bit this project twice, and the tiebreaker is near-decorative until AM-366 writes the cursor. Recorded for whoever writes that migration. |
| 2 | `hygiene` | `migrations/20260817133711_platform_role.up.sql:91` | `TRUNCATE` fires neither `UPDATE` nor `DELETE` triggers, so `TRUNCATE role_changes` — or `TRUNCATE users CASCADE` — wipes the history the trigger claims to protect. Not exploitable: nothing in the repo issues a `TRUNCATE`, and the privilege needed is the table owner's, which could equally `DROP TRIGGER`. It is a gap in the sentence "the guarantee is in the schema", not a hole in a defended boundary. | Say so in the comment. | **deferred — next migration.** Same reason: the comment lives in the applied migration file, and editing it changes the checksum. Noted here so the next migration touching `role_changes` adds the `TRUNCATE`-is-uncovered line. |
| 2 | `hygiene` | `migrations/20260817133711_platform_role.down.sql:8` | The comment says dropping the type before the column "leaves the revert half-applied". sqlx wraps each migration in a transaction and PostgreSQL has transactional DDL, so a failed revert rolls back whole. Ordering is still correct; only the justification overstates. | Reword the down-migration comment. | **deferred — next migration.** Cosmetic, in the applied migration file; batched with the two above. |
| 2 | `structural` — **noted, no change** | The schema, and what it cannot enforce | Recorded because it makes Task 4's review load-bearing rather than belt-and-braces. The schema **cannot** guarantee the audit row and the column update are atomic — a `role_changes` row can exist with `users.platform_role` never updated, and the reverse. Only `set_role`'s transaction makes the pair true. Likewise the `CHECK` catches `from_role = to_role` but **not** two concurrent promoters each writing an individually-valid row; there is no unique or exclusion constraint that would catch a lost update if the advisory lock were forgotten. | None here. Task 4's reviewer must verify the transaction boundary and the lock **directly**, not assume the database would notice. | **carried into Task 4's review brief** |
| 4 | `test-integrity` — **plan text was incomplete** | Plan Task 4 sabotage table | The table says deleting the `from_role == to_role` no-op branch reddens only `setting_the_role_somebody_already_has_...`. The writer proved it reddens **two** tests, because the concurrency test's losing racer legitimately lands on the no-op path under the lock — remove the branch and both go red. A sabotage table that under-states a guard's blast radius makes the guard look weaker than it is, and the next reader trusts it. | Plan table corrected to name both tests. | **plan text fixed this turn** |
| 4 | `correctness` — **noted, review property not a tested one** | `usecase::roles::set_role`, the audit-insert-before-column-update order | Swapping the insert and the update kills **no** test — confirmed empirically, matching the plan's own prediction. Both are inside one transaction, so no observer distinguishes them, which is exactly why the ordering is a review property. It is correct in the code (insert precedes update, a failed insert fails the change); recorded so a future edit that reorders them is caught by reading rather than by a green suite that would stay green. | None — the code is right. The record is the safeguard. | **noted; correct as written** |
| 4 | `hygiene` | `usecase/roles.rs` — `admin_count` | `O(U)` sequential scan over `users`, no supporting index. Correct today: `grant-admin` runs perhaps twice in the platform's lifetime, and the `ponytail:` comment names the upgrade path (`CREATE INDEX users_admins_idx ON users (platform_role) WHERE platform_role = 'admin'`) for if it ever reaches a hot path. Recorded so the comment is not mistaken for an oversight. | None now. | **noted; correct as written** |
| 3 | clean | The authorization floor | Reviewer found no structural finding and no private field on the response, across five hunts and nine lenses (defensive + offensive). Three trivial non-blocking notes: `admin::to_api_error` duplicates `vehicles::to_api_error` (deliberate, keeps the exhaustive match independent per module); two sequential pool acquisitions per admin request (never simultaneous, not a leak); the admin extractor is not rate-limited (AM-356, already an anti-goal). Checked, holds. | — | **closed — clean** |
| 4 | clean | `usecase::roles::set_role` | Reviewer verified the load-bearing guarantees directly: lock on `pool.begin()` not `acquire()`, both re-reads under the lock, audit insert before update and atomic with it, `reason` guard in the use case so the CLI inherits it, no-op path releases the lock by RAII rollback. The concurrency test asserts both `changed == 1` and `rows == 1`, so it cannot pass with two rows. One non-blocking note: the under-lock `NotAdmin` refusal writes no `tracing` line where the extractor-level one does — reachable only by a was-valid admin, not a probing surface, so low value; left to the owner. | — | **closed — clean** |
| 5 | `test-integrity` | `tests/admin_flow.rs::a_non_admin_is_refused_before_any_lookup_happens` | The plan predicted swapping the handler's `Admin` extractor for `Authenticated` would redden this test. It **survived** — because `set_role` re-checks the actor's admin status under the lock (Task 4's defense-in-depth), so a non-admin still gets 403, just from the use case instead of the extractor, and the test asserts only the status code. The property in the test's name ("before any lookup") is real but not observable through a black-box status assertion. Not a defect — it is belt-and-suspenders working — but the test does not pin what its name claims. | Either rename the test to what it actually asserts, or add a white-box check. | **closed — fix pass.** Renamed to `a_non_admin_gets_403_and_the_endpoint_is_not_a_user_id_oracle`; comment now states the outcome is pinned, not the layer. |
| 5 | `test-integrity` | `PATCH /admin/users/{id}/role`, the 401 case | The spec's contract table has a 401 row (caller not signed in). The route inherits 401 from `Admin`→`Authenticated`, and the sibling `GET` route has a 401 test — but there is no 401 test on the PATCH route itself, so a future change breaking PATCH's 401 independently would stay green. The writer followed the plan's literal Step-4 append block, which omitted it, rather than inventing a test under a TDD-no verdict — the right call, flagged for a conscious decision. | Add one 401 test on the PATCH route. ~6 lines. | **closed — fix pass.** `an_unauthenticated_patch_is_401_on_the_role_route_itself`. |
| 6 | **`correctness` — real bug, found only because the CLI exercised a path no test did** | `usecase/roles.rs:135-144` (Task 4's file) + `user_repo.rs` `admin_count` | The `Ok(None)` no-op branch is **structurally unreachable via `Actor::Bootstrap`**. `set_role` checks `admin_count(&mut tx) > 0` unconditionally for bootstrap, before the `from_role == to_role` no-op check, and `admin_count` counts the target itself. So the instant a bootstrap grant succeeds, the target contributes to the count, and running `grant-admin <same-email>` again returns `AdminExists` (Error, exit 1) instead of the documented "no change", exit 0. The no-op branch can only ever fire on the HTTP path. **Untested:** `role_change_flow.rs`'s no-op test uses `Actor::Admin`, and its bootstrap-refusal test uses a target that is not the existing admin — the exact case (bootstrap, target IS the sole admin) had no coverage until the manual CLI run. Low impact (a misleading error on an accidental re-run, no corruption, no security issue) but a real idempotency gap in the one path whose entire purpose is being the reliable way back in. | Reorder per the reviewer's shape (auth first, no-op check, THEN bootstrap precondition on a real change only). | **closed — fix pass, first item.** Done exactly that: `Actor::Bootstrap => None` in the match, precondition moved after the no-op check and gated on `matches!(actor, Bootstrap)`. New test `re_granting_the_sole_admin_is_a_safe_no_op_not_a_failure` proven load-bearing — reddens against the old order (FAILED at line 283), green with the fix. Plan text at 2639 and 2751 corrected. Full gates EXIT=0, 336 tests. |
| fix pass | clean | `usecase/roles.rs` reorder | Independent review confirmed all four floor properties after the bootstrap reorder: the `Actor::Admin` re-read stays FIRST and under the lock (a demoted admin's no-op PATCH still hits 403, never a 204 oracle); the bootstrap `admin_count` stays between `lock_platform_role` and `commit`; the no-op path drops the uncommitted tx and releases the lock; `Actor: Copy` so `matches!` after the match compiles. The new test reddens against the old order (FAILED at line 283). security-coverage 9/9, rust-review clean, no Big O regression. | — | **closed — reviewed clean** |
| fix pass | `hygiene` — process, not code | The reviewer's scoped diff | The `fix-pass.diff` I snapshotted used `git diff HEAD`, which omits UNTRACKED files — and `roles.rs` (the load-bearing subject) is a still-untracked Task-4 file. The reviewer caught this and read the on-disk source instead, so the review stands, but a reviewer trusting the artifact would have reviewed only the two tracked files and missed the bootstrap reorder. **Lesson: snapshot reviewer diffs with `git add -A && git diff --cached`, never bare `git diff`, whenever a task's files are new.** | Use the staged diff for reviewer snapshots. | **noted; binding on future snapshots** |
| later | `correctness` — pre-existing, follow-up | `build_repo.rs`, `page_visible`'s `WHERE b.visibility <> 'private'` | Not introduced by AM-355, but this fix pass is where it became visible next door. The new cost `CASE` uses `IN ('community','public')` — fail-**closed**: a future `Visibility` enum variant hides cost. But `page_visible`'s build-visibility filter is `<> 'private'` — fail-**open**: a new variant would make such builds visible to everyone. Same enum, opposite safety default. | Change `page_visible` to an allow-list `IN (...)` when a third visibility variant is next touched. Worth its own ticket; out of AM-355's scope. | **filed — AM-369** |

Severity vocabulary: `structural` (a column, constraint, or public contract — raised and fixed immediately, because later tasks stand on it) · `correctness` · `test-integrity` · `hygiene`. Everything but `structural` is worked in one fix pass after the final task.
