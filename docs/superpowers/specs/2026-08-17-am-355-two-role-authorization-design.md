# AM-355 — Two-role authorization, an audit trail, and cost filtering in the query

Ticket: [AM-355](https://oksasatyaa.atlassian.net/browse/AM-355) · Epic E16 (Backend) · design approved 2026-08-17

The platform has no concept of a role. Every endpoint that exists is either public or "the signed-in person, acting on their own data", and that has been enough until now. It stops being enough at the next ticket: the curation queues filled by AM-360 and AM-361 have nobody who may work them, `usecase::part_merge::{merge, unmerge}` is complete and reachable by no route, and the whole of E13 waits on an admin existing at all.

This spec adds the smallest thing that makes an admin real: a role on the account, a way to check it that cannot be forgotten, an append-only record of every change to it, and — because the same ticket owns the promise — the move of one cost filter out of Rust and into SQL.

## Decisions

### The role lives on the account, and is called `platform_role`

A Postgres enum `platform_role` with two values, `user` and `admin`, and a column `users.platform_role platform_role NOT NULL DEFAULT 'user'`.

**The name is doing work.** `runtime::Role` already exists and means the process — `Web | Worker | Migrate`. Community membership will bring a third sense of the word: owner, admin, and member *within one community*, which grants nothing platform-wide. Three unrelated concepts, three names, never one column used for two of them. `CONTEXT.md` records all three so the next reader does not have to rediscover the distinction.

The admin role cannot be reached through registration: the column defaults to `user` and no sign-up path writes it.

**The pure-domain copy is deliberately not built.** `ServiceCategory` exists twice — once in `domain`, once in `adapter/postgres` with the sqlx derives — because a domain policy function consumes it. Nothing in `domain` consumes a role. Two variants and no policy function is not a domain model, it is a second place to keep in sync. The adapter carries the only copy; when a policy function earns it, the split is additive.

### `Admin` is an extractor, not a line in a handler

A second `FromRequestParts` beside the existing `Authenticated`. It resolves the Redis session exactly as `Authenticated` does, then reads `users.platform_role` from Postgres — fresh, on every admin request.

This mirrors what `Authenticated` already says about itself: *"the check is in the type, not in a line of code somebody has to remember to write first."* A handler that takes `Admin` cannot be routed without one. With one admin endpoint today the extractor looks like ceremony; with AM-366's queues it is the only thing that scales, and adding it later means auditing every route written in between.

**Failure is closed.** A database error reading the role is a 500, never "assume `user`" and never "assume `admin`". A non-admin receives 403 with a clear message rather than 404 — AM-84's AC2 asks for a rejection the person can understand, and hiding the endpoint's existence from an authenticated account buys nothing.

**Ordinary routes pay nothing.** They keep `Authenticated`, which touches only Redis. The extra Postgres read exists only on routes that need a role.

### AC1 means the next *admin* request, and sessions are not revoked

The ticket says a revoked role must be rejected on the next request without waiting for a new token. Read literally that would also reject the person's ordinary requests, which is wrong: a demoted admin is still a user, and their garage still belongs to them.

So: **a demoted admin's next admin request is rejected; their ordinary requests are unaffected.** The freshness requirement is satisfied because nothing caches the role — it is read from the source on every admin request.

**Demotion does not revoke sessions.** The Redis session maps a token to a `user_id` and carries no privilege, so there is no stale authority to invalidate. Revoking would sign the person out of the mobile app as a side effect of an unrelated administrative change.

**One boundary is accepted rather than solved.** A role revoked *while a request is in flight* cannot un-authorize that request — but the mutation re-reads the actor's role under the lock (below), so the window is the request's own transaction rather than its whole lifetime.

### `role_changes` is append-only, enforced by the schema

```sql
CREATE TABLE role_changes (
    id             UUID PRIMARY KEY,
    actor_id       UUID NULL     REFERENCES users(id) ON DELETE RESTRICT,
    target_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    from_role      platform_role NOT NULL,
    to_role        platform_role NOT NULL,
    reason         TEXT          NOT NULL,
    created_at     TIMESTAMPTZ   NOT NULL DEFAULT now(),
    CONSTRAINT role_changes_real_change CHECK (from_role <> to_role)
);

CREATE INDEX role_changes_target_idx ON role_changes (target_user_id, created_at DESC, id);
CREATE INDEX role_changes_actor_idx  ON role_changes (actor_id) WHERE actor_id IS NOT NULL;
```

plus a `BEFORE UPDATE OR DELETE` trigger that raises.

**The trigger is the point.** AM-361's ledger carries a finding that `part_merges` claims to be append-only in a comment while an `UPDATE` was verified to rewrite its history. A comment is not a constraint. Here the guarantee is in the schema from the first migration.

**`ON DELETE RESTRICT`, and the reasoning changed twice getting here.** The first design used `ON DELETE SET NULL` so the trail would survive account deletion. That is a hard contradiction: PostgreSQL executes a referential action as an ordinary `UPDATE`, the trigger rejects it, and the entire `DELETE` rolls back — account deletion becomes impossible, which is the exact defect class this project has already shipped once. The obvious fix was to drop the foreign keys, and that is the convention for audit tables. What changed it was ADR-0001: once accounts are retained rather than erased, `RESTRICT` is strictly better than no key at all. It never writes to the child row, so the trigger never sees it; it guarantees every audit row points at a user who exists; and a future hard-erasure path meets a refusal and has to make a conscious decision instead of silently orphaning the trail.

**Identity comes from a join, not a copy.** An earlier round stored the actor's name and email so the trail stayed readable after deletion. `RESTRICT` makes the row unconditionally present, so a join always resolves and the copies lose their reason to exist. The remaining argument for them — a point-in-time record that does not change if somebody later edits their email — was weighed and dropped: it is not worth a second, unerasable copy of an address in a two-role system. `users` also has no name column at all today. If evidentiary faithfulness ever matters, the columns are additive.

**Both foreign keys are indexed**, per this repository's own rule that PostgreSQL indexes primary keys and unique constraints but never a foreign key. The target index carries `created_at DESC, id` because the only query that matters is "this person's role history, newest first". The actor index is partial because bootstrap rows have no actor and there is no query for them.

### Two write paths, one use case, one transaction

```
anakmobil grant-admin <email>      → actor_id NULL, only when the admin count is zero
PATCH /admin/users/{id}/role       → actor_id is the calling admin
                    ↓ both
        usecase::roles::set_role()
```

One transaction, in this order:

1. `pg_advisory_xact_lock(hashtext('platform_role'), 0)`
2. re-read the **target's** role under the lock — and, on the HTTP path only, the **actor's** role as well. The CLI path has no actor to re-read; what it re-reads instead is the admin count, which is its own precondition.
3. reject: nothing to change → no-op; actor is no longer an admin → the same 403 the extractor would have given; CLI and the admin count is no longer zero → refuse and say so
4. insert the `role_changes` row
5. update `users.platform_role`

**The lock's job is to make the audit row true.** Without it, two admins promoting the same person concurrently both read `from_role = user`, both write a row claiming `user → admin`, and one of those rows is a lie about a change that did not happen. It is not protecting a last admin — there is no last-admin rule.

**The two-argument key is deliberate.** `pg_advisory_xact_lock(key, 0)` occupies a different keyspace from the single-argument locks already in use — `hashtext('part_merge')` in `part_repo`, and the per-person allowance lock in `usecase::parts`. A single-argument and a two-argument lock cannot collide even on the same hash.

**Step 2 is the finding that a checklist and a second model each found half of.** Re-reading the *target* under the lock is the obvious half. Re-reading the *actor* is the half that closes a real hole: the extractor checked the role before the handler ran, so an admin demoted in between would still complete the mutation they had already started.

**The audit insert precedes the update, and its failure fails the change.** A privilege that exists with no record of how it was granted is worse than a privilege that failed to be granted.

**`grant-admin`'s zero-admin check happens inside the same lock**, not before calling `set_role`. Counting admins and then inserting across two statements is check-then-act: two operators running the command concurrently both see zero and both succeed. That is the same defect the AM-361 fix pass closed twice, and it does not get to appear a third time.

**The reason is read from stdin, not from `argv`.** An operational reason is not a secret, but `--reason "granting Budi admin for catalog curation"` lands in shell history and in every `ps` listing on the box. Reading it from the terminal costs nothing and leaks nothing.

### Zero admins is a legitimate state

There is no rule preventing the last admin from being demoted, and none preventing an admin from demoting themselves.

The alternative was a last-admin guard, and it collapses on contact with the bootstrap rule: if `grant-admin` only succeeds at zero admins and nothing may ever reach zero, then `grant-admin` is dead code from the second day and there is no recovery path at all. Allowing zero makes the CLI permanently useful as exactly what it is — the operator's way back in, requiring shell access to the server, which is a higher authority than any admin session.

It also dissolves a whole finding: with no last-admin invariant there is nothing to enforce in the account-deletion path, which does not exist yet and would otherwise have needed to know about roles.

### The response contract

| Situation | Response |
|---|---|
| Role changed | `200` with `target_user_id`, `from_role`, `to_role`, `created_at` |
| Already in that role | `204`, nothing written |
| Target does not exist | `404` |
| Caller is not an admin | `403`, before any lookup |
| Caller was demoted between the extractor and the lock | `403`, same code and message as above |
| Caller is not signed in | `401` |

**`204` rather than an error is what makes a retry safe.** A dropped connection after a successful `PATCH` leaves the client unsure; retrying hits the no-op branch and succeeds. Without an explicit branch the `CHECK` constraint fires and surfaces as a generic 500 — a correct database rejecting a reasonable client.

**The response carries no email and no reason.** It answers what changed, not who anybody is.

**403 before lookup is what stops the endpoint being a user-id oracle**, and it is automatic: the extractor runs before the handler, so a non-admin never reaches a query. An admin receiving `404` for a missing id is not a leak — they are authorized to see the user list.

**Self-demotion is allowed explicitly.** It is a normal thing for a person to do, and with no last-admin rule there is nothing it can break.

### A rejected admin request leaves a trace

A non-admin reaching an admin route is logged at warning level with the `user_id` and the matched route pattern — nothing else, per this repository's rule about what a log line may contain. `apps/api/CLAUDE.md` already settles the privacy question: *"A user id is not a credential and is enough to investigate."*

Without it, probing for admin endpoints is indistinguishable from silence, on precisely the routes an attacker would probe. The audit table is not the place for this: it records changes that happened, and diluting it with attempts that did not would make it stop being read.

**Nothing on this path logs an email or a reason.** The AM-361 fix pass had to remove a caller-supplied `brand` from a log line for exactly this reason; the same mistake is available here and is closed by saying so.

### AC3 — the one query that actually leaks

`modifications_for(conn, build_ids)` returns every row's `cost` unfiltered and relies on its caller to null it. Its own documentation admits this: *"Every row carries `cost`, unfiltered."* The filtering then happens in `visible_cost` in `adapter/http/builds.rs`, in Rust, after the number has already been read out of the database and into the process.

It gains a `viewer_id`, and the cost becomes:

```sql
CASE WHEN v.owner_id = $viewer OR v.cost_visibility IN ('community','public')
     THEN m.cost ELSE NULL END
```

Still one query for any number of builds, still no N+1 — `builds.vehicle_id` is unique, so the join multiplies nothing. `visible_cost` is deleted.

**The five sabotage-proven tests that pinned `visible_cost` are rewritten, not discarded.** They were each verified in AM-361 to go red when the behaviour they describe is broken, which makes them worth more than their replacements would be if written fresh.

**Service costs were audited and need no change.** Every service query and every summary rollup already carries `WHERE v.owner_id = $1`, so no stranger reads a service record at all and the cost never travels. This is stated because "we checked and it was already correct" is a finding, and the next reader should not have to check again.

### AC4 — an admin read endpoint, pulled in deliberately

`GET /admin/users/{id}/vehicles` — admin-only, listing a person's vehicles with no `plate`, no `vin`, no `purchase_price`.

**This is scope the owner added with the trade-off stated.** The endpoint belongs to AM-366 (backoffice API) serving AM-89 (user management, E13-6). Without it, AC4 has nothing to prove: no admin path reads a vehicle today, so a test asserting "the admin response contains no plate" would pass by receiving a 404 — an assertion that cannot fail, which is the defect class this project's reviewers have caught nine times in one ticket.

The alternative offered was to declare AC4 satisfied structurally and make the enforcing test a requirement of whichever ticket adds the first admin read. The owner chose to build it here so the criterion is genuinely locked. **AM-366 and AM-89 must be told this landed early, or it gets built twice** — the failure AM-361 already paid for with AM-88.

The endpoint reuses `find_private`'s discipline in the negative: it never calls it. The public vehicle projection has no field for a plate, a VIN, or a price, so redaction is structural rather than a filter somebody remembered to apply.

## Tidak boleh ada

- **No third role, and no per-feature permissions.** Two roles, and the community-membership concept stays entirely separate.
- **No RLS, no `SET LOCAL`.** This is not multi-tenant; isolation is per-user through query predicates and use-case authorization, and the ticket says so.
- **No general `audit_log` table.** A shape that cannot record a role change — no `from_role`, no `to_role` — is not a foundation for AM-366; that ticket designs one from real needs.
- **No `part_merge` endpoints.** They belong to AM-88, and pulling a destructive, transitive operation into an authorization ticket mixes two acceptance surfaces.
- **No backoffice session TTL.** AM-84, and it is a frontend concern.
- **No rate limiting on `/admin/*`.** AM-356 owns it. Recorded as a known gap: an authenticated non-admin can probe the route repeatedly, each attempt costing one indexed read and now leaving a log line.
- **No `deleted_at`, no partial unique index, no sign-in filter.** ADR-0001 decides the policy; AM-296 builds it.
- **No pure-domain `PlatformRole`.** Until a policy function consumes it.

## Verification

```
make be-lint            # fmt + clippy -D warnings
make be-boundary        # domain crate imports no framework
cargo sqlx prepare --workspace && git add apps/api/.sqlx    # migrations and queries changed
make be-sqlx-check
make be-test
```

`cargo sqlx prepare` is not optional here: a new table, a new enum, and a changed `modifications_for` all move the offline cache, and a stale cache passes locally and fails in CI.

**Two migration facts that will bite otherwise.** The enum type and the column can ship in one migration; the down migration must drop the column before the type, or the type drop fails with dependent objects. And `role_changes`'s trigger must be created after the table, in the same migration, or a partial apply leaves an unguarded table.

## TDD verdict

**`usecase::roles::set_role` — yes.** A clear input→output contract with three branches (no-op, actor no longer admin, real change) and an ordering requirement inside a transaction. The failing test comes first.

**The migration, the extractor, and the admin read endpoint — no.** Verified by running them: the migration by apply → revert → re-apply, the extractor and endpoint by integration tests written immediately after, including the normal-path regression test.

**One test is required and is the reason AC4 was expanded:** an admin session, another person's vehicle, asserting the response body carries none of `plate`, `vin`, `purchase_price` — and asserting the status is `200`, so it cannot pass by receiving a `404`.

## How this design was arrived at

Recorded because the reasoning is worth more than the conclusions, and three of the conclusions reversed.

**A cross-model adversarial pass ran three times.** Round one overturned three of four initial leanings: it moved the AC2 proof from `part_merge` to a role-mutation endpoint, replaced a general `audit_log` whose shape could not record a role change, and reframed AC1 as being about admin requests. Round two found the trigger-versus-`ON DELETE SET NULL` contradiction, the check-then-act in the bootstrap, the too-narrow lock, and that a CLI cannot be a fourth arm on `runtime::Role`. Round three attacked the three decisions that came out of the human grilling session and could not break any of them, but found the in-flight demotion, the missing 500 branch, the unindexed foreign keys, and that the AC4 test would have passed via a 404.

**A grilling session with the owner produced the three biggest changes**, and each came from noticing that an earlier decision had dissolved the problem a later one was solving. The last-admin guard and the bootstrap rule cancelled each other out. Account retention made the denormalised email unnecessary. And retention also revived the foreign keys that the trigger contradiction had killed.

**Two checklists ran against the design rather than against a diff**, which is the point of running them at design time: three of their answers became requirements above — the log line on a rejected admin request, the zero-admin check moving inside the lock, and the prohibition on logging an email or a reason.

## What is still open

- **Retention has no bound.** ADR-0001 does not say how long a dead account's data is kept, or whether a genuine erasure route exists. AM-296.
- **`/admin/*` has no rate limit.** AM-356.
- **AM-366 and AM-89 must be told the admin vehicle read landed here**, before either is planned.
