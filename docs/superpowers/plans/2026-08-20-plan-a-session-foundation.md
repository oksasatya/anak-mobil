# Plan A — Session foundation

> **For agentic workers:** REQUIRED SUB-SKILL: use `executing-plans-hybrid` to run this
> plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the session contract whole on the server — a session id the logout
handler can revoke directly, a username namespace, a bootstrap identity endpoint, a
countdown on the login 429 — and give the phone the client that speaks it: token
storage, single-flight refresh, an epoch-guarded sign-out, an error taxonomy, a
per-account query cache, and the route groups that gate on all of it.

**Architecture:** Backend first, mobile second, because the mobile half is typed
against contracts the backend half establishes. On the server the change is
deliberately small: `SessionStore::authenticate` already resolves
`at:{digest} -> session_id -> user_id` and throws the session id away, so carrying it
out costs no round trip; three new endpoints and one migration carry the rest. On the
phone, `src/shared/` is a dependency-ordered set of modules with no cycles — state
holds no I/O, the API client imports the refresh promise but never the reverse, and the
sign-out transaction imports neither.

**Tech Stack:** Rust · axum · sqlx (compile-time-checked, committed `.sqlx` cache) ·
Redis (opaque sessions, Lua-atomic rotation) · Postgres 17 + citext · React Native
0.86 · Expo SDK 57 · expo-router (vendored navigation) · TanStack Query · zustand ·
MMKV · expo-secure-store.

**Spec:** [`docs/superpowers/specs/2026-08-20-am-17-auth-session-onboarding-design.md`](../specs/2026-08-20-am-17-auth-session-onboarding-design.md)

**Closes:** [AM-17](https://oksasatyaa.atlassian.net/browse/AM-17) ·
[AM-18](https://oksasatyaa.atlassian.net/browse/AM-18)

**Branch:** `feat/AM-17-auth-session-onboarding`

---

## Global Constraints

Every task's requirements implicitly include this section.

- **Plans B, C, and D are being written in parallel against the FROZEN CONTRACT
  below.** Producing it exactly is this plan's primary obligation. If implementation
  forces a change to any signature in it, that is a **structural finding** — raise it
  immediately in `## Review findings ledger` and stop, rather than editing silently.
- **Product-facing text is Bahasa Indonesia** — every field message, every screen
  string. Code, comments, doc comments, commit messages, and this plan are English.
- **The domain crate imports no framework.** `make be-boundary` proves it. No `axum`,
  `sqlx`, `redis`, `serde`, or `tracing` in `crates/domain`.
- **No JWT.** Session tokens stay opaque so logout revokes rather than waits.
- **No token in MMKV, in the query cache, in a log, or in a URL.** Tokens live in
  `expo-secure-store` and nowhere else.
- **Never log a token, a digest, or an email on the auth path.** A user id is not a
  credential and is enough to investigate.
- **Every credential failure on `/auth/login` returns the same status, code, and
  message.** Unknown email, wrong password, unparseable hash.
- **One migration per story, `-r` always**, and a migration that has reached `dev` is
  never edited.
- **The AM-15 design system is used, not bypassed**: `useTheme()`, `AmButton`,
  `AmTextField`, `AmEmptyState`, `AmErrorState`, `AmSkeleton`, `AmMaterial`,
  `AmGround`, `useToast`. No raw hex, font-size, or spacing literal in a component.
- **Bun, never npm.** `bun add --filter` does not exist.

### FROZEN CONTRACT — HTTP

All responses use the existing `meta` / `data` / `error` envelope. The shapes below are
the `data` payload.

```
POST /auth/register   {email, username, password}    -> 201 {access_token, refresh_token, token_type, expires_in}
POST /auth/login      {email, password}              -> 200 (same shape)   [UNCHANGED contract]
POST /auth/refresh    {refresh_token}                -> 200 (same shape)   [UNCHANGED contract]
POST /auth/logout     (Authorization only, no body)  -> 200 {signed_out: true}
GET  /me              (Authorization)                -> 200 {id, email, username, display_name, has_vehicles}
                                                            username/display_name are nullable
PATCH /me             {display_name?}                -> 200 (same shape as GET /me)
GET  /usernames/{username}/availability              -> 200 {available: bool}
429 on login: error.details carries {retry_after_seconds: <number>}

409 on register, for BOTH a taken email and a taken username:
  error.code = "conflict", error.details = {"email": "<pesan>"} or {"username": "<pesan>"}
```

**The 409 for a taken username is pinned here (CG-2).** The spec says register must
"report the field that actually collided" and names no status code. 409 is the answer
because `/auth/register` already returns 409 for a taken email — `auth_flow.rs`'s
`a_taken_email_is_refused` asserts `StatusCode::CONFLICT` — and giving the two
collisions different statuses would make the client branch on which field failed before
it can read which field failed. Both are 409 with `error.details` naming exactly one
field. Registration inevitably reveals that an address is taken; that is a pre-existing
and deliberate trade on this endpoint and naming the field does not widen it. It has no
bearing on `/auth/login`, where the anti-enumeration rule is absolute.

### FROZEN CONTRACT — TypeScript (`apps/mobile/src/shared/`)

**Every symbol below is imported from `@/shared` and from nowhere else.** Not
`@/shared/session/store`, not `@/shared/api/client`. One barrel,
`apps/mobile/src/shared/index.ts`, re-exports the whole contract; the internal file
layout is this plan's business and may move without touching B, C, or D. A deep import
in any plan is a finding.

```ts
export interface Me { id: string; email: string; username: string | null; displayName: string | null; hasVehicles: boolean }
export type ApiErrorKind = "offline" | "validation" | "rateLimited" | "unauthorized" | "server";
export interface ApiError { kind: ApiErrorKind; message: string; fields?: Record<string, string>; retryAfterSeconds?: number; requestId?: string }
export function apiRequest<T>(path: string, init?: { method?: string; body?: unknown; signal?: AbortSignal }): Promise<T>;
export type SessionStatus = "loading" | "signedOut" | "signedIn";
export function useSession(): { status: SessionStatus; user: Me | null };
export function signIn(tokens: { access_token: string; refresh_token: string; expires_in: number }): Promise<void>;
export function refreshMe(): Promise<void>;
export function signOut(): Promise<void>;
export function useActiveVehicleId(): string | null;
export function setActiveVehicleId(id: string | null): void;
export function AuthGate(props: { readonly children: ReactNode }): ReactNode;
export function OnboardingGate(props: { readonly children: ReactNode }): ReactNode;
export function AppGate(props: { readonly children: ReactNode }): ReactNode;
```

**`apiRequest<T>` resolves the envelope's `data`, unwrapped.** `apiRequest<Me>("/me")`
resolves a `Me` — never a `{meta, data, error}`. The envelope is transport: a caller
wants the thing. A non-2xx, or a body carrying `error`, throws an `ApiError` instead;
nothing hands an envelope back. `meta.request_id` is attached to the thrown error as
`requestId` for logs and is **never** rendered to a person. `requestId` is the one
addition this plan makes to the contract text it was given — optional, so nothing typed
against the earlier version breaks.

**`refreshMe()` re-fetches `GET /me` and updates the session store**, leaving `status`
alone. Two callers need it and neither is obvious, so the ordering is documented beside
the function itself:

- **Plan C**, when the app shell loads an empty vehicle list. `(app)` is only reachable
  with `hasVehicles === true`, so an empty list means the last car went away elsewhere
  and the cached `me` is stale. `refreshMe()` is the precise recovery; invalidating
  every query and bouncing through `/` is not.
- **Plan D**, immediately after `POST /vehicles`. The cached `me.hasVehicles` is still
  `false` at that moment, so navigating before the refresh resolves sends somebody
  **back into the wizard they just finished**. And the *previous* `hasVehicles` has to be
  read **before** calling `refreshMe()`, because it is what decides aha-screen versus
  garage. Read first, then refresh, then navigate.

**The three gates are components, not layout bodies, and that is structural.** Plan C
replaces `(app)/_layout.tsx`'s body with a Tabs navigator and Plan D replaces
`(onboarding)`'s with the wizard stack; a rewrite that silently drops an inlined gate is
the one security-shaped defect those plans could cause. So Plan A ships the gate as a
component that wraps whatever the layout renders:

```tsx
// (app)/_layout.tsx, as Plan A ships it
export default function AppLayout() {
  return (
    <AppGate>
      <Stack screenOptions={{ headerShown: false, contentStyle: { backgroundColor: "transparent" } }} />
    </AppGate>
  );
}
```

C replaces only `<Stack …/>`, inside a gate it never touches. The redirect logic has one
home per subtree and no screen can acquire a second one.

**`signIn` is the only way a session starts (CG-1).** Register and login both receive a
token pair and hand it here; no screen writes to secure storage itself, because
centralising that write is most of what this plan is for. Its order is fixed and is an
acceptance criterion: write the pair → clear `refresh_pending` → `GET /me` →
populate the store → flip `status` to `signedIn`. Flipping status before the user is
loaded would let a group layout render against `user === null`.

**`ApiErrorKind` deliberately has no `conflict` member (CG-2).** A register collision
arrives as a 409 whose `error.details` names one field, and the mapper turns it into
`kind: "validation"` with `fields.email` or `fields.username` set — because from the
person's side "email ini sudah terdaftar" *is* a message belonging under the email
field. The taxonomy describes what somebody sees, not what the status line said. This
is the one place where the kind is not a direct function of the status code, and it is
covered by an acceptance criterion in Task 11 so it cannot regress into `"server"`.

### ENVIRONMENT — paste verbatim into every task brief

```
1. Every make target runs from the REPOSITORY ROOT.
2. Gates: backend `make be-check` (= be-fmt be-lint be-test be-boundary) ·
   `make be-sqlx-check` ** CI RUNS THIS ** · tokens `make ds-check` ·
   landing `make fe-check` · mobile `make mb-check` · everything `make check`.
3. ** THE .sqlx TRAP ** apps/api uses sqlx compile-time-checked queries with a
   COMMITTED .sqlx cache. Any new or changed SQL requires `make be-prepare`
   (needs a running DB) and the regenerated cache COMMITTED, or
   `make be-sqlx-check` fails in CI even though it compiled locally.
4. `make be-boundary` proves the domain crate imports nothing from a framework.
   Keep domain types free of axum/sqlx/redis.
5. `make db-up` starts Postgres (Redis assumed local); `make db-up-all` both;
   `make db-drop` is the fast reset. Integration tests report PASSING when
   DATABASE_URL/REDIS_URL are absent — the Makefile loads the root .env.
6. Bun, never npm. `bun add --filter` DOES NOT EXIST — use
   `bun add --cwd apps/mobile <pkg>`, or `bun x expo install <pkg>` from inside
   apps/mobile for anything with native code. `bun install --frozen-lockfile`
   must stay EXIT=0 with bun.lock unchanged. Never a nested lockfile.
7. Prettier runs from the ROOT only (`bun run format`). Markdown is excluded.
8. ** expo-router SDK 56+ VENDORS ITS OWN NAVIGATION. ** Do NOT install
   @react-navigation/native — the router throws "no longer compatible with
   react-navigation". ThemeProvider/DarkTheme/DefaultTheme are re-exported
   FROM "expo-router".
9. _layout.tsx already overrides the navigation container background to
   transparent so AmGround shows through. Do not undo it.
10. NEVER put a changing `key` on a View wrapping {children} at the app root —
    it unmounts the whole subtree and discards every screen's state.
11. apps/mobile has NO test runner and this work does not add one. Its check is
    tsc + eslint. `@typescript-eslint/no-explicit-any: error`, tsconfig strict.
12. The AM-15 design system is ready and MUST be used: useTheme(), AmButton,
    AmTextField, AmSelect, AmCard, AmChip, AmBadge, AmAvatar, AmBottomSheet,
    AmEmptyState, AmErrorState, AmSkeleton, useToast, AmMaterial, AmGround.
    NO raw hex/font-size/spacing literal in a component. Product strings are
    Bahasa Indonesia; code, comments, commit messages are English.
13. Root .env belongs to the BACKEND. apps/mobile reads only EXPO_PUBLIC_* via
    app.config.ts. Never put a secret in a mobile env.
14. CI workflows are path-filtered per app — read .github/workflows/*.yml
    rather than assuming which job a change reddens.
```

### QUALITY GATE — paste verbatim into every task brief (this repo runs NO Sonar)

```
Rust (apps/api):
- clippy::too_many_arguments <=7 (aim <=5); cognitive_complexity -> extract helpers.
- NO .unwrap()/.expect()/panic!/todo!() on production paths; tests may.
- Duplicated string literal 3+ times -> a const.
- thiserror in domain/usecase, anyhow only at the boundary; never `let _ = fallible();`.
- Map domain -> HTTP at the ONE IntoResponse choke point in shared/errors.rs.
- sqlx: parameterized or the query! macros, never format! into SQL.
- Verify: make be-fmt -> be-lint -> be-test -> be-boundary -> be-sqlx-check.

TypeScript (apps/mobile):
- strict on; NO explicit any (eslint error); no @ts-ignore without a one-line reason.
- React props readonly, exported as `<Component>Props`.
- No raw design values — everything via useTheme().
- Never set allowFontScaling={false}.
- Prefer ?? and ?., arr.at(-1), real elements over ARIA roles.
- Verify: bun run format -> make mb-check (exit codes, not piped output).
```

---

## File structure

### Backend — `apps/api/crates/`

| File | Responsibility | Task |
|---|---|---|
| `runtime/src/adapter/redis/session.rs` | `Resolved { user_id, session_id }`; `authenticate` stops discarding the session id | 1 |
| `runtime/src/adapter/http/auth.rs` | `Authenticated.session_id`; logout revokes directly; register DTO split; login 429 detail | 1, 4, 5 |
| `runtime/migrations/20260820*_username_and_display_name.{up,down}.sql` | `username CITEXT` + partial unique index, `display_name TEXT` | 2 |
| `domain/src/identity/username.rs` | the one canonicaliser and the reserved list — pure, no I/O | 3 |
| `runtime/src/adapter/redis/rate_limit.rs` | `Attempt`/`LoginAttempt`; the window's remaining TTL | 4 |
| `runtime/src/usecase/auth.rs` | `Registration` input struct; the constraint-name `23505` mapping | 5 |
| `runtime/src/adapter/postgres/user_repo.rs` | `NewUser`, `profile_of`, `set_display_name`, `username_exists` | 5, 6, 7 |
| `runtime/src/usecase/profile.rs` | read and update the caller's own profile | 6 |
| `runtime/src/adapter/http/profile.rs` | `GET /me`, `PATCH /me`, `GET /usernames/{username}/availability` | 6, 7 |
| `runtime/src/adapter/http/mod.rs` | the three new routes | 6, 7 |
| `runtime/tests/profile_flow.rs` | the new endpoints end to end | 6, 7 |

`profile.rs` rather than `identity.rs` for the new adapter and use case: "identity" is
already what `auth.rs` is about, and these three endpoints are specifically about the
profile a person shows the world.

### Mobile — `apps/mobile/src/`

```
shared/
  index.ts                  the public surface B/C/D import — exactly the FROZEN CONTRACT
  api/
    errors.ts               ApiError, ApiErrorKind, envelope narrowing        (imports nothing)
    refresh.ts              the single-flight promise + the pending marker    (bare fetch, never client.ts)
    client.ts               apiRequest: base URL, auth header, 401 -> refresh -> ONE retry
    me.ts                   fetchMe(), refreshMe(), the snake_case -> camelCase mapping
    queryClient.ts          QueryClient + per-account MMKV persistence
  session/
    secure.ts               the one expo-secure-store record {access, refresh, refreshPending}
    store.ts                zustand: status, user, epoch                      (imports nothing but zustand)
    signIn.ts               the only way a session starts                     (never imports client.ts directly)
    signOut.ts              the epoch transaction                             (never imports client.ts)
  vehicle/
    activeVehicle.ts        AM-18's active vehicle, persisted
  gates.tsx                 AuthGate, OnboardingGate, AppGate — the only redirects
  bootstrap.ts              useBootstrap(): restore, /me, warm the cache, then flip status
app/
  _layout.tsx               providers + the bootstrap gate (modified)
  index.tsx                 the router: reads the session, redirects (replaces the healthcheck)
  (auth)/_layout.tsx        + index.tsx   — redirects OUT on signedIn (CG-3)
  (onboarding)/_layout.tsx  + index.tsx   — redirects OUT when onboarding is complete
  (app)/_layout.tsx         + index.tsx   (the healthcheck moves here)
  catalog.tsx               unchanged, ungrouped, deliberately unguarded
```

**The import graph is acyclic and that is load-bearing.** `store.ts` imports nothing
but `zustand`, so `client.ts` may read the epoch from it. `refresh.ts` uses a bare
`fetch` rather than `apiRequest`, or a 401 on `/auth/refresh` would recurse.
`signOut.ts` imports `store`, `secure`, and `queryClient` but never `client`, which is
what lets `client.ts` call `signOut()`. `signIn.ts` imports `me.ts` (and so `client.ts`
transitively) — safe, because nothing under `api/` imports `signIn.ts`.

**Every redirect lives in a group layout, never in a screen (CG-3).** `(auth)`
redirects out the moment `status` flips to `signedIn`; `(onboarding)` redirects out the
moment the account has a display name and a vehicle; `(app)` redirects back when either
is missing. A `router.replace()` in a screen's `onSuccess` is precisely the second
redirect the spec bans, and with both a login screen and a register screen able to
start a session it would be two of them. The route groups exist so the decision has one
home per subtree.

---

## Tidak boleh ada

Copied verbatim from the spec. Anything on this list appearing in a diff is a finding,
not a judgement call.

- No JWT. The session tokens are opaque on purpose so that logout revokes rather than
  waits.
- No token in MMKV, in the query cache, in a log, or in a URL.
- No second copy of the username rules on the client. The client may mirror the regex
  for instant feedback, but the server's canonicaliser is the authority and the client
  never decides a name is acceptable on its own.
- No invented counts on the aha screen, no seeded vehicles, no placeholder community
  numbers.
- No skip button in the first-car wizard.
- No distinguishing an unknown email from a wrong password, anywhere, ever — including
  in analytics, logs, or a "did you mean to register?" hint on the login screen.
- No second redirect on session expiry.
- No social login, no email verification, no password reset, no biometrics, no
  two-factor. Those are AM-52, AM-53, AM-54, AM-77 and stay theirs.
- No offline writes. AM-18 explicitly scopes to reading from cache.
- No tab content for Explore and Community beyond an honest empty state.

---

## Task 1: `Authenticated` carries `session_id`, and logout stops rotating

Closes the defect in spec §5. The store already walks `at:{digest} -> session_id ->
user_id` and discards the session id at `session.rs:243`; this stops discarding it, and
logout then revokes that session directly instead of rotating the refresh token to
discover which one to end.

**Files:**
- Modify: `apps/api/crates/runtime/src/adapter/redis/session.rs:227-244` (`authenticate`)
- Modify: `apps/api/crates/runtime/src/adapter/http/auth.rs:22-48` (`Authenticated`), `:80` (`Admin`'s destructure), `:233-264` (`logout`)
- Test: `apps/api/crates/runtime/tests/session_store.rs` (five assertions change shape)
- Test: `apps/api/crates/runtime/tests/auth_flow.rs` (one new test, two updated)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `anakmobil_runtime::adapter::redis::session::Resolved { pub user_id: Uuid, pub session_id: Uuid }`
  - `SessionStore::authenticate(&self, access: &str) -> Result<Option<Resolved>, SessionError>`
  - `adapter::http::auth::Authenticated { pub user_id: Uuid, pub session_id: Uuid }`
  - `logout(State<AppState>, Authenticated) -> Result<ApiResponse<serde_json::Value>, ApiError>` — no body extractor

**TDD: yes** — the defect has a stateable failing test: a logout whose refresh rotation
would report a replay must still revoke the session.

**Facts you will not discover from the plan text:**
- Thirty-odd handlers take `caller: Authenticated` and read `caller.user_id` by field
  access; adding a field leaves every one of them compiling. There is **exactly one**
  destructuring pattern, at `auth.rs:80` inside `Admin::from_request_parts`, and it is
  the only thing that breaks. Verified by grep across `runtime/src`.
- `tests/session_store.rs` asserts `authenticate(...) == Some(user)` in five places
  (lines 38-44, 50-56, 70-76, 95-98, 176-178, 194-202). Each becomes a comparison
  against the new struct, or reads `.user_id`.
- `auth_flow.rs` posts a JSON body to `/auth/logout`. Dropping the `Json` extractor
  makes axum ignore the body rather than reject it, so those tests keep passing — but
  update them anyway so the file stops documenting a contract that no longer exists.
- A handler whose arguments are all `FromRequestParts` compiles fine in axum via the
  blanket impl. `catalog::brands` (`State` + `_caller: Authenticated`, no body) is the
  proof in this codebase.

- [ ] **Step 1: Write the failing test**

Add to `apps/api/crates/runtime/tests/auth_flow.rs`:

```rust
#[tokio::test]
async fn logout_revokes_even_when_the_refresh_token_is_already_spent() {
    // The defect in spec §5. Logout used to rotate the refresh token to find
    // the session; a rotation that reports Reused or Invalid returned success
    // and revoked nothing, so somebody could press sign-out, be told it worked,
    // and still be authenticated a moment later.
    let app = app!();
    let (email, peer) = (an_email(), a_peer());

    let body = json(
        post(
            &app,
            "/auth/register",
            json!({"email": email, "password": "kata sandi panjang"}),
            peer,
        )
        .await,
    )
    .await;
    let access = body["data"]["access_token"]
        .as_str()
        .expect("access token")
        .to_owned();
    let refresh = body["data"]["refresh_token"]
        .as_str()
        .expect("refresh token")
        .to_owned();

    // Spend the refresh token elsewhere, exactly as an in-flight refresh does.
    // The access token from the ORIGINAL pair is still live: rotation slides
    // the session rather than ending it.
    assert_eq!(
        post(&app, "/auth/refresh", json!({"refresh_token": refresh}), peer)
            .await
            .status(),
        StatusCode::OK
    );

    // No body at all — the Authorization header is the whole request.
    let out = post_with_auth(&app, "/auth/logout", json!({}), peer, Some(&access)).await;
    assert_eq!(out.status(), StatusCode::OK);
    assert_eq!(json(out).await["data"]["signed_out"], true);

    // The session must actually be gone.
    let after = post_with_auth(&app, "/auth/logout", json!({}), peer, Some(&access)).await;
    assert_eq!(
        after.status(),
        StatusCode::UNAUTHORIZED,
        "logout reported success without revoking the session"
    );
}

#[tokio::test]
async fn a_logout_against_a_dead_session_answers_like_a_live_one() {
    // Distinguishing them would tell a caller which it was.
    let app = app!();
    let (email, peer) = (an_email(), a_peer());

    let first = json(
        post(
            &app,
            "/auth/register",
            json!({"email": email, "password": "kata sandi panjang"}),
            peer,
        )
        .await,
    )
    .await;
    let live = first["data"]["access_token"]
        .as_str()
        .expect("access token")
        .to_owned();

    let alive = post_with_auth(&app, "/auth/logout", json!({}), peer, Some(&live)).await;
    let alive_status = alive.status();
    let alive_body = json(alive).await;

    // A second session, revoked out from under the token before logout runs.
    let second = json(
        post(
            &app,
            "/auth/login",
            json!({"email": email, "password": "kata sandi panjang"}),
            a_peer(),
        )
        .await,
    )
    .await;
    let dead = second["data"]["access_token"]
        .as_str()
        .expect("access token")
        .to_owned();
    post_with_auth(&app, "/auth/logout", json!({}), peer, Some(&dead)).await;

    // Logging out twice with the same token: the second one meets a dead
    // session. It must be refused as unauthenticated — never a 200 that
    // pretends, and never a distinct code that says "already signed out".
    let again = post_with_auth(&app, "/auth/logout", json!({}), peer, Some(&dead)).await;
    assert_eq!(again.status(), StatusCode::UNAUTHORIZED);

    assert_eq!(alive_status, StatusCode::OK);
    assert_eq!(alive_body["data"]["signed_out"], true);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
make db-up-all
make be-test
```

Expected: `logout_revokes_even_when_the_refresh_token_is_already_spent` FAILS on the
final assertion with `assertion `left == right` failed: logout reported success without
revoking the session`, `left: 200`. If it fails anywhere earlier, the test is wrong —
fix the test before touching the implementation.

- [ ] **Step 3: Carry the session id out of the store**

In `apps/api/crates/runtime/src/adapter/redis/session.rs`, add above `SessionStore`:

```rust
/// Who a live access token belongs to, and which session it came from.
///
/// The session id costs nothing to return: [`SessionStore::authenticate`] has
/// always had to read `at:{digest} -> session_id` before it could read
/// `sess:{session_id} -> user_id`, and used to throw the first hop away. Logout
/// then had to rediscover the session by rotating a refresh token, which is how
/// it came to report success while revoking nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolved {
    pub user_id: Uuid,
    pub session_id: Uuid,
}
```

Replace the body of `authenticate` (lines 235-244):

```rust
    pub async fn authenticate(&self, access: &str) -> Result<Option<Resolved>, SessionError> {
        let mut conn = self.redis.clone();

        let Some(raw_session): Option<String> = conn.get(access_key(access)).await? else {
            return Ok(None);
        };
        let Ok(session_id) = Uuid::parse_str(&raw_session) else {
            return Ok(None);
        };
        let Some(raw_user): Option<String> = conn.get(session_key(session_id)).await? else {
            return Ok(None);
        };
        let Ok(user_id) = Uuid::parse_str(&raw_user) else {
            return Ok(None);
        };

        Ok(Some(Resolved {
            user_id,
            session_id,
        }))
    }
```

Both hops must still be required — that second lookup is what makes logout instant.
Note this now goes through `session_key()` instead of an inline `format!`, which is the
same key and one fewer place to get the prefix wrong.

- [ ] **Step 4: Carry it into the extractor**

In `apps/api/crates/runtime/src/adapter/http/auth.rs`:

```rust
#[derive(Debug, Clone, Copy)]
pub struct Authenticated {
    pub user_id: Uuid,
    /// The session this token belongs to, so a handler that must end it does
    /// not have to rediscover it. Every handler that only wants `user_id`
    /// ignores this field and is unaffected.
    pub session_id: Uuid,
}
```

and in `from_request_parts`, replace the `.map(...)` tail:

```rust
            .map(|resolved| Self {
                user_id: resolved.user_id,
                session_id: resolved.session_id,
            })
            .ok_or_else(ApiError::unauthorized)
```

Fix the one destructure at line 80, inside `Admin::from_request_parts`:

```rust
        let Authenticated { user_id, .. } = Authenticated::from_request_parts(parts, state).await?;
```

- [ ] **Step 5: Logout revokes directly**

Replace `logout` entirely:

```rust
/// `POST /auth/logout`
///
/// Ends the session the access token belongs to, and no other. Signing out on a
/// phone should not sign out the tablet.
///
/// The session id comes from the extractor, which resolved it from the token
/// the caller actually presented — so a client still cannot end a session it
/// cannot prove it holds, and no refresh token is spent to find out which one
/// it is. Rotating to discover the session was the old approach, and a rotation
/// that came back `Reused` or `Invalid` left this handler answering success
/// having revoked nothing.
///
/// A logout against an already-dead session never reaches here: the extractor
/// refuses it as unauthenticated, exactly as it refuses any other request
/// carrying a revoked token. Nothing distinguishes the two cases for a caller.
///
/// # Errors
///
/// A storage failure.
pub async fn logout(
    State(state): State<AppState>,
    caller: Authenticated,
) -> Result<ApiResponse<serde_json::Value>, ApiError> {
    auth::logout(&state.sessions, caller.user_id, caller.session_id)
        .await
        .map_err(to_api_error)?;

    Ok(ApiResponse::ok(serde_json::json!({ "signed_out": true })))
}
```

`RefreshRequest` stays — `refresh` still uses it. `Rotation` is no longer imported by
this handler; remove the now-unused path if clippy flags it.

- [ ] **Step 6: Update the assertions the new type breaks**

In `apps/api/crates/runtime/tests/session_store.rs`, every
`assert_eq!(store.authenticate(&pair.access).await.expect("…"), Some(user))` becomes:

```rust
    assert_eq!(
        store
            .authenticate(&pair.access)
            .await
            .expect("authenticating")
            .map(|resolved| resolved.user_id),
        Some(user)
    );
```

and in `a_new_session_authenticates` add the assertion the change exists for:

```rust
    let resolved = store
        .authenticate(&pair.access)
        .await
        .expect("authenticating")
        .expect("a live session");
    assert_eq!(resolved.user_id, user);
    assert_eq!(
        resolved.session_id, pair.session_id,
        "the session id must survive the walk, or logout has to rediscover it"
    );
```

`assert_eq!(…, None)` cases need no change — `Option<Resolved>` compares to `None`
fine.

In `tests/auth_flow.rs`, change the two existing logout calls
(`logout_stops_the_next_request:322-340`, `refreshing_rotates_and_a_replay_is_refused:397-404`)
and `a_request_without_a_token_is_refused:415-421` to post `json!({})` rather than a
refresh token, so the file stops documenting a body that is no longer read.

- [ ] **Step 7: Run the gates yourself**

```bash
make be-fmt && make be-lint && make be-test && make be-boundary
```

Expected: EXIT=0 on each, and the two new tests pass. No SQL changed, so
`make be-prepare` is not needed here.

**Acceptance criteria**
- `Authenticated` exposes `session_id`; every existing handler still compiles untouched.
- `POST /auth/logout` takes no request body and revokes the caller's own session.
- A logout after the refresh token has been spent still revokes — the new test proves it.
- A logout against a revoked token is a 401 from the extractor, indistinguishable from
  any other request with a dead token.
- `make be-check` EXIT=0.

---

## Task 2: the `username` and `display_name` migration

**Files:**
- Create: `apps/api/crates/runtime/migrations/<timestamp>_username_and_display_name.up.sql`
- Create: `apps/api/crates/runtime/migrations/<timestamp>_username_and_display_name.down.sql`

**Interfaces:**
- Consumes: nothing.
- Produces: `users.username CITEXT NULL`, `users.display_name TEXT NULL`, and the
  unique index **named exactly `users_username_key`** — Task 5 matches on that string.

**TDD: no** — a migration has no unit under test. Verified by applying it and reading
`\d users`; the constraint gets its assertion in Task 5's register test and Task 7's
availability test.

**Facts you will not discover from the plan text:**
- Generate the file with `cd apps/api/crates/runtime && sqlx migrate add -r username_and_display_name`.
  Always `-r`. Migrations resolve relative to `crates/runtime`, not the workspace root.
- `citext` overloads the regex operators to be case-insensitive. A `CHECK (username ~
  '^[a-z0-9._]…')` on a `CITEXT` column therefore **accepts `BUDI`**. The cast to
  `::text` is what forces the case-sensitive operator, and it is not optional.
- A plain `UNIQUE` on a nullable column already permits many NULLs in PostgreSQL; the
  partial index is chosen for the smaller index and the explicit intent, and because
  naming it ourselves is what makes Task 5's constraint match a fixed string rather
  than a guess at what PostgreSQL would have called it.
- `users` already has a `set_updated_at` trigger. Nothing to add.
- This migration is unmerged and unpushed while you work on it, so it may be amended in
  place followed by `make db-drop` — but only while nothing else is running against
  that database. With concurrent agents, amending and resetting your copy leaves every
  other process holding the old checksum and the whole suite fails silently.

- [ ] **Step 1: Generate the migration pair**

```bash
cd apps/api/crates/runtime && sqlx migrate add -r username_and_display_name
```

- [ ] **Step 2: Write the up migration**

```sql
-- The public half of an account: the name people address, and the name they
-- show. Both nullable, and both for the same reason — `NOT NULL UNIQUE` cannot
-- be added to a table that already has rows without a backfill nobody has
-- designed. There are no production rows today; the migration is honest anyway.

-- CITEXT for consistency with `email`, so `Budi` and `budi` cannot both be
-- claimed. CITEXT is NOT the validator: case-insensitive uniqueness is a
-- different thing from a character rule, and the rule lives in exactly one
-- place — `anakmobil_domain::identity::username::canonicalise`.
ALTER TABLE users ADD COLUMN username CITEXT;

-- Not unique, not an identifier, and deliberately plain TEXT. Two people may
-- both be "Budi"; what distinguishes them is the username. Collected during
-- onboarding, which by definition happens after the row exists — so an account
-- that has not finished onboarding has NULL here, and that is one of the two
-- facts `GET /me` reports.
ALTER TABLE users ADD COLUMN display_name TEXT;

-- Cheap sanity, exactly like `users_email_shape` — a floor, not the validator.
-- It knows nothing about consecutive dots or edge punctuation; the canonicaliser
-- does, and duplicating those rules here would create a second copy free to
-- drift from the first.
--
-- The `::text` cast is load-bearing. citext overloads `~` to be
-- case-insensitive, so `username ~ '^[a-z0-9._]{3,30}$'` would happily accept
-- 'BUDI' — precisely the value this constraint exists to keep out of the table.
ALTER TABLE users
    ADD CONSTRAINT users_username_shape
    CHECK (username IS NULL OR username::text ~ '^[a-z0-9._]{3,30}$');

-- Named explicitly rather than left to PostgreSQL, because the register handler
-- matches on this string to report which field collided. A unique violation
-- reports the INDEX name, so the name is part of the contract.
CREATE UNIQUE INDEX users_username_key ON users (username) WHERE username IS NOT NULL;

COMMENT ON COLUMN users.username IS
    'The public namespace and profile address (/@username). Canonicalised server-side before it ever reaches this column; CITEXT provides case-insensitive uniqueness, not validation.';
COMMENT ON COLUMN users.display_name IS
    'What a person calls themselves. Not unique, not an identifier, NULL until onboarding collects it.';
```

- [ ] **Step 3: Write the down migration**

```sql
DROP INDEX IF EXISTS users_username_key;
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_username_shape;
ALTER TABLE users DROP COLUMN IF EXISTS display_name;
ALTER TABLE users DROP COLUMN IF EXISTS username;
```

- [ ] **Step 4: Apply it and read the schema back**

```bash
make db-up
make be-migrate
make db-psql
```

At the psql prompt:

```sql
\d users
```

Expected, and each of these is checked rather than assumed:
- `username | citext |` (nullable) and `display_name | text |` (nullable) are listed.
- `"users_username_key" UNIQUE, btree (username) WHERE username IS NOT NULL` appears
  under Indexes.
- The email unique index is listed, and **you write down its exact name** —
  `users_email_key` is expected, and Task 5 depends on that string being right.

Then prove the cast does what it claims:

```sql
INSERT INTO users (id, email, password_hash, username)
VALUES (gen_random_uuid(), 'shape-check@example.com', 'x', 'BUDI');
-- Expected: ERROR: new row for relation "users" violates check constraint "users_username_shape"

INSERT INTO users (id, email, password_hash, username)
VALUES (gen_random_uuid(), 'shape-check@example.com', 'x', 'budi.dua');
-- Expected: INSERT 0 1

DELETE FROM users WHERE email = 'shape-check@example.com';
```

If the first insert **succeeds**, the `::text` cast is missing or ineffective — stop and
fix it, because the column is then accepting values the canonicaliser would refuse.

- [ ] **Step 5: Regenerate the sqlx cache and run the gates**

No query changed yet, but the schema did, and the cache records column types.

```bash
make be-prepare
make be-check
make be-sqlx-check
```

Expected: EXIT=0 on each. Commit whatever `.sqlx/` files change.

**Acceptance criteria**
- Both columns exist, both nullable, both commented.
- `users_username_key` exists under exactly that name.
- `'BUDI'` is rejected by `users_username_shape`; `'budi.dua'` is accepted.
- The down migration reverses all four objects.
- `make be-sqlx-check` EXIT=0 with the regenerated cache committed.

---

## Task 3: the one username canonicaliser, in the domain crate

**Files:**
- Create: `apps/api/crates/domain/src/identity/username.rs`
- Modify: `apps/api/crates/domain/src/identity/mod.rs` (add `pub mod username;`)

**Interfaces:**
- Consumes: nothing.
- Produces, from `anakmobil_domain::identity::username`:
  - `pub const MIN_LEN: usize = 3;` · `pub const MAX_LEN: usize = 30;`
  - `pub enum UsernameError { TooShort, TooLong, BadCharacter, EdgePunctuation, ConsecutiveDots }`
  - `pub fn canonicalise(raw: &str) -> Result<String, UsernameError>`
  - `pub fn is_reserved(canonical: &str) -> bool`

**TDD: yes** — pure input to output, every rule has a boundary, and the whole point of
the function is that it is the single authority.

**`is_reserved` is separate from `canonicalise`, and that is a correction worth reading.**
The obvious design makes `Reserved` a sixth `UsernameError` variant. It cannot be one:
the spec requires a reserved name and a taken name to answer **identically**, and a
validation error is a 422 while a taken name is a 409 — so folding reserved into the
error enum builds the exact oracle the guard rails exist to prevent. `canonicalise`
answers "is this a well-formed name", `is_reserved` answers "may this one be claimed",
and both callers ask both questions.

**Facts you will not discover from the plan text:**
- `crates/domain/Cargo.toml` has `thiserror`, `uuid`, `time` and nothing else. No
  `serde`, no `regex`. Write the checks by hand — they are simpler than a regex here
  anyway, and `make be-boundary` fails the build if you reach for a framework.
- `identity/mod.rs` is currently doc comments only, with no `pub mod` lines. Adding one
  is the whole modification.
- Use `to_ascii_lowercase`, not `to_lowercase`. The permitted alphabet is ASCII, and
  Unicode lowercasing can turn one char into several (Turkish `İ`), which would make
  the returned length disagree with the length that was validated.
- `check_password_shape` in `adapter/http/auth.rs:158` is the house style for a shape
  check: count characters rather than bytes, and let the adapter write the Indonesian
  message. Follow it — `UsernameError`'s own `#[error(...)]` strings are English, for
  logs; the client-facing Bahasa Indonesia text is written in Task 5's handler.

- [ ] **Step 1: Write the failing tests**

Create `apps/api/crates/domain/src/identity/username.rs` containing only the test module
for now, so the first run fails on missing items rather than on a wrong answer:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_name_survives_unchanged() {
        assert_eq!(canonicalise("budi").as_deref(), Ok("budi"));
        assert_eq!(canonicalise("budi_santoso").as_deref(), Ok("budi_santoso"));
        assert_eq!(canonicalise("budi.s").as_deref(), Ok("budi.s"));
        assert_eq!(canonicalise("b3ngkel99").as_deref(), Ok("b3ngkel99"));
    }

    #[test]
    fn case_and_surrounding_space_are_normalised_away() {
        // Somebody typing on a phone gets a capital first letter for free, and
        // a paste carries whitespace. Neither is a reason to refuse a name.
        assert_eq!(canonicalise("  Budi  ").as_deref(), Ok("budi"));
        assert_eq!(canonicalise("BUDI").as_deref(), Ok("budi"));
    }

    #[test]
    fn the_length_boundaries_are_exact() {
        assert_eq!(canonicalise("ab"), Err(UsernameError::TooShort));
        assert!(canonicalise("abc").is_ok());
        assert!(canonicalise(&"a".repeat(MAX_LEN)).is_ok());
        assert_eq!(
            canonicalise(&"a".repeat(MAX_LEN + 1)),
            Err(UsernameError::TooLong)
        );
    }

    #[test]
    fn length_is_counted_in_characters_not_bytes() {
        // "aé" is two characters and three bytes. Counting bytes would let a
        // two-character name through the minimum.
        assert_eq!(canonicalise("aé"), Err(UsernameError::TooShort));
    }

    #[test]
    fn only_lowercase_letters_digits_dot_and_underscore_are_permitted() {
        for bad in ["budi-santoso", "budi santoso", "budi@id", "budi!", "büdi", "budi😀x"] {
            assert_eq!(
                canonicalise(bad),
                Err(UsernameError::BadCharacter),
                "{bad} should be refused"
            );
        }
    }

    #[test]
    fn a_name_may_not_begin_or_end_with_punctuation() {
        for bad in [".budi", "budi.", "_budi", "budi_"] {
            assert_eq!(
                canonicalise(bad),
                Err(UsernameError::EdgePunctuation),
                "{bad} should be refused"
            );
        }
    }

    #[test]
    fn dots_may_not_run_together() {
        // "budi..s" and "budi.s" would read as the same person at a glance,
        // which is the whole reason to refuse it.
        assert_eq!(canonicalise("budi..s"), Err(UsernameError::ConsecutiveDots));
        assert_eq!(canonicalise("a...b"), Err(UsernameError::ConsecutiveDots));
        assert!(canonicalise("budi.s.t").is_ok());
    }

    #[test]
    fn underscores_may_run_together() {
        // Deliberate asymmetry: `__` is visually distinct from `_`, and the
        // confusion the dot rule prevents does not arise.
        assert!(canonicalise("budi__s").is_ok());
    }

    #[test]
    fn a_reserved_name_is_well_formed_but_unavailable() {
        // The distinction that keeps the availability endpoint from leaking.
        // A reserved name must pass canonicalisation so that it can be reported
        // as unavailable in exactly the way a taken name is; making it a
        // validation error would answer 422 where a taken name answers 200
        // {available:false}, and the difference is the oracle.
        assert_eq!(canonicalise("Admin").as_deref(), Ok("admin"));
        assert!(is_reserved("admin"));
        assert!(is_reserved("anakmobil"));
        assert!(!is_reserved("budi"));
    }

    #[test]
    fn every_reserved_name_is_itself_a_valid_username() {
        // A reserved entry that could never be typed anyway is dead weight, and
        // one that is not canonical would never match the value being checked.
        for name in RESERVED {
            assert_eq!(
                canonicalise(name).as_deref(),
                Ok(name),
                "{name} is reserved but not canonical"
            );
        }
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

```bash
cargo test -p anakmobil-domain identity::username
```

Expected: FAIL to compile with `cannot find function `canonicalise` in this scope` and
similar. That is the correct first failure.

- [ ] **Step 3: Write the implementation**

Prepend to the same file, above the test module:

```rust
//! The one place a username is decided.
//!
//! Pure: text in, a canonical name or a reason out. No async, no database, no
//! clock — so the rules can be exercised exhaustively at their boundaries
//! without a Postgres anywhere.
//!
//! `CITEXT` in the schema gives case-insensitive **uniqueness**. It is not the
//! validator, and the difference matters: uniqueness cannot tell `budi..s` from
//! `budi.s`, cannot refuse a leading dot, and cannot bound a length. Every path
//! that accepts a username — registration and the availability check — calls
//! this function first, so the column never sees a form this file would refuse.

/// The shortest name worth having. Two characters is a namespace nobody can
/// share fairly.
pub const MIN_LEN: usize = 3;

/// The longest. Thirty fits a profile header without truncation.
pub const MAX_LEN: usize = 30;

/// Names the platform keeps, in canonical form.
///
/// `/@username` is a real route, so a person holding `settings` or `login`
/// would sit on a path the product wants. The list is short on purpose: every
/// entry is a name somebody might otherwise reasonably want, and a long
/// speculative list is a land grab against our own users.
///
/// Never reported as its own outcome — see [`is_reserved`].
pub const RESERVED: [&str; 13] = [
    "about", "admin", "anakmobil", "api", "edit", "help", "login", "me", "new", "profile",
    "register", "settings", "support",
];

/// Why a username was refused.
///
/// Reserved is deliberately absent: an unavailable name is not a malformed one,
/// and answering them differently would tell a caller which is which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UsernameError {
    #[error("username is shorter than the minimum")]
    TooShort,
    #[error("username is longer than the maximum")]
    TooLong,
    #[error("username contains a character that is not permitted")]
    BadCharacter,
    #[error("username begins or ends with a dot or an underscore")]
    EdgePunctuation,
    #[error("username contains consecutive dots")]
    ConsecutiveDots,
}

/// Trim, lowercase, and check the shape. Returns the form that goes in the
/// column and in a URL.
///
/// Complexity: `O(n)` time over the input's characters, `O(n)` space for the
/// lowered copy, with `n` bounded by [`MAX_LEN`] on every accepted value.
///
/// # Errors
///
/// One [`UsernameError`] per call — the first rule broken, checked in the order
/// a person would notice: length, then alphabet, then the two placement rules.
pub fn canonicalise(raw: &str) -> Result<String, UsernameError> {
    let lowered = raw.trim().to_ascii_lowercase();

    // Characters, not bytes. A two-character name made of multi-byte glyphs
    // would otherwise clear a byte-counted minimum.
    let length = lowered.chars().count();
    if length < MIN_LEN {
        return Err(UsernameError::TooShort);
    }
    if length > MAX_LEN {
        return Err(UsernameError::TooLong);
    }

    if !lowered
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_')
    {
        return Err(UsernameError::BadCharacter);
    }

    // Byte indexing is safe from here: every character is ASCII, so a byte is a
    // character.
    let bytes = lowered.as_bytes();
    let edge = |b: u8| b == b'.' || b == b'_';
    if bytes.first().copied().is_some_and(edge) || bytes.last().copied().is_some_and(edge) {
        return Err(UsernameError::EdgePunctuation);
    }

    if bytes.windows(2).any(|pair| pair == b"..") {
        return Err(UsernameError::ConsecutiveDots);
    }

    Ok(lowered)
}

/// Is this canonical name one the platform keeps?
///
/// Separate from [`canonicalise`] so that a reserved name can be reported in
/// exactly the way a taken name is. Thirteen entries scanned linearly: `O(1)`
/// for a fixed list, and a `HashSet` here would cost more to build than the
/// scan costs to run.
#[must_use]
pub fn is_reserved(canonical: &str) -> bool {
    RESERVED.contains(&canonical)
}
```

Add to `apps/api/crates/domain/src/identity/mod.rs`, after the existing doc comments:

```rust
pub mod username;
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test -p anakmobil-domain identity::username
```

Expected: PASS, ten tests.

- [ ] **Step 5: Run the gates yourself**

```bash
make be-fmt && make be-lint && make be-test && make be-boundary
```

Expected: EXIT=0 on each. `be-boundary` is the one that matters here — it proves the
new module pulled in no framework.

**Acceptance criteria**
- `canonicalise` is the only implementation of the rules anywhere in the repository.
- Reserved names canonicalise successfully and are reported only by `is_reserved`.
- Every entry in `RESERVED` is itself canonical — asserted, not assumed.
- The domain crate's dependency list is unchanged; `make be-boundary` EXIT=0.

---

## Task 4: `retry_after_seconds` on the login 429

Closes spec §4 and unblocks AM-61's countdown. One aggregate number, the larger of
whichever limiter refused, and never a word about which one that was.

**Files:**
- Modify: `apps/api/crates/runtime/src/adapter/redis/rate_limit.rs` (the Lua, `allow`, `allow_login`)
- Modify: `apps/api/crates/runtime/src/shared/errors.rs:36-38` (the `details` doc), `:71-74` (a new constructor)
- Modify: `apps/api/crates/runtime/src/adapter/http/auth.rs:190-216` (`login`)
- Test: `apps/api/crates/runtime/tests/auth_flow.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `rate_limit::Attempt { pub allowed: bool, pub retry_after_seconds: u64 }`
  - `rate_limit::LoginAttempt { Allowed, Refused { retry_after_seconds: u64 } }`
  - `RateLimiter::allow(&self, key: &str, limit: u32) -> Result<Attempt, redis::RedisError>` (signature change)
  - `RateLimiter::allow_login(&self, ip: &str, email: &str) -> Result<LoginAttempt, redis::RedisError>` (signature change)
  - `ApiError::too_many_requests_in(seconds: u64) -> ApiError`

**TDD: yes** — the aggregation rule is pure and is the part that can be wrong in a way
that leaks. Extract it as a private function and test it without Redis.

**Facts you will not discover from the plan text:**
- `allow` has exactly one caller, `allow_login`, and `allow_login` has exactly one
  caller, `login` at `adapter/http/auth.rs:203`. Changing both signatures touches three
  call sites total. Verified by grep across `runtime/src`.
- Redis `TTL` returns `-1` for a key with no expiry and `-2` for a key that does not
  exist. Neither is reachable straight after the script's `INCR`+`EXPIRE`, and both must
  still be handled without a panic — `.unwrap()` is denied on production paths.
- redis-rs deserialises a Lua array into a Rust tuple, so returning `{count, ttl}` from
  the script and receiving `(u32, i64)` works without a custom `FromRedisValue`.
- This is **not** an account-existence oracle, and spec §8 records why: the account key
  counts `token_digest(email)` before any user lookup, so an unregistered address is
  counted identically to a registered one. Taking the **larger** of the two waits is
  what keeps it that way — reporting only the refusing limiter's own remaining window
  would say which limiter refused.

- [ ] **Step 1: Write the failing tests**

Add to `apps/api/crates/runtime/src/adapter/redis/rate_limit.rs`'s test module:

```rust
    #[test]
    fn an_allowed_attempt_reports_no_refusal() {
        let ok = Attempt {
            allowed: true,
            retry_after_seconds: 900,
        };
        assert_eq!(refusal(&ok, &ok), None);
    }

    #[test]
    fn a_refusal_reports_the_longer_of_the_two_waits() {
        // The two windows started at different moments, so their remaining
        // times differ. Reporting the refusing limiter's own wait would tell
        // the caller which limiter refused; the larger of the two does not.
        let ip = Attempt {
            allowed: false,
            retry_after_seconds: 42,
        };
        let account = Attempt {
            allowed: false,
            retry_after_seconds: 611,
        };
        assert_eq!(refusal(&ip, &account), Some(611));
        assert_eq!(refusal(&account, &ip), Some(611));
    }

    #[test]
    fn one_limiter_refusing_still_reports_the_longer_wait_of_the_two() {
        // Both counters were incremented, so both have a live window. The
        // answer must not depend on which one said no.
        let allowed_long = Attempt {
            allowed: true,
            retry_after_seconds: 800,
        };
        let refused_short = Attempt {
            allowed: false,
            retry_after_seconds: 30,
        };
        assert_eq!(refusal(&allowed_long, &refused_short), Some(800));
        assert_eq!(refusal(&refused_short, &allowed_long), Some(800));
    }

    #[test]
    fn a_nonsense_ttl_never_becomes_a_zero_second_countdown() {
        // Redis answers -1 for no expiry and -2 for no key. Telling somebody to
        // wait zero seconds sends them straight back into the same wall.
        assert_eq!(seconds_from_ttl(-2), WINDOW.as_secs());
        assert_eq!(seconds_from_ttl(-1), WINDOW.as_secs());
        assert_eq!(seconds_from_ttl(0), 1);
        assert_eq!(seconds_from_ttl(37), 37);
    }
```

And add to `apps/api/crates/runtime/tests/auth_flow.rs`:

```rust
#[tokio::test]
async fn a_throttled_login_says_how_long_to_wait() {
    // AM-61's countdown. One aggregate number and nothing else — no attempts
    // remaining, and no hint about which limiter refused.
    let app = app!();
    let peer = a_peer();

    let mut throttled = None;
    for _ in 0..40 {
        let response = post(
            &app,
            "/auth/login",
            json!({"email": an_email(), "password": "salah"}),
            peer,
        )
        .await;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            throttled = Some(json(response).await);
            break;
        }
    }

    let body = throttled.expect("40 failed logins from one address were all allowed");
    assert_eq!(body["error"]["code"], "too_many_requests");

    let wait = body["error"]["details"]["retry_after_seconds"]
        .as_u64()
        .expect("retry_after_seconds must be a number");
    assert!(wait > 0, "a zero-second countdown is not a countdown");
    assert!(wait <= 15 * 60, "the wait cannot exceed the window: {wait}");

    // Nothing may say which limiter refused, or how many attempts are left.
    let details = body["error"]["details"].to_string();
    for leak in ["ip", "account", "remaining", "attempts", "limit"] {
        assert!(
            !details.contains(leak),
            "the 429 detail leaked `{leak}`: {details}"
        );
    }
}

#[tokio::test]
async fn a_successful_login_carries_no_retry_hint() {
    let app = app!();
    let (email, peer) = (an_email(), a_peer());

    post(
        &app,
        "/auth/register",
        json!({"email": email, "password": "kata sandi panjang"}),
        peer,
    )
    .await;

    let body = json(
        post(
            &app,
            "/auth/login",
            json!({"email": email, "password": "kata sandi panjang"}),
            a_peer(),
        )
        .await,
    )
    .await;
    assert!(body.get("error").is_none());
    assert!(body["data"]["access_token"].is_string());
}
```

Note: `a_throttled_login_says_how_long_to_wait` will need a `username` field once Task 5
lands. It registers nothing, so it is unaffected; `a_successful_login_carries_no_retry_hint`
does register and Task 5 updates it.

- [ ] **Step 2: Run them to verify they fail**

```bash
make be-test
```

Expected: the unit tests fail to compile (`cannot find function `refusal``), and the
integration test fails on `retry_after_seconds must be a number`.

- [ ] **Step 3: Return the remaining window from the script**

In `rate_limit.rs`, replace the `HIT` script and add the two helpers:

```rust
/// Increment, set the expiry, and report how long the window has left.
///
/// `INCR` followed by a separate `EXPIRE` leaves a window in which the process
/// dies after creating the key and before giving it a lifetime — and a counter
/// with no expiry locks that address or account out permanently. `TTL` rides
/// along in the same script so a caller cannot observe the count and the
/// remaining time from two different moments.
const HIT: &str = r"
    local count = redis.call('INCR', KEYS[1])
    if count == 1 then
        redis.call('EXPIRE', KEYS[1], ARGV[1])
    end
    return {count, redis.call('TTL', KEYS[1])}
";

/// One counted attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attempt {
    /// Whether it is still within the allowance.
    pub allowed: bool,
    /// Seconds until this counter's window resets.
    pub retry_after_seconds: u64,
}

/// What happened when a login attempt was counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginAttempt {
    Allowed,
    /// Refused, with one aggregate wait. It deliberately does not say which
    /// limiter refused and never reports attempts remaining — either would be
    /// the oracle that returning a number at all was challenged for being.
    Refused { retry_after_seconds: u64 },
}

/// Turn a Redis `TTL` reply into a wait a person can be shown.
///
/// `-1` means no expiry and `-2` means no key; neither is reachable directly
/// after the script above, and neither may become a panic or a zero-second
/// countdown that sends somebody back into the same wall.
fn seconds_from_ttl(ttl: i64) -> u64 {
    u64::try_from(ttl).map_or(WINDOW.as_secs(), |seconds| seconds.max(1))
}

/// The wait to report, given both counters.
///
/// `None` when the attempt is allowed. Otherwise the **larger** of the two
/// remaining windows, whichever limiter actually refused: the two windows start
/// at different moments, so reporting the refusing one's own wait would
/// distinguish a per-address refusal from a per-account one.
fn refusal(by_ip: &Attempt, by_account: &Attempt) -> Option<u64> {
    if by_ip.allowed && by_account.allowed {
        return None;
    }
    Some(by_ip.retry_after_seconds.max(by_account.retry_after_seconds))
}
```

Replace `allow` and `allow_login`:

```rust
    /// Record an attempt.
    ///
    /// Counted before the password is checked, so a wrong guess and a right one
    /// cost the same — otherwise the limit would only apply to attackers who
    /// fail, which is not a limit.
    ///
    /// # Errors
    ///
    /// Returns [`redis::RedisError`] when the store is unreachable. The caller
    /// decides whether to fail open or closed; on the login path it fails
    /// closed, because an unthrottled login is worse than a brief outage.
    pub async fn allow(&self, key: &str, limit: u32) -> Result<Attempt, redis::RedisError> {
        let mut conn = self.redis.clone();
        let (count, ttl): (u32, i64) = Script::new(HIT)
            .key(key)
            .arg(WINDOW.as_secs())
            .invoke_async(&mut conn)
            .await?;

        Ok(Attempt {
            allowed: count <= limit,
            retry_after_seconds: seconds_from_ttl(ttl),
        })
    }

    /// Check both limits for a login attempt.
    ///
    /// Both are recorded even when the first already failed, so an attacker
    /// cannot use one limit to shield the other from counting.
    ///
    /// # Errors
    ///
    /// Returns [`redis::RedisError`] when the store is unreachable.
    pub async fn allow_login(&self, ip: &str, email: &str) -> Result<LoginAttempt, redis::RedisError> {
        let by_ip = self.allow(&format!("rl:login:ip:{ip}"), PER_IP).await?;
        let by_account = self
            .allow(
                &format!("rl:login:acct:{}", token_digest(email)),
                PER_ACCOUNT,
            )
            .await?;

        Ok(refusal(&by_ip, &by_account).map_or(LoginAttempt::Allowed, |retry_after_seconds| {
            LoginAttempt::Refused {
                retry_after_seconds,
            }
        }))
    }
```

- [ ] **Step 4: Let the error carry the number**

In `apps/api/crates/runtime/src/shared/errors.rs`, widen the `details` field doc — it
currently says "Field-level detail for a validation failure":

```rust
    /// Detail the client can act on. Reaches the client, so it carries only
    /// what the client supplied or a value we chose to publish — never a cause.
    details: Option<serde_json::Value>,
```

and add, beside `too_many_requests`:

```rust
    /// 429 that says how long to wait.
    ///
    /// One aggregate number. It never names which limiter refused and never
    /// reports attempts remaining — see `adapter::redis::rate_limit::refusal`
    /// for why the distinction is the whole safety argument.
    #[must_use]
    pub fn too_many_requests_in(seconds: u64) -> Self {
        Self {
            code: ErrorCode::TooManyRequests,
            details: Some(serde_json::json!({ "retry_after_seconds": seconds })),
            source: None,
        }
    }
```

`too_many_requests()` stays — `catalog::to_api_error` and `parts` still use it.

- [ ] **Step 5: Report it from the login handler**

In `adapter/http/auth.rs`, replace the `allowed` block in `login` (lines 201-209):

```rust
    let attempt = state
        .limiter
        .allow_login(&peer.ip().to_string(), &body.email)
        .await
        .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?;

    if let crate::adapter::redis::rate_limit::LoginAttempt::Refused {
        retry_after_seconds,
    } = attempt
    {
        return Err(ApiError::too_many_requests_in(retry_after_seconds));
    }
```

- [ ] **Step 6: Run the tests to verify they pass**

```bash
make be-test
```

Expected: PASS, including the four new unit tests and the two new integration tests.

- [ ] **Step 7: Run the gates yourself**

```bash
make be-fmt && make be-lint && make be-test && make be-boundary
```

Expected: EXIT=0 on each. No SQL changed.

**Acceptance criteria**
- A 429 from `/auth/login` carries `error.details.retry_after_seconds`, a positive
  integer no greater than the window.
- Nothing in the response distinguishes the per-IP limiter from the per-account one, and
  nothing reports attempts remaining — asserted by the leak scan in the test.
- A 200 login carries no `error` key at all.
- A `TTL` of `-1` or `-2` produces the full window rather than a panic or a zero.
- `make be-check` EXIT=0.

---

## Task 5: register takes a username, and `23505` names the field that collided

Two changes that must land together: `CredentialsRequest` splits so `/auth/login`'s
shipped contract stays byte-identical, and the unique-violation mapping stops assuming
the email index — which stopped being the only one the moment Task 2 ran.

**Files:**
- Modify: `apps/api/crates/runtime/src/adapter/http/auth.rs:117-121` (DTO split), `:167-183` (`register`), `:266-280` (`to_api_error`)
- Modify: `apps/api/crates/runtime/src/usecase/auth.rs:20-63` (`AuthError`, `register`)
- Modify: `apps/api/crates/runtime/src/adapter/postgres/user_repo.rs:42-64` (`insert`)
- Modify: `apps/api/crates/runtime/src/shared/errors.rs` (`conflict_on`)
- Test: `apps/api/crates/runtime/tests/auth_flow.rs`

**Interfaces:**
- Consumes: `anakmobil_domain::identity::username::{canonicalise, is_reserved, UsernameError}` (Task 3); the `users_username_key` index (Task 2).
- Produces:
  - `user_repo::NewUser<'a> { id: Uuid, email: &'a str, username: &'a str, password_hash: &'a str }`
  - `user_repo::insert(conn: &mut PgConnection, user: NewUser<'_>) -> Result<(), sqlx::Error>`
  - `usecase::auth::Registration<'a> { email: &'a str, username: &'a str, password: &'a str }`
  - `usecase::auth::register(pool, sessions, input: Registration<'_>) -> Result<TokenPair, AuthError>`
  - `AuthError::UsernameTaken`
  - `ApiError::conflict_on(field: &'static str, message: &'static str) -> ApiError`
  - `http::auth::RegistrationRequest { email, username, password }` — `CredentialsRequest` keeps only `email` and `password`

**TDD: yes** — the collision mapping has a stateable failing test that is also a real
defect: a username collision currently reports a taken *email*, which sends somebody to
change the wrong field.

**Facts you will not discover from the plan text:**
- `CredentialsRequest` is used by **both** `register` and `login` today
  (`auth.rs:174`, `:193`). Adding `username` to it would make `/auth/login` demand one
  and break the shipped contract, which is the entire reason for the split.
- `sqlx`'s `Box<dyn DatabaseError>` exposes `.constraint() -> Option<&str>`, and for a
  unique-index violation it reports the **index** name. Task 2 named the index
  explicitly for this. Confirm the email index's real name from Task 2's `\d users`
  output before hard-coding `users_email_key`.
- `register`'s new argument count would be five with two adjacent `&str` that compile
  when swapped — `("budi@example.com", "budi", …)` and `("budi", "budi@example.com", …)`
  are both valid Rust. `user_repo::RoleChangeRow` exists in this codebase for exactly
  this reason; follow it.
- Every existing register call in `auth_flow.rs` needs a `username` field added, and each
  must be unique per run for the same reason `an_email()` is. There are register calls in
  `register_then_login_then_use_the_token`, `a_password_is_never_echoed_back`,
  `an_unknown_email_and_a_wrong_password_are_indistinguishable`,
  `a_short_password_is_refused_with_a_field_message`, `a_taken_email_is_refused`,
  `email_matching_ignores_case`, `logout_stops_the_next_request`,
  `refreshing_rotates_and_a_replay_is_refused`, plus Task 1's and Task 4's new tests.
- `a_taken_email_is_refused` reuses one body twice; it now needs the same email with a
  **different** username, or it proves nothing about which index fired.

- [ ] **Step 1: Write the failing tests**

Add a helper beside `an_email()` in `tests/auth_flow.rs`:

```rust
/// A unique, always-valid username per test run.
fn a_username() -> String {
    // Canonical by construction: lowercase hex, no dots, no edges.
    format!("u{}", uuid::Uuid::now_v7().simple())[..20].to_owned()
}
```

and the new tests:

```rust
#[tokio::test]
async fn a_taken_username_is_reported_as_a_username_not_an_email() {
    // The defect: `23505` was mapped to EmailTaken with a comment saying a
    // unique violation "can only be the email index". Adding the username index
    // made that false, and somebody would be told to change an address that is
    // perfectly free.
    let app = app!();
    let username = a_username();

    assert_eq!(
        post(
            &app,
            "/auth/register",
            json!({"email": an_email(), "username": username, "password": "kata sandi panjang"}),
            a_peer(),
        )
        .await
        .status(),
        StatusCode::CREATED
    );

    // A different address, the same name.
    let clash = post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": username, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;
    assert_eq!(clash.status(), StatusCode::CONFLICT);

    let body = json(clash).await;
    assert_eq!(body["error"]["code"], "conflict");
    assert!(
        body["error"]["details"]["username"].is_string(),
        "the collision must name the username: {body}"
    );
    assert!(
        body["error"]["details"].get("email").is_none(),
        "a username collision must not be reported against the email: {body}"
    );
}

#[tokio::test]
async fn a_taken_email_names_the_email() {
    let app = app!();
    let email = an_email();

    post(
        &app,
        "/auth/register",
        json!({"email": email, "username": a_username(), "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;

    let clash = post(
        &app,
        "/auth/register",
        json!({"email": email, "username": a_username(), "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;
    assert_eq!(clash.status(), StatusCode::CONFLICT);

    let body = json(clash).await;
    assert!(body["error"]["details"]["email"].is_string(), "{body}");
    assert!(body["error"]["details"].get("username").is_none(), "{body}");
}

#[tokio::test]
async fn a_username_is_canonicalised_before_it_is_stored() {
    // Uppercase in, lowercase held. The second registration proves the first
    // one claimed the canonical form rather than the typed form.
    let app = app!();
    let username = a_username();

    assert_eq!(
        post(
            &app,
            "/auth/register",
            json!({"email": an_email(), "username": username.to_uppercase(), "password": "kata sandi panjang"}),
            a_peer(),
        )
        .await
        .status(),
        StatusCode::CREATED
    );

    assert_eq!(
        post(
            &app,
            "/auth/register",
            json!({"email": an_email(), "username": username, "password": "kata sandi panjang"}),
            a_peer(),
        )
        .await
        .status(),
        StatusCode::CONFLICT,
        "`BUDI` and `budi` must be one name"
    );
}

#[tokio::test]
async fn a_malformed_username_is_a_field_level_validation_failure() {
    let app = app!();

    for bad in ["ab", ".budi", "budi.", "budi..s", "budi-santoso", "budi santoso"] {
        let response = post(
            &app,
            "/auth/register",
            json!({"email": an_email(), "username": bad, "password": "kata sandi panjang"}),
            a_peer(),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "`{bad}` should be refused"
        );

        let body = json(response).await;
        assert_eq!(body["error"]["code"], "validation_failed");
        assert!(
            body["error"]["details"]["username"].is_string(),
            "`{bad}` gave no message under the username field: {body}"
        );
    }
}

#[tokio::test]
async fn a_reserved_username_answers_exactly_like_a_taken_one() {
    // The guard rail from the spec: nothing may distinguish reserved from taken,
    // or the endpoint becomes a way to enumerate the reserved list.
    let app = app!();
    let taken = a_username();

    post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": taken, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;

    let on_taken = post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": taken, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;
    let on_reserved = post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": "admin", "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;

    assert_eq!(on_taken.status(), on_reserved.status());

    let (a, b) = (json(on_taken).await, json(on_reserved).await);
    assert_eq!(a["error"], b["error"], "reserved and taken must be identical");
}

#[tokio::test]
async fn login_still_takes_only_an_email_and_a_password() {
    // The shipped contract. Splitting the DTO exists so that adding a username
    // to registration cannot make `/auth/login` demand one.
    let app = app!();
    let (email, password) = (an_email(), "kata sandi panjang");

    post(
        &app,
        "/auth/register",
        json!({"email": email, "username": a_username(), "password": password}),
        a_peer(),
    )
    .await;

    let response = post(
        &app,
        "/auth/login",
        json!({"email": email, "password": password}),
        a_peer(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}
```

- [ ] **Step 2: Run them to verify they fail**

```bash
make be-test
```

Expected: the register calls return `422` because `username` is an unknown field — no,
`serde` ignores unknown fields by default, so they return `201` and then
`a_taken_username_is_reported_as_a_username_not_an_email` fails on
`body["error"]["details"]["username"].is_string()`. Confirm that is the failure you see;
if the tests fail earlier, fix the tests first.

- [ ] **Step 3: The repository takes a row struct**

In `apps/api/crates/runtime/src/adapter/postgres/user_repo.rs`, replace `insert`:

```rust
/// An account on its way to being created.
///
/// A struct rather than four positional arguments: three adjacent `&str` are
/// exactly the shape that gets swapped at a call site and still compiles.
/// `RoleChangeRow` below is the same fix for the same reason.
pub struct NewUser<'a> {
    pub id: Uuid,
    pub email: &'a str,
    pub username: &'a str,
    pub password_hash: &'a str,
}

/// Create an account.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails, including a unique violation
/// on either `users_email_key` or `users_username_key` — which the caller must
/// translate by constraint name, because the two send somebody to different
/// fields.
pub async fn insert(conn: &mut PgConnection, user: NewUser<'_>) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO users (id, email, username, password_hash)
        VALUES ($1, $2::citext, $3::citext, $4)
        "#,
        user.id,
        user.email,
        user.username,
        user.password_hash
    )
    .execute(conn)
    .await
    .map(drop)
}
```

- [ ] **Step 4: The use case takes an input struct and reads the constraint**

In `apps/api/crates/runtime/src/usecase/auth.rs`, add the variant to `AuthError`:

```rust
    #[error("that username is already taken")]
    UsernameTaken,
```

and replace `register`:

```rust
/// The unique indexes a registration can collide with.
///
/// Named rather than inlined: each string appears in the match below and in the
/// migration that created it, and a typo in one of them would silently route a
/// collision to the fallback.
const USERS_EMAIL_KEY: &str = "users_email_key";
const USERS_USERNAME_KEY: &str = "users_username_key";

/// What a registration supplies.
///
/// A struct rather than three positional `&str`: `("budi@example.com", "budi", …)`
/// and `("budi", "budi@example.com", …)` both compile, and only one of them is
/// right.
pub struct Registration<'a> {
    pub email: &'a str,
    pub username: &'a str,
    pub password: &'a str,
}

/// Create an account and sign it in.
///
/// The username arrives already canonicalised — the HTTP layer calls
/// `identity::username::canonicalise` so that a malformed name is a validation
/// failure with a field message rather than a database error.
///
/// # Errors
///
/// [`AuthError::EmailTaken`] or [`AuthError::UsernameTaken`] when the
/// corresponding unique index fires, otherwise a storage error.
pub async fn register(
    pool: &PgPool,
    sessions: &SessionStore,
    input: Registration<'_>,
) -> Result<TokenPair, AuthError> {
    let hash = security::hash_password(input.password)?;
    let id = Uuid::now_v7();

    let mut tx = pool.begin().await?;
    let result = user_repo::insert(
        &mut tx,
        user_repo::NewUser {
            id,
            email: input.email,
            username: input.username,
            password_hash: &hash,
        },
    )
    .await;

    match result {
        Ok(()) => {
            tx.commit().await?;
            Ok(sessions.create(id).await?)
        }
        // 23505 is a unique violation. It used to be mapped straight to
        // EmailTaken with a comment saying it "can only be the email index" —
        // true until `users_username_key` existed, and afterwards a way to tell
        // somebody to change an address that was never the problem.
        Err(sqlx::Error::Database(err)) if err.code().as_deref() == Some("23505") => {
            match err.constraint() {
                Some(USERS_USERNAME_KEY) => Err(AuthError::UsernameTaken),
                Some(USERS_EMAIL_KEY) => Err(AuthError::EmailTaken),
                // A third unique index nobody mapped. Guessing which field
                // collided would be worse than a 500 that shows up in the log
                // with its constraint name attached.
                _ => Err(AuthError::Database(sqlx::Error::Database(err))),
            }
        }
        Err(err) => Err(err.into()),
    }
}
```

- [ ] **Step 5: A 409 that names its field**

In `apps/api/crates/runtime/src/shared/errors.rs`, beside `conflict`:

```rust
    /// 409 that names the field that collided.
    ///
    /// Registration inevitably reveals that an address or a name is taken —
    /// there is no way to refuse a duplicate without saying so. Naming which of
    /// the two it was does not widen that, and not naming it sends somebody to
    /// change the wrong field. This has no bearing on `/auth/login`, where an
    /// unknown email and a wrong password stay indistinguishable.
    #[must_use]
    pub fn conflict_on(field: &'static str, message: &'static str) -> Self {
        Self {
            code: ErrorCode::Conflict,
            details: Some(serde_json::json!({ field: message })),
            source: None,
        }
    }
```

- [ ] **Step 6: Split the DTO and canonicalise at the boundary**

In `apps/api/crates/runtime/src/adapter/http/auth.rs`, add the import and the new DTO:

```rust
use anakmobil_domain::identity::username::{self, UsernameError};
```

```rust
/// What `/auth/login` takes, and nothing else.
///
/// Registration has its own type. Sharing one would have meant adding
/// `username` here, which would make the shipped login contract demand a field
/// no client sends.
#[derive(Debug, Deserialize)]
pub struct CredentialsRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RegistrationRequest {
    pub email: String,
    pub username: String,
    pub password: String,
}
```

Add the message mapping and the shape check:

```rust
/// The Bahasa Indonesia message for each way a username can be wrong.
///
/// The domain crate has no i18n and should not grow one — its `#[error]`
/// strings are English, for logs. Product text is written here, where the rest
/// of this file's field messages are.
const fn username_message(err: UsernameError) -> &'static str {
    match err {
        UsernameError::TooShort => "Minimal 3 karakter.",
        UsernameError::TooLong => "Maksimal 30 karakter.",
        UsernameError::BadCharacter => "Hanya huruf kecil, angka, titik, dan garis bawah.",
        UsernameError::EdgePunctuation => "Tidak boleh diawali atau diakhiri titik atau garis bawah.",
        UsernameError::ConsecutiveDots => "Titik tidak boleh berurutan.",
    }
}

/// Canonicalise a username, or fail with a message under the field.
fn check_username(raw: &str) -> Result<String, ApiError> {
    username::canonicalise(raw).map_err(|err| {
        ApiError::validation(serde_json::json!({ "username": username_message(err) }))
    })
}
```

Replace `register`:

```rust
/// `POST /auth/register`
///
/// # Errors
///
/// Validation failure, a taken email or username, or a storage failure.
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegistrationRequest>,
) -> Result<ApiResponse<TokensResponse>, ApiError> {
    check_password_shape(&body.password)?;
    let username = check_username(&body.username)?;

    // A reserved name answers exactly as a taken one does. Reporting it as a
    // validation failure instead would answer 422 where a taken name answers
    // 409, and that difference is a way to enumerate the reserved list.
    if username::is_reserved(&username) {
        return Err(to_api_error(AuthError::UsernameTaken));
    }

    let pair = auth::register(
        &state.pool,
        &state.sessions,
        auth::Registration {
            email: &body.email,
            username: &username,
            password: &body.password,
        },
    )
    .await
    .map_err(to_api_error)?;

    Ok(ApiResponse::created(pair.into()))
}
```

and extend `to_api_error`:

```rust
        AuthError::EmailTaken => ApiError::conflict_on("email", "Email ini sudah terdaftar."),
        AuthError::UsernameTaken => {
            ApiError::conflict_on("username", "Username ini sudah dipakai.")
        }
```

- [ ] **Step 7: Update every existing register call**

Add `"username": a_username()` to each `json!` body posted to `/auth/register` in
`tests/auth_flow.rs`. In `a_taken_email_is_refused`, the two bodies must share an email
and differ in username, or the test no longer proves which index fired.

- [ ] **Step 8: Run the tests, regenerate the cache, run the gates**

```bash
make be-prepare
make be-test
make be-fmt && make be-lint && make be-boundary && make be-sqlx-check
```

Expected: EXIT=0 on each, six new tests passing. `insert`'s SQL changed, so the `.sqlx`
regeneration is mandatory and its output is committed.

**Acceptance criteria**
- `/auth/login` accepts exactly `{email, password}`, unchanged.
- `/auth/register` requires `{email, username, password}` and stores the canonical
  username.
- A username collision is a 409 naming `username`; an email collision is a 409 naming
  `email`; neither names the other.
- A reserved name and a taken name produce byte-identical `error` objects.
- A malformed username is a 422 with a Bahasa Indonesia message under `username`.
- A unique violation on an unmapped index is a 500, not a guess.
- `make be-check` and `make be-sqlx-check` both EXIT=0.

---

## Task 6: `GET /me` and `PATCH /me`

The bootstrap endpoint. Onboarding completion is **derived, never stored** — a person
who has a car has finished onboarding, and a stored flag is a second source of truth
free to disagree with the first.

**Files:**
- Create: `apps/api/crates/runtime/src/usecase/profile.rs`
- Create: `apps/api/crates/runtime/src/adapter/http/profile.rs`
- Create: `apps/api/crates/runtime/tests/profile_flow.rs`
- Modify: `apps/api/crates/runtime/src/adapter/postgres/user_repo.rs` (`Profile`, `profile_of`, `set_display_name`)
- Modify: `apps/api/crates/runtime/src/usecase/mod.rs`, `apps/api/crates/runtime/src/adapter/http/mod.rs` (module + two routes)

**Interfaces:**
- Consumes: `Authenticated.user_id` (Task 1); `users.username` / `users.display_name` (Task 2).
- Produces:
  - `user_repo::Profile { id: Uuid, email: String, username: Option<String>, display_name: Option<String>, has_vehicles: bool }`
  - `user_repo::profile_of(conn, id) -> Result<Option<Profile>, sqlx::Error>`
  - `user_repo::set_display_name(conn, id, display_name: &str) -> Result<(), sqlx::Error>`
  - `usecase::profile::{ProfileError, me, update_display_name}`
  - `http::profile::{MeResponse, ProfileUpdateRequest, me, update_me}`
  - Routes `GET /me` and `PATCH /me`

**TDD: yes** — "reflects each onboarding stage" is a four-state truth table, and the
display-name rule is an input-to-output check.

**Facts you will not discover from the plan text:**
- **sqlx cannot map `citext` to `String`.** `email` and `username` are `CITEXT`, so both
  need an explicit `::text` cast in the `SELECT` or the generated `.sqlx` entry carries
  an unknown user type and the macro refuses. `find_id_by_email` only ever casts the
  *parameter*; this is the first query in the repo that *reads* a citext column.
- `EXISTS(...)` and a cast both come back as `Option<T>` from the macro unless you use
  the `"name!"` force-not-null syntax. `admin_count` at `user_repo.rs:174` shows the
  `.map(|c| c.unwrap_or(0))` alternative; the `!` suffix is cleaner for a struct.
- `vehicles_owner_position_idx ON vehicles (owner_id, position, id)` already exists, so
  the `EXISTS` sub-select is an index probe, not a scan.
- `Option<String>` fields in a serde `Deserialize` struct default to `None` when the key
  is absent — no `#[serde(default)]` needed. That is what makes `PATCH` with an empty
  body a no-op rather than a 422.
- `usecase/mod.rs` lists its modules alphabetically; keep it that way.

- [ ] **Step 1: Write the failing tests**

Create `apps/api/crates/runtime/tests/profile_flow.rs`. Copy the `app!` macro,
`an_email`, `a_peer`, `post`, `post_with_auth`, and `json` helpers verbatim from
`auth_flow.rs:9-140` — the integration suites in this repo are self-contained by
convention, and a shared harness would be a fourth thing to keep in sync. Add `a_username()`
from Task 5, plus `get_with_auth` and `patch_with_auth`:

```rust
async fn send_with_auth(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Option<Value>,
    bearer: &str,
) -> Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("authorization", format!("Bearer {bearer}"))
        .header("content-type", "application/json");
    if body.is_none() {
        builder = builder.header("content-length", "0");
    }
    let mut request = builder
        .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
        .expect("building the request");
    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(a_peer()));
    app.clone()
        .oneshot(request)
        .await
        .expect("the router is infallible")
}

/// Register and return the access token, so each test starts from a real account.
async fn an_account(app: &axum::Router) -> String {
    let body = json(
        post(
            app,
            "/auth/register",
            json!({"email": an_email(), "username": a_username(), "password": "kata sandi panjang"}),
            a_peer(),
        )
        .await,
    )
    .await;
    body["data"]["access_token"]
        .as_str()
        .expect("access token")
        .to_owned()
}
```

then the tests:

```rust
#[tokio::test]
async fn me_reports_a_fresh_account_as_onboarding_incomplete() {
    // The state a person is in the instant after registering: a username,
    // because register demanded one, and nothing else yet.
    let app = app!();
    let token = an_account(&app).await;

    let response = send_with_auth(&app, "GET", "/me", None, &token).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = json(response).await;
    assert!(body["data"]["id"].is_string());
    assert!(body["data"]["email"].is_string());
    assert!(body["data"]["username"].is_string());
    assert!(
        body["data"]["display_name"].is_null(),
        "a fresh account has no display name: {body}"
    );
    assert_eq!(
        body["data"]["has_vehicles"], false,
        "a fresh account has no car"
    );
}

#[tokio::test]
async fn me_never_returns_the_password_hash() {
    let app = app!();
    let token = an_account(&app).await;

    let response = send_with_auth(&app, "GET", "/me", None, &token).await;
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let text = String::from_utf8(bytes.to_vec()).expect("utf-8");

    assert!(!text.contains("argon2"), "the hash reached the client: {text}");
    assert!(
        !text.contains("password"),
        "the response mentions a password: {text}"
    );
}

#[tokio::test]
async fn me_refuses_a_caller_without_a_token() {
    let app = app!();
    let request = Request::builder()
        .method("GET")
        .uri("/me")
        .body(Body::empty())
        .expect("building the request");
    let response = app.oneshot(request).await.expect("infallible");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn setting_a_display_name_moves_the_account_past_the_profile_step() {
    let app = app!();
    let token = an_account(&app).await;

    let response = send_with_auth(
        &app,
        "PATCH",
        "/me",
        Some(json!({"display_name": "  Budi Santoso  "})),
        &token,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = json(response).await;
    assert_eq!(
        body["data"]["display_name"], "Budi Santoso",
        "surrounding whitespace should be trimmed, not stored"
    );

    // And it is the same shape GET answers with, so one client parser covers both.
    let fetched = json(send_with_auth(&app, "GET", "/me", None, &token).await).await;
    assert_eq!(fetched["data"], body["data"]);
}

#[tokio::test]
async fn an_empty_patch_changes_nothing_and_still_answers_the_profile() {
    // A PATCH with no keys is not an error. It is how a client asks for the
    // current state without caring which fields it might have sent.
    let app = app!();
    let token = an_account(&app).await;

    send_with_auth(
        &app,
        "PATCH",
        "/me",
        Some(json!({"display_name": "Budi"})),
        &token,
    )
    .await;

    let response = send_with_auth(&app, "PATCH", "/me", Some(json!({})), &token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["data"]["display_name"], "Budi");
}

#[tokio::test]
async fn a_blank_or_oversized_display_name_is_refused_under_its_field() {
    let app = app!();
    let token = an_account(&app).await;

    for bad in ["", "   ", &"a".repeat(61)] {
        let response = send_with_auth(
            &app,
            "PATCH",
            "/me",
            Some(json!({"display_name": bad})),
            &token,
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "`{bad}` should be refused"
        );
        let body = json(response).await;
        assert!(body["error"]["details"]["display_name"].is_string(), "{body}");
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

```bash
make be-test
```

Expected: every test in the new file fails with `404 Not Found`, because neither route
exists. That is the correct first failure.

- [ ] **Step 3: The repository reads and writes the profile**

Add to `apps/api/crates/runtime/src/adapter/postgres/user_repo.rs`:

```rust
/// An account as the app's bootstrap needs it.
///
/// Deliberately a different struct from [`Credentials`]: that one carries the
/// password hash and is deliberately not `Serialize`. Keeping the two apart is
/// what makes it impossible to hand a hash to a response by widening a query.
#[derive(Debug, Clone)]
pub struct Profile {
    pub id: Uuid,
    pub email: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    /// Derived, never stored. A person who has a car has finished onboarding,
    /// and a stored completion flag is a second source of truth that can
    /// disagree with the first.
    pub has_vehicles: bool,
}

/// This account's profile and whether it has any car.
///
/// One query rather than two. The `::text` casts are required: `email` and
/// `username` are `CITEXT`, and sqlx has no mapping for it — without the cast
/// the macro fails on an unknown type rather than at runtime.
///
/// Complexity: `O(log n)` — a primary-key lookup plus an index probe into
/// `vehicles_owner_position_idx`, which leads on `owner_id`.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn profile_of(conn: &mut PgConnection, id: Uuid) -> Result<Option<Profile>, sqlx::Error> {
    sqlx::query_as!(
        Profile,
        r#"
        SELECT
            u.id,
            u.email::text        AS "email!",
            u.username::text     AS "username?",
            u.display_name       AS "display_name?",
            EXISTS(SELECT 1 FROM vehicles v WHERE v.owner_id = u.id) AS "has_vehicles!"
        FROM users u
        WHERE u.id = $1
        "#,
        id
    )
    .fetch_optional(conn)
    .await
}

/// Write the display name. Already trimmed and length-checked by the caller.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn set_display_name(
    conn: &mut PgConnection,
    id: Uuid,
    display_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE users SET display_name = $2 WHERE id = $1"#,
        id,
        display_name
    )
    .execute(conn)
    .await
    .map(drop)
}
```

- [ ] **Step 4: The use case**

Create `apps/api/crates/runtime/src/usecase/profile.rs`:

```rust
//! Reading and changing the caller's own profile.
//!
//! Separate from [`crate::usecase::auth`], which owns credentials and sessions.
//! Nothing here authenticates; every function takes a user id the HTTP layer
//! already proved.

use sqlx::PgPool;
use uuid::Uuid;

use crate::adapter::postgres::user_repo::{self, Profile};

/// Why a profile operation did not succeed.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    /// The session outlived the row it points at. Refused rather than assumed.
    #[error("no such account")]
    NotFound,
    #[error("the database could not be reached")]
    Database(#[from] sqlx::Error),
}

/// This account's identity and derived onboarding state.
///
/// # Errors
///
/// [`ProfileError::NotFound`] when the account is gone, otherwise a storage
/// error.
pub async fn me(pool: &PgPool, user_id: Uuid) -> Result<Profile, ProfileError> {
    let mut conn = pool.acquire().await?;
    user_repo::profile_of(&mut conn, user_id)
        .await?
        .ok_or(ProfileError::NotFound)
}

/// Set the display name and answer with the profile that results.
///
/// One transaction: the write and the read-back cannot straddle a concurrent
/// change, so the response is the state that was actually committed rather than
/// a hopeful echo of the request.
///
/// # Errors
///
/// [`ProfileError::NotFound`] when the account is gone, otherwise a storage
/// error.
pub async fn update_display_name(
    pool: &PgPool,
    user_id: Uuid,
    display_name: &str,
) -> Result<Profile, ProfileError> {
    let mut tx = pool.begin().await?;
    user_repo::set_display_name(&mut tx, user_id, display_name).await?;
    let profile = user_repo::profile_of(&mut tx, user_id)
        .await?
        .ok_or(ProfileError::NotFound)?;
    tx.commit().await?;
    Ok(profile)
}
```

Add `pub mod profile;` to `usecase/mod.rs`, alphabetically between `person`-less
neighbours `parts` and `roles`.

- [ ] **Step 5: The HTTP adapter**

Create `apps/api/crates/runtime/src/adapter/http/profile.rs`:

```rust
//! The caller's own profile, and the username namespace.
//!
//! `profile.rs` rather than `identity.rs`: `auth.rs` is already the identity
//! adapter, and these endpoints are specifically about the profile a person
//! shows the world rather than about proving who they are.

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::adapter::http::auth::Authenticated;
use crate::adapter::postgres::user_repo::Profile;
use crate::platform::state::AppState;
use crate::shared::errors::ApiError;
use crate::shared::response::ApiResponse;
use crate::usecase::profile::{self, ProfileError};

/// What the app reads once at launch to decide where to send somebody.
///
/// `username` and `display_name` are nullable and that is the whole point:
/// their absence is how the client knows onboarding is unfinished. There is no
/// completion flag — `has_vehicles` is derived from the garage, so it cannot
/// disagree with the garage.
#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub id: Uuid,
    pub email: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub has_vehicles: bool,
}

impl From<Profile> for MeResponse {
    fn from(profile: Profile) -> Self {
        Self {
            id: profile.id,
            email: profile.email,
            username: profile.username,
            display_name: profile.display_name,
            has_vehicles: profile.has_vehicles,
        }
    }
}

/// The longest display name a profile header renders without truncating.
const MAX_DISPLAY_NAME: usize = 60;

#[derive(Debug, Deserialize)]
pub struct ProfileUpdateRequest {
    /// Absent means "leave it alone". serde defaults an `Option` field to
    /// `None` when the key is missing, which is what makes an empty PATCH a
    /// no-op rather than a rejection.
    pub display_name: Option<String>,
}

impl ProfileUpdateRequest {
    /// The trimmed name to write, or `None` when the request asks for no change.
    fn check(&self) -> Result<Option<&str>, ApiError> {
        let Some(raw) = self.display_name.as_deref() else {
            return Ok(None);
        };
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            return Err(field_error("display_name", "Nama tidak boleh kosong."));
        }
        if trimmed.chars().count() > MAX_DISPLAY_NAME {
            return Err(field_error(
                "display_name",
                "Nama terlalu panjang. Maksimal 60 karakter.",
            ));
        }

        Ok(Some(trimmed))
    }
}

fn field_error(field: &'static str, message: &'static str) -> ApiError {
    ApiError::validation(serde_json::json!({ field: message }))
}

/// `GET /me`
///
/// # Errors
///
/// A storage failure, or an account that no longer exists.
pub async fn me(
    State(state): State<AppState>,
    caller: Authenticated,
) -> Result<ApiResponse<MeResponse>, ApiError> {
    let profile = profile::me(&state.pool, caller.user_id)
        .await
        .map_err(to_api_error)?;
    Ok(ApiResponse::ok(profile.into()))
}

/// `PATCH /me`
///
/// Answers with the same shape as [`me`], so a client writes one parser.
///
/// # Errors
///
/// Validation failure, a storage failure, or an account that no longer exists.
pub async fn update_me(
    State(state): State<AppState>,
    caller: Authenticated,
    Json(body): Json<ProfileUpdateRequest>,
) -> Result<ApiResponse<MeResponse>, ApiError> {
    let profile = match body.check()? {
        Some(display_name) => profile::update_display_name(&state.pool, caller.user_id, display_name)
            .await
            .map_err(to_api_error)?,
        None => profile::me(&state.pool, caller.user_id)
            .await
            .map_err(to_api_error)?,
    };

    Ok(ApiResponse::ok(profile.into()))
}

fn to_api_error(err: ProfileError) -> ApiError {
    match err {
        // A session that outlived its account. Refused rather than assumed —
        // the same failure-closed choice the Admin extractor makes.
        ProfileError::NotFound => ApiError::unauthorized(),
        ProfileError::Database(inner) => ApiError::internal(anyhow::anyhow!(inner)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(display_name: Option<&str>) -> ProfileUpdateRequest {
        ProfileUpdateRequest {
            display_name: display_name.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn an_absent_field_asks_for_no_change() {
        assert_eq!(request(None).check().ok().flatten(), None);
    }

    #[test]
    fn whitespace_is_trimmed_rather_than_stored() {
        assert_eq!(
            request(Some("  Budi Santoso  ")).check().ok().flatten(),
            Some("Budi Santoso")
        );
    }

    #[test]
    fn a_name_of_only_spaces_is_not_a_name() {
        assert!(request(Some("   ")).check().is_err());
        assert!(request(Some("")).check().is_err());
    }

    #[test]
    fn the_length_bound_counts_characters() {
        // Sixty emoji is sixty characters and 240 bytes. Counting bytes would
        // refuse a name well inside the limit.
        assert!(request(Some(&"a".repeat(MAX_DISPLAY_NAME))).check().is_ok());
        assert!(
            request(Some(&"a".repeat(MAX_DISPLAY_NAME + 1)))
                .check()
                .is_err()
        );
    }
}
```

Add `pub mod profile;` to `adapter/http/mod.rs`'s module list (alphabetical, after
`probe`), and the routes inside `router()` beside the auth block:

```rust
        .route("/me", get(profile::me).patch(profile::update_me))
```

- [ ] **Step 6: Run the tests, regenerate the cache, run the gates**

```bash
make be-prepare
make be-test
make be-fmt && make be-lint && make be-boundary && make be-sqlx-check
```

Expected: EXIT=0 on each; six integration tests and four unit tests passing. Two new
queries, so `make be-prepare` is mandatory and its `.sqlx/` output is committed.

**Acceptance criteria**
- `GET /me` answers `{id, email, username, display_name, has_vehicles}` and nothing else.
- `has_vehicles` is computed from `vehicles`, in the same query, with no stored flag
  anywhere.
- No password hash, no token, and no `plat`/VIN reaches the response.
- `PATCH /me` with no keys is a 200 that changes nothing; with a blank or over-long name
  it is a 422 naming `display_name`; with a good one it trims and answers the `GET`
  shape.
- An unauthenticated `GET /me` is a 401.
- `make be-check` and `make be-sqlx-check` both EXIT=0.

---

## Task 7: `GET /usernames/{username}/availability`

Public, rate-limited, and the one endpoint in this codebase that deliberately confirms
something exists. Spec §Username argues why that is defensible for a public namespace
and lists the guard rails; this task implements them.

**Files:**
- Modify: `apps/api/crates/runtime/src/adapter/http/profile.rs` (the handler)
- Modify: `apps/api/crates/runtime/src/adapter/postgres/user_repo.rs` (`username_exists`)
- Modify: `apps/api/crates/runtime/src/adapter/redis/rate_limit.rs` (`PER_IP_LOOKUP`, `allow_lookup`)
- Modify: `apps/api/crates/runtime/src/adapter/http/mod.rs` (one route)
- Test: `apps/api/crates/runtime/tests/profile_flow.rs`

**Interfaces:**
- Consumes: `username::{canonicalise, is_reserved}` (Task 3); `check_username` and
  `username_message` from `http::auth` (Task 5) — make them `pub(crate)` rather than
  writing a second copy; `RateLimiter::allow` returning `Attempt` (Task 4); the
  `users_username_key` index (Task 2).
- Produces:
  - `user_repo::username_exists(conn, canonical: &str) -> Result<bool, sqlx::Error>`
  - `rate_limit::PER_IP_LOOKUP` and `RateLimiter::allow_lookup(&self, ip: &str) -> Result<bool, redis::RedisError>`
  - `http::profile::availability`
  - Route `GET /usernames/{username}/availability`

**TDD: yes** — "reserved answers identically to taken" is the property this endpoint
lives or dies by, and it is exactly expressible as a test.

**Facts you will not discover from the plan text:**
- This is the **first unauthenticated non-probe endpoint** in the router. Every other
  route takes `Authenticated` or `Admin`. Do not add one here — the register screen
  needs it before a session exists, which is the whole reason it is public.
- It therefore also needs `ConnectInfo<SocketAddr>`, and `oneshot` does not run the
  connect-info layer. Every test must insert the extension by hand, exactly as
  `post_with_auth` in `auth_flow.rs:124-127` does.
- A path segment arrives percent-decoded by axum, so `%2F` and spaces reach the handler
  as themselves and are refused by the canonicaliser rather than routed oddly.
- Reuse `check_username` from Task 5 rather than reimplementing. Two copies of the
  boundary check is precisely the "no second copy of the username rules" prohibition,
  one layer in from where the spec aims it.

- [ ] **Step 1: Write the failing tests**

Add to `apps/api/crates/runtime/tests/profile_flow.rs`:

```rust
async fn availability(app: &axum::Router, username: &str) -> Response {
    let mut request = Request::builder()
        .method("GET")
        .uri(format!("/usernames/{username}/availability"))
        .body(Body::empty())
        .expect("building the request");
    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(a_peer()));
    app.clone().oneshot(request).await.expect("infallible")
}

#[tokio::test]
async fn a_free_username_is_available_without_a_session() {
    // Public on purpose: the register screen needs this before a session
    // exists. Every other non-probe route in this router is authenticated.
    let app = app!();
    let response = availability(&app, &a_username()).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["data"]["available"], true);
}

#[tokio::test]
async fn a_taken_username_is_unavailable() {
    let app = app!();
    let username = a_username();

    post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": username, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;

    assert_eq!(
        json(availability(&app, &username).await)["data"]["available"],
        false
    );
}

#[tokio::test]
async fn availability_ignores_case_the_way_the_column_does() {
    let app = app!();
    let username = a_username();

    post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": username, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;

    assert_eq!(
        json(availability(&app, &username.to_uppercase()).await)["data"]["available"],
        false,
        "`BUDI` and `budi` are one name"
    );
}

#[tokio::test]
async fn a_reserved_name_answers_byte_for_byte_like_a_taken_one() {
    // The guard rail the whole endpoint rests on. If these ever differ, the
    // endpoint becomes a way to enumerate the reserved list — and, worse, a
    // precedent for answering "unavailable" in two distinguishable ways.
    let app = app!();
    let taken = a_username();

    post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": taken, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;

    let on_taken = availability(&app, &taken).await;
    let on_reserved = availability(&app, "admin").await;
    assert_eq!(on_taken.status(), on_reserved.status());

    let (a, b) = (json(on_taken).await, json(on_reserved).await);
    assert_eq!(a["data"], b["data"], "reserved and taken must be identical");
}

#[tokio::test]
async fn every_reserved_name_answers_unavailable() {
    let app = app!();
    for name in [
        "about", "admin", "anakmobil", "api", "edit", "help", "login", "me", "new", "profile",
        "register", "settings", "support",
    ] {
        assert_eq!(
            json(availability(&app, name).await)["data"]["available"],
            false,
            "{name} is reserved and must be unavailable"
        );
    }
}

#[tokio::test]
async fn a_malformed_username_is_a_validation_failure_not_an_availability_answer() {
    // Shape is public knowledge — the client mirrors the regex for instant
    // feedback — so refusing a malformed name leaks nothing and is more useful
    // than answering `false`.
    let app = app!();

    for bad in ["ab", ".budi", "budi.", "budi..s", "budi-santoso"] {
        let response = availability(&app, bad).await;
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "`{bad}` should be refused"
        );
        assert!(json(response).await["error"]["details"]["username"].is_string());
    }
}

#[tokio::test]
async fn availability_never_mentions_an_email_or_an_account() {
    // The endpoint answers one boolean about one name. Anything else here
    // would correlate the username namespace with account state.
    let app = app!();
    let username = a_username();

    post(
        &app,
        "/auth/register",
        json!({"email": an_email(), "username": username, "password": "kata sandi panjang"}),
        a_peer(),
    )
    .await;

    let response = availability(&app, &username).await;
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let text = String::from_utf8(bytes.to_vec()).expect("utf-8");

    for leak in ["email", "@example.com", "user_id", "created_at", "reserved"] {
        assert!(!text.contains(leak), "the response leaked `{leak}`: {text}");
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

```bash
make be-test
```

Expected: every one fails with `404 Not Found`.

- [ ] **Step 3: The repository answers one boolean**

Add to `user_repo.rs`:

```rust
/// Is this canonical username already held?
///
/// `CITEXT`, so the comparison is case-insensitive in the database rather than
/// in whichever caller remembered to lowercase — the same reason `email` is.
///
/// Complexity: `O(log n)` — an index probe into `users_username_key`.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the query fails.
pub async fn username_exists(conn: &mut PgConnection, username: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM users WHERE username = $1::citext)"#,
        username
    )
    .fetch_one(conn)
    .await
    .map(|found| found.unwrap_or(false))
}
```

- [ ] **Step 4: Its own rate limit**

Add to `rate_limit.rs`:

```rust
/// Availability lookups allowed from one address per window.
///
/// Generous next to [`PER_IP`], and deliberately so: a register form checks a
/// name as somebody types it, debounced, and a person who cannot find a free
/// name legitimately tries a dozen. It is a bound on scraping the namespace,
/// not on using it.
pub const PER_IP_LOOKUP: u32 = 60;
```

and beside `allow_login`:

```rust
    /// Count an unauthenticated lookup against the calling address.
    ///
    /// Per-IP only. There is no second key to add: the thing being looked up is
    /// a public name, so counting per-name would throttle a popular name for
    /// everybody rather than throttling whoever is scraping.
    ///
    /// # Errors
    ///
    /// Returns [`redis::RedisError`] when the store is unreachable.
    pub async fn allow_lookup(&self, ip: &str) -> Result<bool, redis::RedisError> {
        Ok(self
            .allow(&format!("rl:lookup:ip:{ip}"), PER_IP_LOOKUP)
            .await?
            .allowed)
    }
```

- [ ] **Step 5: The handler**

In `adapter/http/auth.rs`, widen the two helpers Task 5 added so this handler can reuse
them rather than growing a second copy of the rules:

```rust
pub(crate) fn check_username(raw: &str) -> Result<String, ApiError> {
```

Add to `adapter/http/profile.rs`:

```rust
use axum::extract::{ConnectInfo, Path};
use std::net::SocketAddr;

/// `GET /usernames/{username}/availability`
///
/// The one endpoint here that answers without a session, because the register
/// screen needs it before there is one.
///
/// **Taken and reserved answer identically**, and that is the guard rail the
/// whole endpoint rests on. A username is a public namespace — `/@username` is
/// a real address — so "this one is taken" is not the leak that "this email has
/// an account" would be. What would be a leak is distinguishing a name somebody
/// holds from a name the platform keeps, so nothing does.
///
/// The response never accepts, returns, or correlates an email or any account
/// state. One name in, one boolean out.
///
/// # Errors
///
/// A malformed name, too many lookups from this address, or a storage failure.
pub async fn availability(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(raw): Path<String>,
) -> Result<ApiResponse<serde_json::Value>, ApiError> {
    // Fails closed, like the login limiter: an unreachable Redis means the
    // bound cannot be enforced, and an unthrottled namespace scrape is worse
    // than a brief refusal.
    let allowed = state
        .limiter
        .allow_lookup(&peer.ip().to_string())
        .await
        .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?;
    if !allowed {
        return Err(ApiError::too_many_requests());
    }

    let username = crate::adapter::http::auth::check_username(&raw)?;

    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?;
    let taken = crate::adapter::postgres::user_repo::username_exists(&mut conn, &username)
        .await
        .map_err(|err| ApiError::internal(anyhow::anyhow!(err)))?;

    // `taken || reserved`, collapsed into one boolean before it leaves the
    // process. Two fields, or two codes, would be the distinction this endpoint
    // exists to avoid making.
    let available = !taken && !anakmobil_domain::identity::username::is_reserved(&username);

    Ok(ApiResponse::ok(serde_json::json!({ "available": available })))
}
```

Add the route in `adapter/http/mod.rs`:

```rust
        .route(
            "/usernames/{username}/availability",
            get(profile::availability),
        )
```

- [ ] **Step 6: Run the tests, regenerate the cache, run the gates**

```bash
make be-prepare
make be-test
make be-fmt && make be-lint && make be-boundary && make be-sqlx-check
```

Expected: EXIT=0 on each, seven new tests passing.

- [ ] **Step 7: Confirm the endpoint really is reachable without a token**

```bash
make be-web    # in one terminal
curl -i http://localhost:8080/usernames/budi/availability
```

Expected: `HTTP/1.1 200 OK` and `{"meta":…,"data":{"available":true}}` with no
`Authorization` header sent. A `401` means the handler picked up an `Authenticated`
argument by copy-paste.

**Acceptance criteria**
- The endpoint answers with no `Authorization` header — verified by curl, not by
  reading the signature.
- A taken name and a reserved name produce identical `data`.
- All thirteen reserved names answer `false`.
- A malformed name is a 422 naming `username`; the rules come from
  `username::canonicalise` and exist nowhere else.
- The response body contains no email, account id, timestamp, or the word "reserved".
- Repeated lookups from one address are eventually refused with a 429.
- `make be-check` and `make be-sqlx-check` both EXIT=0.

---

## Task 8: the mobile dependencies, and the ones deliberately not installed

**Files:**
- Modify: `apps/mobile/package.json`
- Modify: `bun.lock` (root — never a nested one)

**Interfaces:**
- Consumes: nothing.
- Produces: `expo-secure-store`, `react-native-mmkv`, `@tanstack/react-query`,
  `@tanstack/react-query-persist-client`, `@tanstack/query-sync-storage-persister`,
  `zustand`, importable from `apps/mobile`.

**TDD: no** — a dependency install has no unit under test. Verified by
`bun install --frozen-lockfile` staying EXIT=0 and by a dev build that launches.

### Minimality check (§21)

Six packages, each argued against **this plan's own acceptance criteria**, not against
what a later plan might want.

| Package | Verdict | Why |
|---|---|---|
| `expo-secure-store` | **keep** | The spec forbids a token in MMKV, in the query cache, in a log, or in a URL. Keychain/Keystore is the only place left, and there is no stdlib equivalent. |
| `react-native-mmkv` | **keep** | AM-18 is "read from cache", so the cache has to survive a cold start. `AsyncStorage` would also do it, is async, and is not installed either — MMKV is synchronous, which is what lets the query cache restore inside the bootstrap gate rather than after it. |
| `@tanstack/react-query` | **keep** | The storage split is a settled repo decision (spec §Storage split). Rolling request caching, deduplication, and invalidation by hand is the larger diff, not the smaller one. |
| `@tanstack/react-query-persist-client` | **keep** | `persistQueryClient` is here. Hand-rolling it means getting throttling, a cache buster, and `maxAge` right, and the failure mode of getting it wrong is the previous account's garage appearing under the next account's name — the exact leak the per-account key exists to prevent. `dehydrate`/`hydrate` from core are not enough on their own. |
| `@tanstack/query-sync-storage-persister` | **keep** | The synchronous persister that pairs with MMKV. The async one would defeat the reason MMKV was chosen. |
| `zustand` | **keep** | `client.ts` reads the auth epoch from **outside React** — a hook cannot serve it, and React context cannot be read by a fetch wrapper. A module-level store with a React subscription is exactly the shape needed, and writing one by hand is `useSyncExternalStore` plumbing this would only get subtly wrong. |
| `react-hook-form` | **DROP** | Plan A renders no input of any kind. Its first consumer is Plan B's register form. |
| `zod` | **DROP** | The only runtime parsing Plan A does is narrowing one envelope shape, which is a twenty-line type guard in `errors.ts`. Its first real consumer is Plan B's form schemas. |

**Plan B installs `react-hook-form` and `zod` as its first task.** If Plan B was written
assuming Plan A had already installed them, that is a gap to close in Plan B — say so
rather than adding them here to paper over it. Both are pure-JS, so
`bun add --cwd apps/mobile react-hook-form zod` is the whole of it.

**Facts you will not discover from the plan text:**
- `bun add --filter <pkg>` **does not exist**. Use `bun add --cwd apps/mobile <pkg>` for
  pure JavaScript, and `bun x expo install <pkg>` **run from inside `apps/mobile`** for
  anything with native code, so Expo picks the version matching SDK 57 rather than the
  newest on npm.
- `expo-secure-store` and `react-native-mmkv` both ship native code. **The existing dev
  client cannot load them** — every task after this one needs a rebuilt binary, not a
  Metro reload. That rebuild is Step 4 and it takes minutes, not seconds.
- Do **not** install `@react-navigation/native`. expo-router SDK 56+ vendors its own
  navigation and throws "no longer compatible with react-navigation" if it is present.
- Never create `apps/mobile/bun.lock`. The workspace has exactly one lockfile, at the
  root, and CI's `--frozen-lockfile` is what proves it.

- [ ] **Step 1: Install the native modules through Expo**

```bash
cd /Volumes/Project/anak-mobil/apps/mobile
bun x expo install expo-secure-store react-native-mmkv
```

If either prints a config-plugin requirement, add it to `plugins` in `app.config.ts` and
say so in the commit message. Neither is expected to need one at SDK 57.

- [ ] **Step 2: Install the JavaScript-only packages**

```bash
cd /Volumes/Project/anak-mobil
bun add --cwd apps/mobile @tanstack/react-query @tanstack/react-query-persist-client @tanstack/query-sync-storage-persister zustand
```

- [ ] **Step 3: Prove the lockfile is honest**

```bash
git status --short          # expect: apps/mobile/package.json, bun.lock — nothing else
ls apps/mobile/bun.lock     # expect: No such file or directory
bun install --frozen-lockfile && echo "EXIT=$?"
```

Expected: `EXIT=0` and `git diff --stat bun.lock` unchanged by that last command. A
non-zero exit means the lockfile and `package.json` disagree — re-run the installs above
rather than editing either by hand.

- [ ] **Step 4: Rebuild the dev client and confirm the native modules load**

```bash
cd /Volumes/Project/anak-mobil
make mb-run-dev p=ios
```

Expected: the app builds and launches to the existing healthcheck screen. This is the
build that links the two native modules in; skipping it makes every later task fail at
runtime with "Cannot find native module 'ExpoSecureStore'" while type-checking cleanly.

- [ ] **Step 5: Run the gates yourself**

```bash
bun run format
make mb-check
```

Expected: EXIT=0 on both.

**Acceptance criteria**
- Exactly six packages added; `react-hook-form` and `zod` are **not** among them.
- `@react-navigation/native` is absent.
- No `apps/mobile/bun.lock` exists.
- `bun install --frozen-lockfile` EXIT=0 leaving `bun.lock` unchanged.
- A rebuilt dev client launches.

---

## Task 9: secure storage — the one record, and the pending marker

**Files:**
- Create: `apps/mobile/src/shared/session/secure.ts`

**Interfaces:**
- Consumes: `expo-secure-store` (Task 8).
- Produces:
  ```ts
  export interface StoredSession { access: string; refresh: string; refreshPending: boolean }
  export function readSession(): Promise<StoredSession | null>;
  export function writeSession(value: StoredSession): Promise<void>;
  export function markRefreshPending(): Promise<void>;
  export function clearSession(): Promise<void>;
  ```

**TDD: no — there is no test runner in `apps/mobile` and this work does not add one.**
Verified by exercising it on a simulator in Task 17: sign in, force-quit, relaunch, and
confirm the session survives; then corrupt the marker by hand and confirm the relaunch
asks for a password.

**Facts you will not discover from the plan text:**
- **One key, one JSON value, not three keys.** Three separate `setItemAsync` calls are
  three separate writes with no atomicity, and a process killed between them leaves a
  refresh token paired with a stale access token. One record cannot half-write.
- `expo-secure-store` values are capped around 2048 bytes on iOS. The two opaque tokens
  are far under that; a future addition here is not automatically safe.
- Every function is async and every one can throw — Keychain is unavailable while the
  device is locked on some configurations. A read that throws is treated as "no
  session", never as a crash, because the recovery from both is the same screen.
- `expires_in` is **not** stored. The spec is explicit that it is a hint and that a 401
  is the only authority; persisting it invites a client to decide a token is still valid.

- [ ] **Step 1: Write the module**

```ts
import * as SecureStore from "expo-secure-store";

/**
 * The session as it survives a cold start.
 *
 * One key holding one JSON record, deliberately, rather than three keys. Three
 * writes are three chances to be killed halfway and leave a refresh token
 * paired with an access token from a different pair; a single record cannot
 * half-write.
 *
 * `expires_in` is absent on purpose. The server sends a duration so a client
 * need not trust its own clock, and it may be used to refresh early — it may
 * never be used to decide a token is still valid. A 401 is the only authority,
 * and a stored expiry is an invitation to believe otherwise.
 */
export interface StoredSession {
  access: string;
  refresh: string;
  /**
   * Set immediately BEFORE a refresh request goes out, cleared once the new
   * pair is written.
   *
   * Finding it set at launch means the previous refresh's outcome is unknown:
   * the server may have rotated and the response may have been lost. Presenting
   * the old refresh token then looks exactly like a replayed stolen token, and
   * the server answers a replay by revoking every session on every device. So
   * the client discards its credentials and asks for a password instead. One
   * device asks for a login; the account does not lose the tablet.
   */
  refreshPending: boolean;
}

const KEY = "am.session";

/**
 * The stored session, or null when there is none.
 *
 * A read that throws is a session that cannot be used — the Keychain is
 * unavailable while the device is locked under some configurations — and the
 * recovery from "no session" and "unreadable session" is the same screen, so
 * they answer the same.
 */
export async function readSession(): Promise<StoredSession | null> {
  try {
    const raw = await SecureStore.getItemAsync(KEY);
    if (raw === null) return null;

    const parsed: unknown = JSON.parse(raw);
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      typeof (parsed as StoredSession).access !== "string" ||
      typeof (parsed as StoredSession).refresh !== "string"
    ) {
      return null;
    }
    const value = parsed as StoredSession;
    return {
      access: value.access,
      refresh: value.refresh,
      refreshPending: value.refreshPending === true,
    };
  } catch {
    return null;
  }
}

/** Replace the stored session. Always clears `refreshPending`. */
export async function writeSession(value: StoredSession): Promise<void> {
  await SecureStore.setItemAsync(KEY, JSON.stringify(value));
}

/**
 * Record that a refresh is about to be attempted.
 *
 * Written BEFORE the request, never after. A marker set after a response
 * arrives records nothing about the case it exists for — a response that never
 * arrived.
 */
export async function markRefreshPending(): Promise<void> {
  const current = await readSession();
  if (current === null) return;
  await SecureStore.setItemAsync(KEY, JSON.stringify({ ...current, refreshPending: true }));
}

/** Remove the session entirely. Part of the sign-out transaction. */
export async function clearSession(): Promise<void> {
  await SecureStore.deleteItemAsync(KEY);
}
```

- [ ] **Step 2: Run the gates yourself**

```bash
bun run format
make mb-check
```

Expected: EXIT=0 on both.

**Acceptance criteria**
- One `SecureStore` key holds one JSON record.
- `readSession` returns `null` rather than throwing, for a missing, unparseable, or
  wrong-shaped value.
- `writeSession` always lands `refreshPending: false`.
- No `expires_in` is persisted anywhere.
- No `any`, no `@ts-ignore`; `make mb-check` EXIT=0.

---

## Task 10: the session store

**Files:**
- Create: `apps/mobile/src/shared/session/store.ts`

**Interfaces:**
- Consumes: `zustand` (Task 8).
- Produces:
  ```ts
  export interface Me { id: string; email: string; username: string | null; displayName: string | null; hasVehicles: boolean }
  export type SessionStatus = "loading" | "signedOut" | "signedIn";
  export function useSession(): { status: SessionStatus; user: Me | null };
  export function setSignedIn(user: Me): void;
  export function setSignedOut(): void;
  export function setUser(user: Me): void;
  export function currentEpoch(): number;
  export function bumpEpoch(): number;
  export function useSessionStoreUserId(): string | null;   // added by Task 14, which needs it
  ```

**TDD: no — no runner.** Verified in Task 17 by watching the gate move between states on
a simulator.

**This file imports nothing but `zustand`, and that is structural.** `client.ts` has to
read the auth epoch from outside React, so it imports this module; if this module ever
imported anything under `api/`, that becomes a cycle and Metro resolves it to
`undefined` at runtime rather than failing the build.

**Facts you will not discover from the plan text:**
- `status` starts at `"loading"`, not `"signedOut"`. Starting signed-out flashes the
  welcome screen for one frame on every cold start of a signed-in app.
- The epoch is **not** part of the React state. It is read synchronously by
  `apiRequest`, which is not a component, and a `useState` value read from a closure is
  the stale-value bug this is designed around. Keep it as a module-level counter.
- `useSession` returns a fresh object literal each render. Select the two fields
  individually rather than returning `useStore((s) => ({...}))`, or every subscriber
  re-renders on every store write.

- [ ] **Step 1: Write the module**

```ts
import { create } from "zustand";

/** The caller's identity and derived onboarding state, as `GET /me` reports it. */
export interface Me {
  id: string;
  email: string;
  username: string | null;
  displayName: string | null;
  hasVehicles: boolean;
}

export type SessionStatus = "loading" | "signedOut" | "signedIn";

interface SessionState {
  status: SessionStatus;
  user: Me | null;
}

/**
 * Starts at "loading", never at "signedOut".
 *
 * A cold start of a signed-in app has to read the Keychain and call `/me`
 * before it knows anything. Defaulting to signed-out shows the welcome screen
 * for a frame every single launch.
 */
const useStore = create<SessionState>(() => ({ status: "loading", user: null }));

/**
 * The auth epoch, deliberately outside React state.
 *
 * `apiRequest` is not a component and cannot call a hook; it reads this
 * synchronously to decide whether a response that has just resolved still
 * belongs to the account that asked for it. A value held in React state and
 * captured by a closure is precisely the stale read this exists to prevent.
 *
 * Incremented first in the sign-out transaction. Any response landing after
 * that is dropped instead of written, which is what stops an in-flight request
 * from repopulating a cache that was just cleared — the mechanism by which the
 * next account would otherwise see the previous account's garage.
 */
let epoch = 0;

export function currentEpoch(): number {
  return epoch;
}

export function bumpEpoch(): number {
  epoch += 1;
  return epoch;
}

/**
 * The session, for components.
 *
 * Two individual selectors rather than one returning an object literal: a
 * literal is a new reference every render, so every subscriber would re-render
 * on every unrelated store write.
 */
export function useSession(): { status: SessionStatus; user: Me | null } {
  const status = useStore((state) => state.status);
  const user = useStore((state) => state.user);
  return { status, user };
}

/** Both at once, so nothing observes `signedIn` with a null user. */
export function setSignedIn(user: Me): void {
  useStore.setState({ status: "signedIn", user });
}

export function setSignedOut(): void {
  useStore.setState({ status: "signedOut", user: null });
}

/** Replace the user without touching `status` — what `refreshMe` needs. */
export function setUser(user: Me): void {
  useStore.setState({ user });
}
```

- [ ] **Step 2: Run the gates yourself**

```bash
bun run format
make mb-check
```

Expected: EXIT=0 on both.

**Acceptance criteria**
- The module imports `zustand` and nothing else.
- `status` initialises to `"loading"`.
- The epoch is a module-level counter, readable without React.
- `setSignedIn` writes status and user in one call.
- `make mb-check` EXIT=0.

---

## Task 11: the error taxonomy (AM-17 AC4)

Four kinds, four different things to say. This is the module that decides which.

**Files:**
- Create: `apps/mobile/src/shared/api/errors.ts`

**Interfaces:**
- Consumes: nothing — this module imports nothing at all.
- Produces:
  ```ts
  export type ApiErrorKind = "offline" | "validation" | "rateLimited" | "unauthorized" | "server";
  export interface ApiError { kind: ApiErrorKind; message: string; fields?: Record<string, string>; retryAfterSeconds?: number; requestId?: string }
  export interface Envelope { meta?: { request_id?: string }; data?: unknown; error?: { code: string; message: string; details?: unknown } }
  export function narrowEnvelope(body: unknown): Envelope | null;
  export function offlineError(): ApiError;
  export function serverError(requestId?: string): ApiError;
  export function toApiError(status: number, body: unknown): ApiError;
  ```

**TDD: no — no runner.** This is the pure logic that most deserves one, and the spec
says so; when a runner arrives it is the second thing to cover after the refresh state
machine. Until then it is verified in Task 17 by driving each of the five kinds against
a live API and reading what the screen says.

**Facts you will not discover from the plan text:**
- **A register collision must not become `"server"` (CG-2).** A 409 whose
  `error.details` names a field maps to `kind: "validation"` with that field populated.
  Without this rule, "email ini sudah terdaftar" reaches a person as "Ada gangguan di
  server", which breaks AM-50 AC3 outright. This is the one place where the kind is not
  a direct function of the status code.
- **Prefer the server's message where one exists.** The API's messages are already
  Bahasa Indonesia by default with `Accept-Language` support, and inventing a second
  copy on the client is how the two drift.
- **Never render `requestId` to a person.** It is carried for logs.
- `error.details` is `unknown` from the wire. Narrow it to
  `Record<string, string>` by checking each value, rather than casting — a details
  object carrying a nested object would otherwise reach a `<Text>` as `[object Object]`.
- A 5xx body may not be JSON at all (a proxy's HTML error page). `narrowEnvelope`
  returning `null` is the normal case there, not an exception.

- [ ] **Step 1: Write the module**

```ts
/**
 * What went wrong, from the point of view of the person looking at the screen.
 *
 * Four kinds because there are four genuinely different things to say — and
 * because a taxonomy with one "error" kind produces a product that says "Ada
 * gangguan di server" when somebody's password was simply wrong.
 *
 * | kind         | cause                        | what the person sees                |
 * |--------------|------------------------------|-------------------------------------|
 * | offline      | no connectivity              | "Tidak ada koneksi" + retry         |
 * | validation   | 422, or a 409 naming a field | messages under the fields that failed |
 * | rateLimited  | 429                          | the countdown from retryAfterSeconds |
 * | unauthorized | 401 after a failed refresh   | back to the welcome screen          |
 * | server       | 5xx, or a malformed response | "Ada gangguan di server" + retry     |
 */
export type ApiErrorKind = "offline" | "validation" | "rateLimited" | "unauthorized" | "server";

export interface ApiError {
  kind: ApiErrorKind;
  /** Already in Bahasa Indonesia and ready to show. Never a raw error string. */
  message: string;
  /** Field name to message, for `kind: "validation"`. */
  fields?: Record<string, string>;
  /** Seconds, for `kind: "rateLimited"`. */
  retryAfterSeconds?: number;
  /**
   * The server's request id, for a log line or a support thread.
   * NEVER rendered to a person.
   */
  requestId?: string;
}

/** The envelope every endpoint answers in. Transport, not payload. */
export interface Envelope {
  meta?: { request_id?: string };
  data?: unknown;
  error?: { code: string; message: string; details?: unknown };
}

const OFFLINE = "Tidak ada koneksi. Periksa jaringan, lalu coba lagi.";
const SERVER = "Ada gangguan di server. Coba lagi sebentar lagi.";
const RATE_LIMITED = "Terlalu banyak percobaan. Tunggu sebentar.";
const UNAUTHORIZED = "Sesi kamu sudah berakhir. Masuk lagi, ya.";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Recognise an envelope, or answer null.
 *
 * Null is an ordinary outcome, not an exception: a proxy in front of the API
 * answers a 502 with HTML, and a body that is not our envelope is exactly the
 * "malformed response" the taxonomy maps to `server`.
 */
export function narrowEnvelope(body: unknown): Envelope | null {
  if (!isRecord(body)) return null;

  const envelope: Envelope = {};

  const meta: unknown = body.meta;
  if (isRecord(meta) && typeof meta.request_id === "string") {
    envelope.meta = { request_id: meta.request_id };
  }
  if ("data" in body) envelope.data = body.data;

  const error: unknown = body.error;
  if (isRecord(error) && typeof error.code === "string" && typeof error.message === "string") {
    envelope.error = { code: error.code, message: error.message, details: error.details };
  }

  return envelope;
}

/**
 * The string values of a details object, and nothing else.
 *
 * Checked rather than cast: a details object carrying a nested value would
 * otherwise reach a `<Text>` as "[object Object]".
 */
function stringFields(details: unknown): Record<string, string> | undefined {
  if (!isRecord(details)) return undefined;

  const fields: Record<string, string> = {};
  for (const [key, value] of Object.entries(details)) {
    if (typeof value === "string") fields[key] = value;
  }
  return Object.keys(fields).length > 0 ? fields : undefined;
}

function retryAfter(details: unknown): number | undefined {
  if (!isRecord(details)) return undefined;
  const seconds = details.retry_after_seconds;
  return typeof seconds === "number" && seconds > 0 ? seconds : undefined;
}

export function offlineError(): ApiError {
  return { kind: "offline", message: OFFLINE };
}

export function serverError(requestId?: string): ApiError {
  return { kind: "server", message: SERVER, requestId };
}

/**
 * Map a non-2xx response to what a person should be told.
 *
 * The server's own message wins wherever there is one: the API answers in
 * Bahasa Indonesia by default and honours `Accept-Language`, so a second copy
 * of every message on the client is a second thing to keep in step.
 */
export function toApiError(status: number, body: unknown): ApiError {
  const envelope = narrowEnvelope(body);
  const requestId = envelope?.meta?.request_id;
  const error = envelope?.error;
  const message = error?.message ?? SERVER;
  const fields = stringFields(error?.details);

  if (status === 429) {
    return {
      kind: "rateLimited",
      message: error?.message ?? RATE_LIMITED,
      retryAfterSeconds: retryAfter(error?.details),
      requestId,
    };
  }

  if (status === 401) {
    return { kind: "unauthorized", message: error?.message ?? UNAUTHORIZED, requestId };
  }

  if (status === 422) {
    return { kind: "validation", message, fields, requestId };
  }

  // A 409 that names a field is a validation failure from the person's side.
  // "Email ini sudah terdaftar" belongs under the email input, not in a
  // server-fault banner — and without this branch a taken email reaches
  // somebody as "Ada gangguan di server".
  if (status === 409 && fields !== undefined) {
    return { kind: "validation", message, fields, requestId };
  }

  if (status >= 400 && status < 500 && error !== undefined) {
    // A 403, a 404, or a 409 with no field detail. The server's message is
    // still the right thing to show; there is simply no field to attach it to.
    return { kind: "server", message, requestId };
  }

  return serverError(requestId);
}
```

- [ ] **Step 2: Run the gates yourself**

```bash
bun run format
make mb-check
```

Expected: EXIT=0 on both.

**Acceptance criteria**
- The module imports nothing.
- A 409 carrying `details.email` maps to `kind: "validation"` with `fields.email` set —
  never to `"server"`.
- A 429 carries `retryAfterSeconds` when the server sent one, and omits it otherwise.
- A non-JSON or non-envelope body maps to `"server"` without throwing.
- `fields` contains only string values.
- No `any`; `make mb-check` EXIT=0.

---

## Task 12: the API client and the single-flight refresh

The heart of the plan. Single-flight is **mandatory, not an optimisation**: the server
answers refresh-token reuse by revoking every session, and two concurrent refreshes with
the same token are indistinguishable from a stolen one — so the naive version signs
somebody out of every device precisely when the app is busiest.

**Files:**
- Create: `apps/mobile/src/shared/api/refresh.ts`
- Create: `apps/mobile/src/shared/api/client.ts`

**Interfaces:**
- Consumes: `readSession`, `writeSession`, `markRefreshPending`, `clearSession` (Task 9);
  `currentEpoch` (Task 10); the error taxonomy (Task 11).
- Produces:
  ```ts
  // refresh.ts
  export function ensureRefreshed(): Promise<string>;   // resolves the NEW access token
  // client.ts
  export function apiRequest<T>(path: string, init?: { method?: string; body?: unknown; signal?: AbortSignal }): Promise<T>;
  ```

**TDD: no — no runner.** The spec names this state machine as the single thing most
worth testing and the first to cover when a runner arrives. Until then Task 17 verifies
it by hand: two concurrent requests against an expired access token must produce exactly
one `POST /auth/refresh` in the server log.

**Facts you will not discover from the plan text:**
- **`refresh.ts` uses a bare `fetch`, never `apiRequest`.** A 401 on `/auth/refresh`
  would otherwise call `ensureRefreshed` from inside `ensureRefreshed`.
- **`apiRequest` resolves `data`, unwrapped.** `apiRequest<Me>("/me")` gives a `Me`.
  Every call site in B, C, and D depends on it.
- **The client never sends `X-Request-Id`.** `apps/api/CLAUDE.md` is explicit that the
  server always mints its own and never reads an inbound one, because a caller-supplied
  value landing in a log line is how log injection works. Read it off the response.
- **No proactive refresh.** `expires_in` is a hint; a 401 is the only authority. A
  client that refreshes on a timer is a client racing the server's clock.
- **Exactly one retry.** A second 401 after a successful refresh means the token is not
  the problem.
- In React Native a failed `fetch` rejects with a `TypeError` ("Network request
  failed"). That rejection **is** the offline signal — no NetInfo dependency, which is
  why one is not installed.
- `EXPO_PUBLIC_API_URL` is inlined by babel at build time. An unset value is an empty
  string, not `undefined`, in some build paths — check for both.

- [ ] **Step 1: Write `refresh.ts`**

```ts
import {
  clearSession,
  markRefreshPending,
  readSession,
  writeSession,
} from "@/shared/session/secure";
import { offlineError, narrowEnvelope, serverError, type ApiError } from "@/shared/api/errors";

const BASE_URL = process.env.EXPO_PUBLIC_API_URL ?? "";

/**
 * The one refresh in flight, if there is one.
 *
 * Single-flight is a correctness requirement here, not a performance tweak. The
 * server treats a refresh token presented twice as theft and answers by
 * revoking every session the account has — correctly, for a real theft. Two
 * concurrent requests that each notice a 401 and each refresh with the same
 * stored token look identical to that from the server's side, so the naive
 * implementation signs somebody out of their phone, tablet, and laptop at
 * exactly the moment the app is making the most requests.
 */
let inFlight: Promise<string> | null = null;

interface TokenPair {
  access_token: string;
  refresh_token: string;
}

function narrowPair(data: unknown): TokenPair | null {
  if (typeof data !== "object" || data === null) return null;
  const pair = data as Partial<TokenPair>;
  return typeof pair.access_token === "string" && typeof pair.refresh_token === "string"
    ? { access_token: pair.access_token, refresh_token: pair.refresh_token }
    : null;
}

async function run(): Promise<string> {
  const stored = await readSession();
  if (stored === null) throw { kind: "unauthorized", message: "Sesi kamu sudah berakhir." } satisfies ApiError;

  // Written BEFORE the request leaves, which is the only ordering that records
  // anything about the case this exists for: a response that never arrives. If
  // the app is killed here, the next launch finds the marker, discards these
  // credentials, and asks for a password — rather than presenting a token the
  // server may already have rotated, which would look like a replay and cost
  // the account every one of its sessions.
  await markRefreshPending();

  let response: Response;
  try {
    // A bare fetch, deliberately. Routing this through `apiRequest` would mean
    // a 401 here calls `ensureRefreshed` from inside `ensureRefreshed`.
    response = await fetch(`${BASE_URL}/auth/refresh`, {
      method: "POST",
      headers: { "content-type": "application/json", "accept-language": "id" },
      body: JSON.stringify({ refresh_token: stored.refresh }),
    });
  } catch {
    // The network failed, so the server's state is unknown and the marker
    // stays set. The credentials are NOT discarded here — the next launch
    // makes that call, once, with the marker to tell it why.
    throw offlineError();
  }

  const body: unknown = await response.json().catch(() => null);
  const pair = response.ok ? narrowPair(narrowEnvelope(body)?.data) : null;

  if (pair === null) {
    // A refused refresh is final: the token is spent, replayed, or the session
    // is gone. Nothing to keep.
    await clearSession();
    throw response.ok
      ? serverError(narrowEnvelope(body)?.meta?.request_id)
      : ({ kind: "unauthorized", message: "Sesi kamu sudah berakhir. Masuk lagi, ya." } satisfies ApiError);
  }

  await writeSession({
    access: pair.access_token,
    refresh: pair.refresh_token,
    refreshPending: false,
  });
  return pair.access_token;
}

/**
 * Refresh once, however many callers ask.
 *
 * Every caller beyond the first awaits the same promise and gets the same new
 * access token.
 */
export function ensureRefreshed(): Promise<string> {
  inFlight ??= run().finally(() => {
    inFlight = null;
  });
  return inFlight;
}
```

- [ ] **Step 2: Write `client.ts`**

```ts
import { ensureRefreshed } from "@/shared/api/refresh";
import {
  narrowEnvelope,
  offlineError,
  serverError,
  toApiError,
  type ApiError,
} from "@/shared/api/errors";
import { readSession } from "@/shared/session/secure";
import { currentEpoch } from "@/shared/session/store";
import { signOut } from "@/shared/session/signOut";

const BASE_URL = process.env.EXPO_PUBLIC_API_URL ?? "";

export interface RequestInit_ {
  method?: string;
  body?: unknown;
  signal?: AbortSignal;
}

async function send(path: string, init: RequestInit_, token: string | null): Promise<Response> {
  const headers: Record<string, string> = {
    "content-type": "application/json",
    // The API answers in Bahasa Indonesia by default; sending it makes the
    // choice explicit rather than accidental.
    "accept-language": "id",
  };
  if (token !== null) headers.authorization = `Bearer ${token}`;

  // No X-Request-Id is sent, deliberately. The server always mints its own and
  // never reads an inbound one — a caller-supplied value reaching a log line is
  // how log injection works — so sending one would be ignored at best.
  return fetch(`${BASE_URL}${path}`, {
    method: init.method ?? "GET",
    headers,
    body: init.body === undefined ? undefined : JSON.stringify(init.body),
    signal: init.signal,
  });
}

/**
 * One request, and the envelope unwrapped.
 *
 * `apiRequest<Me>("/me")` resolves a `Me` — never a `{meta, data, error}`. The
 * envelope is transport; a caller wants the thing. Anything else throws an
 * `ApiError` from the taxonomy, already carrying text a person can read.
 *
 * On a 401 it refreshes ONCE, through the single-flight promise, and retries
 * the request ONCE. A second 401 after a successful refresh means the token was
 * never the problem, and a retry loop would only make it a longer failure.
 *
 * There is no proactive refresh anywhere. `expires_in` is a hint the server
 * sends so a client need not trust its own clock; a 401 is the only authority
 * on whether a token still works.
 */
export async function apiRequest<T>(path: string, init: RequestInit_ = {}): Promise<T> {
  // Captured before the request goes out. If sign-out happens while this is in
  // flight, the response is dropped rather than returned — which is what stops
  // a late response writing the previous account's data into a cache that was
  // just cleared.
  const epoch = currentEpoch();

  const stored = await readSession();
  let response: Response;
  try {
    response = await send(path, init, stored?.access ?? null);
  } catch {
    // A rejected fetch in React Native is a TypeError, and it is the only
    // connectivity signal there is. That is why no NetInfo dependency exists.
    throw offlineError();
  }

  if (response.status === 401 && stored !== null) {
    let fresh: string;
    try {
      fresh = await ensureRefreshed();
    } catch (err: unknown) {
      // The refresh is final. One sign-out, and the epoch makes it exactly one
      // however many requests failed together.
      await signOut();
      throw err;
    }
    try {
      response = await send(path, init, fresh);
    } catch {
      throw offlineError();
    }
  }

  const body: unknown = await response.json().catch(() => null);

  if (currentEpoch() !== epoch) {
    // Signed out while this was in flight. Nothing here belongs to whoever is
    // using the app now.
    throw { kind: "unauthorized", message: "Sesi sudah berganti." } satisfies ApiError;
  }

  if (!response.ok) throw toApiError(response.status, body);

  const envelope = narrowEnvelope(body);
  if (envelope === null || envelope.data === undefined) {
    // A 2xx that is not our envelope. The taxonomy calls that a server fault,
    // because from the client's side it is indistinguishable from one.
    throw serverError(envelope?.meta?.request_id);
  }

  return envelope.data as T;
}
```

The single `as T` is the one unavoidable assertion: the wire is `unknown` and the
caller declares the shape. Zod would move that assertion rather than remove it, which is
why it is not a dependency here.

- [ ] **Step 3: Run the gates yourself**

```bash
bun run format
make mb-check
```

Expected: EXIT=0. `client.ts` imports `signOut`, which Task 13 creates — dispatch these
two tasks in that order, or `mb-check` fails on an unresolved import.

**Acceptance criteria**
- `apiRequest` resolves `data` unwrapped; a 2xx that is not an envelope throws
  `kind: "server"`.
- Exactly one `POST /auth/refresh` for N concurrent 401s — verified in Task 17 against
  the server log.
- `refreshPending` is written before the refresh request leaves, and cleared only when
  the new pair is stored.
- A network failure during refresh leaves the marker set and does not clear credentials.
- No `X-Request-Id` is sent; no proactive refresh exists anywhere.
- At most one retry per request.
- `make mb-check` EXIT=0.

---

## Task 13: the query client, persisted per account

**Files:**
- Create: `apps/mobile/src/shared/api/queryClient.ts`

**Interfaces:**
- Consumes: `@tanstack/react-query`, `@tanstack/react-query-persist-client`,
  `@tanstack/query-sync-storage-persister`, `react-native-mmkv` (Task 8).
- Produces:
  ```ts
  export const queryClient: QueryClient;
  export function startPersistence(userId: string): Promise<void>;
  export function stopPersistence(): void;
  export function purgePersistedCache(userId: string): Promise<void>;
  ```

**TDD: no — no runner.** Verified in Task 17 by signing in as account A, adding cached
data, signing out, signing in as account B, and confirming nothing of A's appears.

**Facts you will not discover from the plan text:**
- **The cache key carries the account id.** Spec §Storage split: "the persisted cache is
  keyed per account, so switching accounts cannot surface the previous one's data even
  if a delete were to fail." Belt and braces, on purpose.
- **Persistence starts only once `/me` has answered**, because the account id is what
  keys it. Before that the client runs unpersisted, which is correct — there is nothing
  yet worth keeping.
- **Do not remount the tree to swap accounts.** Environment fact 10 is explicit: a
  changing `key` on a View wrapping `{children}` at the app root discards every screen's
  state. `persistQueryClient` returns an unsubscribe function; call it and start a new
  one. The `QueryClient` itself stays a module-level singleton for the app's whole life.
- MMKV v3 is synchronous and exposes `getString` / `set` / `delete`, which is not the
  `Storage` shape `createSyncStoragePersister` wants — a four-line adapter bridges them.
  It is synchronous, which is the reason MMKV was chosen over AsyncStorage: the restore
  completes inside the bootstrap gate rather than after it.
- **No token ever reaches this store.** The prohibition is absolute; the persister writes
  whatever the query cache holds, so a query that returns a token would persist it.

- [ ] **Step 1: Write the module**

```ts
import { createSyncStoragePersister } from "@tanstack/query-sync-storage-persister";
import { QueryClient } from "@tanstack/react-query";
import { persistQueryClient } from "@tanstack/react-query-persist-client";
import { MMKV } from "react-native-mmkv";

/**
 * Server state, cached. Never tokens — those live in expo-secure-store and
 * nowhere else, and this store is plain unencrypted MMKV.
 */
const storage = new MMKV({ id: "am.query" });

/** MMKV is sync and names its methods differently; the persister wants Storage. */
const mmkvStorage = {
  getItem: (key: string) => storage.getString(key) ?? null,
  setItem: (key: string, value: string) => {
    storage.set(key, value);
  },
  removeItem: (key: string) => {
    storage.delete(key);
  },
};

/**
 * One client for the app's whole life.
 *
 * A module-level singleton rather than a per-account instance, deliberately.
 * Swapping the client on sign-in would mean remounting the provider, and a
 * changing `key` on anything wrapping the route tree unmounts every screen and
 * throws away its state. The cache is emptied by `signOut` instead.
 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // AM-18 reads from cache. Five minutes fresh, a day usable while a
      // refetch happens behind it.
      staleTime: 5 * 60 * 1000,
      gcTime: 24 * 60 * 60 * 1000,
      // The client already retries a 401 exactly once, at the transport layer.
      // Retrying a 401, a 422, or a 429 here would multiply it.
      retry: 1,
    },
  },
});

/** Namespaced per account — see `startPersistence`. */
function cacheKey(userId: string): string {
  return `am.query.${userId}`;
}

let unsubscribe: (() => void) | null = null;

/**
 * Begin persisting this account's cache, and restore whatever it already has.
 *
 * **Keyed by account id**, so that switching accounts cannot surface the
 * previous one's data even if a delete were to fail. Sign-out deletes the cache
 * outright; this is the second lock on the same door, and the thing behind that
 * door is somebody's garage.
 *
 * Called only after `GET /me` has answered, because the id is the key. Until
 * then the client runs unpersisted, which is right: there is nothing worth
 * keeping before anybody is signed in.
 *
 * Awaits the restore, so the bootstrap gate can hold the first frame until the
 * cache is actually warm rather than letting a screen render empty and pop.
 */
export async function startPersistence(userId: string): Promise<void> {
  stopPersistence();

  const [unsub, restored] = persistQueryClient({
    queryClient,
    persister: createSyncStoragePersister({ storage: mmkvStorage, key: cacheKey(userId) }),
    maxAge: 24 * 60 * 60 * 1000,
    // Bump when a cached shape changes incompatibly; an old cache is then
    // discarded rather than deserialised into the wrong type.
    buster: "v1",
  });

  unsubscribe = unsub;
  await restored;
}

/** Stop writing to disk. Does not delete what is already there. */
export function stopPersistence(): void {
  unsubscribe?.();
  unsubscribe = null;
}

/** Remove this account's persisted cache. Part of the sign-out transaction. */
export async function purgePersistedCache(userId: string): Promise<void> {
  stopPersistence();
  // MMKV is synchronous; the Promise is for the caller's benefit, so that
  // sign-out can await this step in order with the async ones around it.
  storage.delete(cacheKey(userId));
  await Promise.resolve();
}
```

- [ ] **Step 2: Run the gates yourself**

```bash
bun run format
make mb-check
```

Expected: EXIT=0 on both.

**Acceptance criteria**
- The `QueryClient` is a module-level singleton; nothing recreates it per account.
- The persist key contains the account id.
- `startPersistence` awaits the restore.
- No token can reach this store — asserted by inspection of every query added in this
  plan.
- `make mb-check` EXIT=0.

---

## Task 14: `signIn`, `refreshMe`, and the sign-out epoch transaction

**Files:**
- Create: `apps/mobile/src/shared/api/me.ts`
- Create: `apps/mobile/src/shared/session/signIn.ts`
- Create: `apps/mobile/src/shared/session/signOut.ts`

**Interfaces:**
- Consumes: `apiRequest` (Task 12); secure storage (Task 9); the session store (Task 10);
  the query client (Task 13).
- Produces:
  ```ts
  // me.ts
  export function fetchMe(): Promise<Me>;
  export function refreshMe(): Promise<void>;
  // signIn.ts
  export function signIn(tokens: { access_token: string; refresh_token: string; expires_in: number }): Promise<void>;
  // signOut.ts
  export function signOut(): Promise<void>;
  ```

**TDD: no — no runner.** Verified in Task 17: sign in on a simulator, sign out, and
confirm one redirect, an empty cache, and an empty Keychain entry.

**Facts you will not discover from the plan text:**
- **`signOut.ts` must not import `client.ts`.** `client.ts` imports `signOut`; the
  reverse would be a cycle Metro resolves to `undefined` at runtime rather than failing
  the build.
- **The epoch is incremented first, before anything else in the transaction.** Every
  other step is a cleanup; the epoch is what makes a response that lands mid-cleanup get
  dropped instead of written. Reordering it to last is the bug this design exists to
  prevent, and it is invisible until two accounts are used on one device.
- **`signIn` flips `status` last.** A group layout that sees `signedIn` with a null user
  renders against `user!` or crashes.
- **`refreshMe` leaves `status` alone.** It replaces the user, nothing more.
- `expires_in` is accepted by `signIn` and deliberately **not stored** — it is in the
  signature because that is the shape the server sends, and discarding it explicitly is
  clearer than making every caller destructure around it.

- [ ] **Step 1: Write `me.ts`**

```ts
import { apiRequest } from "@/shared/api/client";
import { setUser, type Me } from "@/shared/session/store";

/** The wire shape. snake_case, exactly as the API sends it. */
interface MeWire {
  id: string;
  email: string;
  username: string | null;
  display_name: string | null;
  has_vehicles: boolean;
}

/** Read the caller's identity and derived onboarding state. */
export async function fetchMe(): Promise<Me> {
  const wire = await apiRequest<MeWire>("/me");
  return {
    id: wire.id,
    email: wire.email,
    username: wire.username,
    displayName: wire.display_name,
    hasVehicles: wire.has_vehicles,
  };
}

/**
 * Re-read `/me` and update the session store. `status` is untouched.
 *
 * Two callers, and the ORDER matters for both:
 *
 * - **Plan C**, when the app shell loads an empty vehicle list. `(app)` is only
 *   reachable with `hasVehicles === true`, so an empty list means the last car
 *   went away somewhere else and the cached `me` is stale. This is the precise
 *   recovery; invalidating every query and bouncing through `/` is not.
 *
 * - **Plan D**, immediately after `POST /vehicles`. At that moment the cached
 *   `me.hasVehicles` is still `false`, so navigating before this resolves sends
 *   somebody straight back into the wizard they just finished. And the PREVIOUS
 *   value of `hasVehicles` is what decides aha-screen versus garage — so read
 *   it BEFORE calling this, then refresh, then navigate.
 */
export async function refreshMe(): Promise<void> {
  setUser(await fetchMe());
}
```

- [ ] **Step 2: Write `signIn.ts`**

```ts
import { fetchMe } from "@/shared/api/me";
import { writeSession } from "@/shared/session/secure";
import { setSignedIn } from "@/shared/session/store";

/**
 * The only way a session starts.
 *
 * Register and login both receive a token pair and hand it here. No screen
 * writes to secure storage itself — centralising that write is most of what
 * this module exists for, and a second copy of it in a form's `onSuccess` is
 * how a token ends up somewhere it should not be.
 *
 * The order is fixed and is an acceptance criterion:
 *
 *   1. write the pair (clearing any `refresh_pending` marker, which
 *      `writeSession` does unconditionally)
 *   2. GET /me
 *   3. populate the store and flip `status` to `signedIn`, together
 *
 * Flipping status before the user is loaded would let a group layout render
 * against `user === null`. Fetching before writing would send the request with
 * no token.
 *
 * `expires_in` is accepted and deliberately discarded: it is a hint, a 401 is
 * the only authority, and a stored expiry is an invitation to believe
 * otherwise. It is in the signature because it is in the server's response, and
 * discarding it here is clearer than making every caller destructure around it.
 */
export async function signIn(tokens: {
  access_token: string;
  refresh_token: string;
  expires_in: number;
}): Promise<void> {
  await writeSession({
    access: tokens.access_token,
    refresh: tokens.refresh_token,
    refreshPending: false,
  });

  const user = await fetchMe();
  setSignedIn(user);
}
```

- [ ] **Step 3: Write `signOut.ts`**

```ts
import { purgePersistedCache, queryClient } from "@/shared/api/queryClient";
import { clearSession, readSession } from "@/shared/session/secure";
import { bumpEpoch, setSignedOut, useSessionStoreUserId } from "@/shared/session/store";

let inFlight: Promise<void> | null = null;

async function run(): Promise<void> {
  // FIRST. Everything below is cleanup; this is the part that makes a response
  // landing mid-cleanup get dropped instead of written. Without it, a request
  // that was already in flight writes fresh data into a cache that was just
  // cleared — which is how the next account sees the previous account's garage.
  //
  // It also gives "exactly one redirect" for free: ten requests failing to
  // refresh all call signOut, the first bumps the epoch, and the rest are the
  // same awaited promise.
  bumpEpoch();

  const userId = useSessionStoreUserId();

  // Tell the API the session is over. Best-effort by design: the local session
  // ends whether or not this succeeds, because a person on a plane pressing
  // sign-out must still be signed out. A bare fetch rather than apiRequest —
  // importing the client here would close the cycle client.ts -> signOut.ts.
  const stored = await readSession();
  if (stored !== null) {
    try {
      await fetch(`${process.env.EXPO_PUBLIC_API_URL ?? ""}/auth/logout`, {
        method: "POST",
        headers: { authorization: `Bearer ${stored.access}` },
      });
    } catch {
      // Deliberately swallowed. There is nothing to tell the person and
      // nothing to retry: the server's session expires on its own.
    }
  }

  // In-flight requests, cancelled. Anything Query is not managing is covered by
  // the epoch above.
  await queryClient.cancelQueries();
  queryClient.clear();

  // Awaited, in order, so the delete completes before the next account's
  // persistence starts. The per-account key means even a failure here could not
  // surface this account's data under another — belt and braces.
  if (userId !== null) await purgePersistedCache(userId);

  await clearSession();
  setSignedOut();
}

/**
 * End the session: one transaction, one redirect.
 *
 * Single-flight, so ten simultaneous failures are one sign-out. The gates in
 * `gates.tsx` do the redirecting; nothing here navigates, because a navigation
 * call in a cleanup function is the second redirect the spec bans.
 */
export function signOut(): Promise<void> {
  inFlight ??= run().finally(() => {
    inFlight = null;
  });
  return inFlight;
}
```

Add the one accessor `signOut` needs to `session/store.ts` (Task 10), beside the others:

```ts
/** The signed-in account's id, read outside React — the persisted cache is keyed on it. */
export function useSessionStoreUserId(): string | null {
  return useStore.getState().user?.id ?? null;
}
```

The name is deliberately awkward: it is not a hook, and calling it `userId()` invites
somebody to reach for it inside a component where `useSession()` is correct.

- [ ] **Step 4: Run the gates yourself**

```bash
bun run format
make mb-check
```

Expected: EXIT=0 on both.

**Acceptance criteria**
- `signOut.ts` does not import `client.ts` — grep it.
- `bumpEpoch()` is the first statement of the transaction.
- `signIn` flips `status` only after `/me` resolves.
- `refreshMe` does not touch `status`.
- Nothing in any of the three files calls `router.replace` or `router.push`.
- A failed `POST /auth/logout` still ends the local session.
- `make mb-check` EXIT=0.

---

## Task 15: the active vehicle (AM-18)

**Files:**
- Create: `apps/mobile/src/shared/vehicle/activeVehicle.ts`

**Interfaces:**
- Consumes: `zustand`, `react-native-mmkv` (Task 8).
- Produces:
  ```ts
  export function useActiveVehicleId(): string | null;
  export function setActiveVehicleId(id: string | null): void;
  export function clearActiveVehicle(): void;
  ```

**TDD: no — no runner.** Verified in Task 17: set a value, force-quit, relaunch, and
confirm it survives; sign out and confirm it does not.

**Facts you will not discover from the plan text:**
- **Its own MMKV instance, not the query store.** This is client state — which car the
  person is looking at — and it must not be swept away when the server-state cache is
  cleared or busted. The spec's storage split puts them in different places for exactly
  this reason.
- **Read synchronously at module load**, not in an effect. MMKV is sync; an effect would
  render one frame with no active vehicle and reflow the whole garage screen.
- **Cleared by sign-out.** A vehicle id is not sensitive on its own, but it is the
  previous account's, and the next account has no car with that id — every query keyed
  on it would 404. Task 14's `signOut` calls `clearActiveVehicle()`; add that call when
  this task lands.

- [ ] **Step 1: Write the module**

```ts
import { MMKV } from "react-native-mmkv";
import { create } from "zustand";

/**
 * Client state, in its own store.
 *
 * Not in the query cache: that holds server state and is cleared, busted, and
 * keyed per account. Which car somebody is looking at is a preference, and it
 * should survive a cache bust the way a scroll position would.
 */
const storage = new MMKV({ id: "am.client" });
const KEY = "activeVehicleId";

interface ActiveVehicleState {
  id: string | null;
}

/**
 * Read synchronously at module load rather than in an effect.
 *
 * MMKV is synchronous, so there is no reason to render one frame with no
 * active vehicle and then reflow the garage when it arrives.
 */
const useStore = create<ActiveVehicleState>(() => ({
  id: storage.getString(KEY) ?? null,
}));

/** The car currently in focus, or null when none has been chosen. */
export function useActiveVehicleId(): string | null {
  return useStore((state) => state.id);
}

/** Choose the active car, or clear it with null. Persists immediately. */
export function setActiveVehicleId(id: string | null): void {
  if (id === null) {
    storage.delete(KEY);
  } else {
    storage.set(KEY, id);
  }
  useStore.setState({ id });
}

/**
 * Forget it entirely. Called by the sign-out transaction.
 *
 * Not because a vehicle id is a secret, but because it belongs to the previous
 * account: the next person has no car with that id, so every query keyed on it
 * would answer 404 and the garage would open on an error.
 */
export function clearActiveVehicle(): void {
  setActiveVehicleId(null);
}
```

- [ ] **Step 2: Wire it into sign-out**

In `apps/mobile/src/shared/session/signOut.ts`, add the import and the call, immediately
before `clearSession()`:

```ts
import { clearActiveVehicle } from "@/shared/vehicle/activeVehicle";
```

```ts
  clearActiveVehicle();
  await clearSession();
  setSignedOut();
```

- [ ] **Step 3: Run the gates yourself**

```bash
bun run format
make mb-check
```

Expected: EXIT=0 on both.

**Acceptance criteria**
- Its own MMKV instance, separate from the query cache.
- The initial value is read synchronously at module load.
- `signOut` clears it.
- `make mb-check` EXIT=0.

---

## Task 16: the gates, the route groups, and the bootstrap

The route groups are what make the gate declarative: a group's layout decides whether
its subtree may render at all, so no screen needs a guard of its own.

**Files:**
- Create: `apps/mobile/src/shared/gates.tsx`
- Create: `apps/mobile/src/shared/bootstrap.ts`
- Create: `apps/mobile/src/shared/index.ts` — the barrel, because this is where its first consumer appears
- Create: `apps/mobile/src/app/(auth)/_layout.tsx`, `(auth)/index.tsx`
- Create: `apps/mobile/src/app/(onboarding)/_layout.tsx`, `(onboarding)/index.tsx`
- Create: `apps/mobile/src/app/(app)/_layout.tsx`, `(app)/index.tsx`
- Modify: `apps/mobile/src/app/_layout.tsx` (providers + the bootstrap)
- Modify: `apps/mobile/src/app/index.tsx` (becomes the router; its healthcheck body moves to `(app)/index.tsx`)

**Interfaces:**
- Consumes: everything from Tasks 9–15.
- Produces: `AuthGate`, `OnboardingGate`, `AppGate`, and the three route groups.

**TDD: no — no runner.** Verified by §27's visual pass in Task 17, across every state.

**Plans C and D replace layout *bodies*, never gates.** `(app)/_layout.tsx` ships as
`<AppGate><Stack …/></AppGate>`; Plan C replaces only the `<Stack …/>` with its Tabs
navigator. `(onboarding)` is the same shape for Plan D. A rewrite that silently drops an
inlined gate is the one security-shaped defect those plans could cause, and making the
gate a component removes the hazard rather than warning about it.

**Facts you will not discover from the plan text:**
- The router uses `src/app`, not a root `app/`. `src/app/index.tsx` and
  `src/app/catalog.tsx` exist today.
- `experiments.typedRoutes` is on, so a `<Redirect href="/(auth)">` to a group with no
  screen in it is a **type error**, not a runtime surprise. Each group needs its
  `index.tsx` for the build to pass.
- `catalog.tsx` stays ungrouped and therefore unguarded, deliberately — it is the AM-15
  component catalogue and a developer surface, reachable regardless of session.
- The existing `_layout.tsx` already overrides the navigation container background to
  transparent so `AmGround` shows through. Do not undo it, and do not put a changing
  `key` on anything wrapping `{children}`.
- The splash screen is currently hidden as soon as fonts are ready. It must now stay up
  until the session resolves too, or the welcome screen flashes before the garage.
- The three placeholder `index.tsx` screens are exactly that. Mark each with a
  `ponytail:` comment naming the plan that replaces it; without them there is nothing to
  redirect to and the gate cannot be demonstrated.

- [ ] **Step 1: Write `gates.tsx`**

```tsx
import { Redirect } from "expo-router";
import type { ReactNode } from "react";

import { useSession, type Me } from "@/shared/session/store";

/**
 * Onboarding completion is DERIVED, never stored.
 *
 * A person who has a car has finished onboarding. A stored completion flag
 * would be a second source of truth free to disagree with the first, and the
 * disagreement would surface as somebody stuck outside their own garage.
 *
 * `username` is set at registration, so a null one only happens for an account
 * created before the username migration — none exist, and treating it as
 * "profile incomplete" is the honest answer if one ever does.
 */
function needsProfile(user: Me): boolean {
  return user.username === null || user.displayName === null;
}

function needsFirstVehicle(user: Me): boolean {
  return !user.hasVehicles;
}

/**
 * The signed-out subtree.
 *
 * Redirects OUT the moment a session starts. This is why no screen calls
 * `router.replace()` in its `onSuccess`: with both a login screen and a
 * register screen able to sign in, that would be two redirects for one event,
 * and the spec bans the second one.
 */
export function AuthGate({ children }: { readonly children: ReactNode }): ReactNode {
  const { status, user } = useSession();

  if (status === "loading") return null;
  if (status === "signedIn" && user !== null) {
    return needsProfile(user) || needsFirstVehicle(user) ? (
      <Redirect href="/(onboarding)" />
    ) : (
      <Redirect href="/(app)" />
    );
  }
  return children;
}

/**
 * The onboarding subtree.
 *
 * Plan D replaces the layout body with its wizard stack; this gate is not part
 * of that body and is not its to edit. Which STEP renders is D's decision —
 * this only decides whether the group renders at all.
 */
export function OnboardingGate({ children }: { readonly children: ReactNode }): ReactNode {
  const { status, user } = useSession();

  if (status === "loading") return null;
  if (status === "signedOut" || user === null) return <Redirect href="/(auth)" />;
  if (!needsProfile(user) && !needsFirstVehicle(user)) return <Redirect href="/(app)" />;
  return children;
}

/**
 * The signed-in app.
 *
 * Plan C replaces the layout body with a Tabs navigator, inside this gate. A
 * deep link into a protected route lands here first and is held or redirected
 * before any screen mounts — which is what makes AM-55 AC2's "no skip" real
 * rather than a missing button.
 */
export function AppGate({ children }: { readonly children: ReactNode }): ReactNode {
  const { status, user } = useSession();

  if (status === "loading") return null;
  if (status === "signedOut" || user === null) return <Redirect href="/(auth)" />;
  if (needsProfile(user) || needsFirstVehicle(user)) return <Redirect href="/(onboarding)" />;
  return children;
}
```

- [ ] **Step 1b: Write the barrel, because this is where its first consumer appears**

The route files below import from `@/shared`, so it has to exist now rather than in
Task 17. Create `apps/mobile/src/shared/index.ts`:

```ts
/**
 * The public surface of the mobile session foundation.
 *
 * Plans B, C, and D import from `@/shared` and from nowhere else — never
 * `@/shared/session/store`, never `@/shared/api/client`. The file layout behind
 * this barrel is Plan A's business and may move without touching them.
 *
 * Everything exported here is in Plan A's FROZEN CONTRACT. Adding to it is a
 * decision; changing a signature in it is a structural finding.
 */
export { apiRequest } from "./api/client";
export type { ApiError, ApiErrorKind } from "./api/errors";
export { fetchMe, refreshMe } from "./api/me";
export { queryClient } from "./api/queryClient";
export { useBootstrap } from "./bootstrap";
export { AppGate, AuthGate, OnboardingGate } from "./gates";
export { signIn } from "./session/signIn";
export { signOut } from "./session/signOut";
export { useSession } from "./session/store";
export type { Me, SessionStatus } from "./session/store";
export { setActiveVehicleId, useActiveVehicleId } from "./vehicle/activeVehicle";
```

`useBootstrap` and `queryClient` are not in the FROZEN CONTRACT — B, C, and D have no
reason to call either. They are exported so that `_layout.tsx` needs no deep import,
which is what keeps the "one import path" rule literally true for every file under
`src/app`.

Task 17 verifies the barrel against the contract symbol by symbol; this step only has to
make it exist and compile.

- [ ] **Step 2: Write the three group layouts**

`src/app/(auth)/_layout.tsx`:

```tsx
import { Stack } from "expo-router";

import { AuthGate } from "@/shared";

// Plan B replaces the <Stack> body with welcome / login / register.
// It does not touch AuthGate.
export default function AuthLayout() {
  return (
    <AuthGate>
      <Stack screenOptions={{ headerShown: false, contentStyle: { backgroundColor: "transparent" } }} />
    </AuthGate>
  );
}
```

`src/app/(onboarding)/_layout.tsx`:

```tsx
import { Stack } from "expo-router";

import { OnboardingGate } from "@/shared";

// Plan D replaces the <Stack> body with the profile step, the six-step wizard,
// and the aha screen. It does not touch OnboardingGate.
export default function OnboardingLayout() {
  return (
    <OnboardingGate>
      <Stack screenOptions={{ headerShown: false, contentStyle: { backgroundColor: "transparent" } }} />
    </OnboardingGate>
  );
}
```

`src/app/(app)/_layout.tsx`:

```tsx
import { Stack } from "expo-router";

import { AppGate } from "@/shared";

// Plan C replaces the <Stack> body with the five-tab navigator. It does not
// touch AppGate — the gate is the authorization boundary for this whole
// subtree, and an overwrite that dropped it would be invisible.
export default function AppLayout() {
  return (
    <AppGate>
      <Stack screenOptions={{ headerShown: false, contentStyle: { backgroundColor: "transparent" } }} />
    </AppGate>
  );
}
```

- [ ] **Step 3: Write the three placeholder screens**

`src/app/(auth)/index.tsx`:

```tsx
import { StyleSheet, Text, View } from "react-native";

import { useTheme } from "@/theme";

// ponytail: a placeholder so the gate has somewhere to redirect to and can be
// demonstrated. Plan B replaces this group with welcome / login / register.
export default function AuthPlaceholder() {
  const theme = useTheme();
  return (
    <View style={[styles.container, { padding: theme.space[6], gap: theme.space[3] }]}>
      <Text style={[theme.type.h3, { color: theme.color.textPrimary }]}>Belum masuk</Text>
      <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
        Layar masuk dan daftar menyusul.
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, justifyContent: "center" },
});
```

`src/app/(onboarding)/index.tsx`:

```tsx
import { StyleSheet, Text, View } from "react-native";

import { useTheme } from "@/theme";

// ponytail: a placeholder so the gate has somewhere to redirect to and can be
// demonstrated. Plan D replaces this group with the profile step, the six-step
// wizard, and the aha screen.
export default function OnboardingPlaceholder() {
  const theme = useTheme();
  return (
    <View style={[styles.container, { padding: theme.space[6], gap: theme.space[3] }]}>
      <Text style={[theme.type.h3, { color: theme.color.textPrimary }]}>Lengkapi profil</Text>
      <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
        Langkah profil dan mobil pertama menyusul.
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, justifyContent: "center" },
});
```

`(app)/index.tsx` is **not** a placeholder: move the whole body of the existing
`src/app/index.tsx` here verbatim — the AM-14 connectivity screen is a real diagnostic —
and add one control that exercises the sign-out transaction:

```tsx
import { AmButton } from "@/components/input";
import { signOut, useSession } from "@/shared";
```

```tsx
      <Text style={[styles.row, { color: theme.color.textPrimary }]}>
        Masuk sebagai: {user?.displayName ?? user?.username ?? "—"}
      </Text>
      <AmButton label="Keluar" variant="secondary" onPress={() => void signOut()} />
```

with `const { user } = useSession();` beside the existing `useTheme()`. Plan C replaces
this screen with the real home.

- [ ] **Step 4: The root router**

Replace `src/app/index.tsx` entirely — its old body now lives in `(app)/index.tsx`:

```tsx
import { Redirect } from "expo-router";

import { useSession } from "@/shared";

/**
 * The one entry point, and the only place that chooses a group.
 *
 * Each group's own gate then re-checks, so a deep link straight into a
 * protected route is held whether or not it came through here.
 */
export default function Index() {
  const { status, user } = useSession();

  // The splash screen is still up — the root layout holds it until the session
  // resolves — so there is nothing to render and nothing to flash.
  if (status === "loading") return null;
  if (status === "signedOut" || user === null) return <Redirect href="/(auth)" />;
  if (user.username === null || user.displayName === null || !user.hasVehicles) {
    return <Redirect href="/(onboarding)" />;
  }
  return <Redirect href="/(app)" />;
}
```

- [ ] **Step 5: The bootstrap, as a hook in `@/shared`**

The restore sequence is session-foundation logic, not layout logic, and Plans C and D
will both edit `_layout.tsx`. Putting it behind `@/shared` for the same reason the gates
are components: it keeps the ordering somewhere their edits do not reach, and it keeps
`_layout.tsx` free of deep imports.

Create `apps/mobile/src/shared/bootstrap.ts`:

```ts
import { useEffect, useState } from "react";

import { fetchMe } from "@/shared/api/me";
import { startPersistence } from "@/shared/api/queryClient";
import { clearSession, readSession } from "@/shared/session/secure";
import { setSignedIn, setSignedOut } from "@/shared/session/store";

/**
 * Restore the session, once, before anything renders.
 *
 * The order is the whole point:
 *
 *   1. read secure storage
 *   2. a set `refreshPending` marker means the previous refresh's outcome is
 *      UNKNOWN — the server may already have rotated. Presenting that refresh
 *      token would look like a replay, and the server answers a replay by
 *      revoking every session on every device. So the credentials are discarded
 *      and this one device asks for a password.
 *   3. GET /me, which also proves the access token still works
 *   4. start the per-account cache and AWAIT the restore, so the first frame is
 *      warm rather than empty-then-popping
 *   5. only then flip status
 */
export function useBootstrap(): boolean {
  const [done, setDone] = useState(false);

  useEffect(() => {
    let alive = true;

    void (async () => {
      const stored = await readSession();

      if (stored === null || stored.refreshPending) {
        if (stored?.refreshPending === true) await clearSession();
        if (alive) setSignedOut();
      } else {
        try {
          const user = await fetchMe();
          await startPersistence(user.id);
          if (alive) setSignedIn(user);
        } catch {
          // Any failure here — an expired session the refresh could not save,
          // or an unreachable API — lands on the welcome screen. The tokens
          // stay put when the cause was the network: the client has already
          // discarded them if the server actually refused.
          if (alive) setSignedOut();
        }
      }

      if (alive) setDone(true);
    })();

    return () => {
      alive = false;
    };
  }, []);

  return done;
}
```

Then modify `src/app/_layout.tsx`. Keep `ThemeProvider`, `CapabilityControlContext`,
`AmGround`, `ToastProvider`, and `TransparentNavigationTheme` exactly as they are — and
keep the transparent-background override. Its only new imports are from `@/shared` and
from TanStack:

```tsx
import { QueryClientProvider } from "@tanstack/react-query";

import { queryClient, useBootstrap } from "@/shared";
```

Hold the splash until both fonts and the session are ready:

```tsx
  const fontsReady = useAppFonts();
  const sessionReady = useBootstrap();
  const ready = fontsReady && sessionReady;

  useEffect(() => {
    if (ready) SplashScreen.hideAsync().catch(() => {});
  }, [ready]);

  if (!ready) return null;
```

Wrap the existing tree in `<QueryClientProvider client={queryClient}>` immediately
inside `ThemeProvider`. **Do not add a `key` to anything wrapping the `<Stack>`** — a
changing key there unmounts every screen and discards its state.

- [ ] **Step 6: Run the gates yourself**

```bash
bun run format
make mb-check
```

Expected: EXIT=0 on both, with the typed-route generation resolving `/(auth)`,
`/(onboarding)`, and `/(app)`.

**Acceptance criteria**
- Three group layouts, each `<XGate><Stack …/></XGate>` and nothing more.
- No screen anywhere calls `router.replace` or `router.push` for an auth decision.
- The splash screen stays up until fonts **and** session are ready.
- `catalog.tsx` is still reachable and still ungrouped.
- The transparent navigation-theme override survives untouched.
- No changing `key` on anything wrapping the route tree.
- `make mb-check` EXIT=0.

---

## Task 17: contract verification and the visual pass

**Files:**
- Modify: `apps/mobile/src/shared/index.ts` (created in Task 16 — completed and verified here)

**Interfaces:**
- Consumes: every mobile module.
- Produces: exactly the FROZEN CONTRACT, from one import path, proven rather than
  assumed.

**TDD: no — no runner.** This task **is** the verification, and §27 makes it mandatory:
the deliverable renders in a simulator, so screenshots of every meaningful state are
proof, not a nicety.

- [ ] **Step 1: Prove the contract is complete and the import path is single**

```bash
cd /Volumes/Project/anak-mobil
grep -rn "@/shared/" apps/mobile/src/app && echo "DEEP IMPORT FOUND — fix it"
```

Expected: no matches inside `src/app` other than none at all — every route file imports
from `@/shared`. Deep imports inside `src/shared` itself are correct and expected.

Then check each contract symbol resolves, by adding a scratch file, type-checking, and
deleting it:

```bash
cat > apps/mobile/src/shared/.contract-check.ts <<'EOF'
import {
  apiRequest, signIn, signOut, refreshMe, useSession,
  useActiveVehicleId, setActiveVehicleId, AuthGate, OnboardingGate, AppGate,
  type Me, type ApiError, type ApiErrorKind, type SessionStatus,
} from "@/shared";
export const used = [apiRequest, signIn, signOut, refreshMe, useSession,
  useActiveVehicleId, setActiveVehicleId, AuthGate, OnboardingGate, AppGate];
export type Check = [Me, ApiError, ApiErrorKind, SessionStatus];
EOF
make mb-check
rm apps/mobile/src/shared/.contract-check.ts
```

Expected: EXIT=0 with the scratch file present. A missing export fails here rather than
in Plan B.

- [ ] **Step 2: Run the whole repository's gates**

```bash
bun run format
make check
make be-sqlx-check
```

Expected: EXIT=0 on each. `make check` runs `be-check`, `fe-check`, `mb-check`, and
`fmt-check`; `be-sqlx-check` is separate and is the one CI runs.

- [ ] **Step 3: The visual pass (§27 — mandatory, not optional)**

Start the API and the app:

```bash
make db-up-all
make be-web            # one terminal
make mb-run-dev p=ios  # another
```

Then walk every state and **capture a screenshot of each**, light and dark:

| # | State | How to reach it | What must be true |
|---|---|---|---|
| 1 | Cold start, no session | Fresh install | Splash holds, then `(auth)` — no flash of the app |
| 2 | Cold start, signed in | Register via curl, `signIn` via the placeholder, force-quit, relaunch | Splash holds, then `(app)` — no flash of `(auth)` |
| 3 | Onboarding incomplete | Register a new account, relaunch | Lands in `(onboarding)`, not `(app)` |
| 4 | Deep link into `(app)` while signed out | `npx uri-scheme open "anakmobil://(app)" --ios` | Redirected to `(auth)` |
| 5 | Sign-out | Press "Keluar" | One redirect to `(auth)`; relaunch stays signed out |
| 6 | Offline | Turn off the API mid-session, pull to refresh | "Tidak ada koneksi", never a raw error |
| 7 | Rate limited | 11 bad logins via curl, then one more | 429 body carries `retry_after_seconds` |
| 8 | Expired access token | `redis-cli DEL "at:<digest>"`, then use the app | One `POST /auth/refresh` in the log, request succeeds |

- [ ] **Step 4: The two properties that need a log, not a screenshot**

**Single-flight.** With an expired access token, trigger several requests at once and
count refreshes in the API log:

```bash
make be-web 2>&1 | grep -c "POST /auth/refresh"
```

Expected: exactly `1` for a burst of N requests. More than one means the promise is not
shared and the account is one race away from losing every device.

**Per-account cache isolation.** Sign in as account A, let the garage cache, sign out,
sign in as account B, and confirm nothing of A's appears — including for the frame
before B's data arrives.

- [ ] **Step 5: Share the screenshots and the two log results**

They are the evidence this plan is done. A green `make check` is not evidence a screen
is right.

**Acceptance criteria**
- Every FROZEN CONTRACT symbol is exported from `@/shared` with exactly that signature.
- No file under `src/app` imports a path deeper than `@/shared`.
- `make check` EXIT=0.
- Screenshots exist for all eight states, light and dark.
- Exactly one `POST /auth/refresh` for a burst of concurrent 401s.
- Account B never sees account A's cached data.

---

## Execution mode

### 1. What runs in parallel, and what is serialised on what

**The backend half — Tasks 1 through 7 — is serial, and that verdict comes from
analysis rather than from not looking.** Three shared resources force it, and each one
would produce a silent failure rather than a loud one:

- **The committed `.sqlx` cache.** Tasks 2, 5, 6, and 7 all change SQL, and each must run
  `make be-prepare`, which regenerates the whole `.sqlx/` directory. Two writers doing
  that concurrently overwrite each other's entries and the loser fails only later, in
  CI, on `make be-sqlx-check`.
- **The shared development database.** `apps/api/CLAUDE.md` records the incident
  directly: with several agents against one database, a migration amended and reset by
  one of them leaves every other process holding the old checksum, and `sqlx::migrate!`
  then fails for **every test file in the workspace** — silently, because the `app!`
  macro swallows the message and cargo captures stderr for passing tests.
- **`adapter/http/auth.rs`.** Tasks 1, 4, and 5 all edit it.

Task 3 (the domain canonicaliser) is the one genuine exception — a new file in a
different crate, no SQL, no shared file. It could run alongside Task 1. It buys almost
nothing, because verification is what takes the wall-clock time and both writers would
serialise on cargo's target-directory lock anyway. **Run it concurrently with Task 1 if
the controller wants the slot filled; otherwise take the serial chain and lose nothing.**

**The mobile half has real parallelism, and it is worth taking.**

```
T8 (deps) ─┬─ T9  secure.ts      ─┐
           ├─ T10 store.ts       ─┼─ T12 refresh + client ─┐
           ├─ T11 errors.ts      ─┘                        ├─ T14 signIn/refreshMe/signOut ─ T15 activeVehicle ─┐
           └─ T13 queryClient.ts ──────────────────────────┘                                                     ├─ T16 gates + routes ─ T17 verify
```

- **T8 has no backend dependency at all** — it edits `package.json` and `bun.lock`. Start
  it concurrently with backend Task 1.
- **{T9, T10, T11, T13} run concurrently** once T8 lands. Four disjoint new files, none
  importing another, each compiling alone — so `make mb-check` stays meaningful after
  every individual landing rather than only after the group.
- **T12 waits on T9 + T10 + T11.** It also imports `signOut` from T14, so dispatch T12
  before T14 and expect `mb-check` to fail on that one unresolved import until T14
  lands — or dispatch them as one pair. The plan says so in T12's Step 3.
- **T15 modifies `signOut.ts`**, which T14 creates. Serial on that file.
- **T16 needs a running backend**, not just a compiled contract: its bootstrap calls
  `GET /me` and its sign-out button hits the new logout. Hold it until Tasks 1, 5, and 6
  are merged and `make be-web` answers.

### 2. What the writers cannot discover for themselves

Every task carries a **Facts you will not discover from the plan text** block. The four
that would cost the most if missed:

- **citext defeats a regex CHECK** (Task 2) and **sqlx cannot map citext to String**
  (Task 6). Both are silent: the first accepts `BUDI` into a column that must not hold
  it, the second fails at macro-expansion with a confusing message about an unknown type.
- **`tests/session_store.rs` has five assertions that change shape** when `authenticate`
  returns a struct (Task 1). A writer who does not know will fix them by casting instead
  of reading the new field.
- **Adding a native module requires a rebuilt dev client** (Task 8). Everything after it
  type-checks perfectly and fails at runtime with "Cannot find native module".
- **`bun add --filter` does not exist.** It is the reflex from npm and it fails
  confusingly.

### 3. Where the risk concentrates

Four places, in order:

1. **Task 1** changes an extractor used by roughly thirty routes. It is a type-level
   addition and no route's behaviour changes but logout's — and it is still the diff
   where a mistake reaches everything.
2. **Task 5's `23505` mapping.** Getting the constraint name wrong routes every
   collision to the fallback 500. The name is verified against `\d users` in Task 2, not
   assumed.
3. **Task 13 + Task 14's per-account cache and epoch.** The failure mode is one account
   seeing another's garage, and it is invisible until two accounts are used on one
   device — which is why Task 17 tests exactly that.
4. **Task 12's single-flight refresh.** A broken one signs somebody out of every device,
   under load, intermittently.

**The structural carve-out fires on Tasks 2, 5, 6, and 7** — all four touch a column, a
constraint, or a public contract. A review finding on any of them is fixed immediately
rather than deferred to the fix pass, because every later task is built on top.

### 4. Anything the plan is missing

Nothing that blocks execution. Three things the controller should know:

- **`react-hook-form` and `zod` are deliberately not installed** (Task 8's minimality
  check). Plan B installs them. If Plan B assumes otherwise, close it there.
- **`requestId` was added to the `ApiError` interface** — optional, so nothing typed
  against the earlier contract text breaks.
- **Three placeholder screens ship** in Task 16, each marked with a `ponytail:` comment
  naming the plan that replaces it. Without them the gate has nowhere to redirect to and
  cannot be demonstrated, and `typedRoutes` makes a redirect to an empty group a
  compile error.

### Tiers (§28a)

| Task | Writer | Reviewer | Why |
|---|---|---|---|
| 1 | `sonnet` | **`opus`** | sessions + an extractor on thirty routes — the floor list |
| 2 | `sonnet` | **`opus`** | a column and a constraint |
| 3 | `sonnet` | `sonnet` | pure function, complete spec, tests written out |
| 4 | `sonnet` | **`opus`** | rate limiting, and an oracle argument to check |
| 5 | `sonnet` | **`opus`** | a public contract and an auth path |
| 6 | `sonnet` | **`opus`** | a public contract |
| 7 | `sonnet` | **`opus`** | an unauthenticated endpoint |
| 8 | `haiku` | `sonnet` | mechanical — six installs and a lockfile check |
| 9–11, 13, 15 | `sonnet` | `sonnet` | the code is in the plan; no floor-list surface |
| 12 | `sonnet` | **`opus`** | sessions and concurrency |
| 14 | `sonnet` | **`opus`** | sessions and the cross-account leak path |
| 16 | `sonnet` | **`opus`** | access control — the gates are the authorization boundary |
| 17 | `sonnet` | **`opus`** | the contract, verified |

Task 8 is the only foldable one: hold its review and let it ride with Task 9's.

---

## Execution status

Every task unstarted. Tick each as it lands, with anything the next reader needs —
corrections the plan got wrong, deliberate cuts, defects found and how.

- [ ] **Task 1** — `Authenticated` carries `session_id`, logout stops rotating
- [ ] **Task 2** — the `username` and `display_name` migration
- [ ] **Task 3** — the username canonicaliser, in the domain crate
- [ ] **Task 4** — `retry_after_seconds` on the login 429
- [ ] **Task 5** — register takes a username; `23505` names the field
- [ ] **Task 6** — `GET /me` and `PATCH /me`
- [ ] **Task 7** — `GET /usernames/{username}/availability`
- [ ] **Task 8** — the mobile dependencies
- [ ] **Task 9** — secure storage and the pending marker
- [ ] **Task 10** — the session store
- [ ] **Task 11** — the error taxonomy
- [ ] **Task 12** — the API client and single-flight refresh
- [ ] **Task 13** — the query client, persisted per account
- [ ] **Task 14** — `signIn`, `refreshMe`, and the sign-out epoch transaction
- [ ] **Task 15** — the active vehicle (AM-18)
- [ ] **Task 16** — the gates, the route groups, and the bootstrap
- [ ] **Task 17** — contract verification and the visual pass

**Finishing (§31), in order:** consolidate the ledger → fix pass → final gates
(`make check` **and** `make be-sqlx-check`) → `security-review` over the whole branch
diff, because this touches auth, sessions, rate limiting, and adds four endpoints →
redeploy the ticket's Artifact → move AM-17 and AM-18 → show the owner → commit
(`haiku`, several focused commits) → push and watch CI to green → `graphify update .`.

---

## Review findings ledger

Empty. Each finding is written here the moment it arrives — not only in chat, which is
lost to compaction — with: task, severity, file and line, the concrete failure scenario,
and the smallest fix.

Severity vocabulary: `structural` (a column, a constraint, or a public contract — fixed
**immediately**, never deferred) · `correctness` · `test-integrity` · `hygiene`.

| # | Task | Severity | File:line | Failure scenario | Smallest fix | Closed by |
|---|---|---|---|---|---|---|
| — | — | — | — | — | — | — |
