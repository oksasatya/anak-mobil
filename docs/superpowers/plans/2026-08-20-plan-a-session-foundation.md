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
// Corrected during execution: this list held 13 entries including "me", which
// is two characters against a MIN_LEN of 3 — so `canonicalise("me")` returns
// Err(TooShort) and the invariant test below (every reserved name is itself a
// valid username) fails. Nothing is lost by dropping it: the length floor makes
// "me" unclaimable more absolutely than reservation would, and profile
// addresses carry an `@` sigil, so `/@me` could never collide with `GET /me`.
pub const RESERVED: [&str; 12] = [
    "about", "admin", "anakmobil", "api", "edit", "help", "login", "new", "profile",
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
/// exactly the way a taken name is. Twelve entries scanned linearly: `O(1)`
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
- Test: `apps/api/crates/runtime/tests/admin_flow.rs`, `build_flow.rs`, `build_list_flow.rs`,
  `catalog_flow.rs`, `garage_flow.rs`, `parts_flow.rs`, `service_history_flow.rs`,
  `service_summary_flow.rs`

  CORRECTED after Task 5's review (ledger 64). This list named only `auth_flow.rs`.
  Making `username` required on `RegistrationRequest` breaks every caller of
  `/auth/register`, and these eight files each carry their own register-helper call
  site — self-contained by convention, so none of them import a shared harness. Every
  one needs a locally-generated username added to its register body (mirroring
  `a_username()` from Step 1 below). A required field added to a shared request type
  is never a one-file change.

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
    //
    // CORRECTED after Task 5's review (ledger 63). The original sliced the
    // FIRST 20 hex characters of a UUIDv7 — `format!("u{}",
    // Uuid::now_v7().simple())[..20]` — and a UUIDv7's leading hex is mostly
    // the deterministic millisecond timestamp, so almost no entropy survived.
    // Concurrent tests generated colliding usernames: `admin_flow.rs` failed
    // 5-6 of its 13 tests with `left: 409, right: 201`, reproduced three
    // times. Slicing the high-entropy SUFFIX instead fixes it; the result
    // still satisfies the 3-30 length rule and the `^[a-z0-9._]+$` shape.
    format!("u{}", &uuid::Uuid::now_v7().simple().to_string()[13..])
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

CORRECTED after Task 5's review (ledger 64). This step named only `auth_flow.rs`, but
making `username` required on `RegistrationRequest` breaks every caller of
`/auth/register` — and eight more files carry their own register-helper call site:
`admin_flow.rs`, `build_flow.rs`, `build_list_flow.rs`, `catalog_flow.rs`,
`garage_flow.rs`, `parts_flow.rs`, `service_history_flow.rs`, `service_summary_flow.rs`.
Each is self-contained by convention (no shared harness), so each needs its own
locally-generated username added to its register body — the landed fix is a one-line
`let username = format!("u{}", &Uuid::now_v7().simple().to_string()[13..]);` beside the
existing `email` line, then `"username": username` added to the `json!` body. A required
field added to a shared request type is never a one-file change.

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
  `username_message` from `http::auth` (Task 5) — make them `pub(super)` (CORRECTED
  after Task 7's review, ledger 86: `pub(crate)` was one notch wider than the only
  consumer needs — `crate::adapter::http::profile` is a sibling module of `auth`
  under `adapter::http`, which `pub(super)` reaches exactly; `pub(crate)` additionally
  exposed both to `usecase/`, `adapter/postgres/`, and `platform/`, opening a layering
  inversion — a use case calling `check_username` and receiving an HTTP-typed
  `ApiError` — that `make be-boundary` cannot catch, since both live in `runtime`)
  rather than writing a second copy; `RateLimiter::allow` returning `Attempt`
  (Task 4); the `users_username_key` index (Task 2).
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
// CORRECTED after Task 7's review (ledger 85): the original single-function
// helper minted a fresh `a_peer()` on every call, so no test here could ever
// accumulate lookups against one address — the endpoint's only security
// control (the per-IP lookup limiter) had no test capable of exercising it.
// Hoisting the peer to a parameter, with a zero-arg wrapper for the seven
// tests that do not care, is what makes the throttle test below possible.
async fn availability_from(app: &axum::Router, username: &str, peer: SocketAddr) -> Response {
    let mut request = Request::builder()
        .method("GET")
        .uri(format!("/usernames/{username}/availability"))
        .body(Body::empty())
        .expect("building the request");
    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(peer));
    app.clone().oneshot(request).await.expect("infallible")
}

async fn availability(app: &axum::Router, username: &str) -> Response {
    availability_from(app, username, a_peer()).await
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

    // Corrected during execution: the outer `json(...)` is itself async and
    // was never awaited, so this failed to compile ("cannot index into a
    // value of type `impl Future`") rather than running red for the intended
    // reason (404, the route did not exist yet).
    assert_eq!(
        json(availability(&app, &username).await).await["data"]["available"],
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
        json(availability(&app, &username.to_uppercase()).await).await["data"]["available"],
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
    // Corrected during execution: this iterated a COPIED literal list, which is
    // the drift-prone shape Task 3's own twin test avoids. Iterate the constant
    // — a copy silently disagrees with the source the moment either changes,
    // and it already had: the copy still held "me" after Task 3 dropped it,
    // which would have failed here with `available` = null rather than false.
    for name in anakmobil_domain::identity::username::RESERVED {
        assert_eq!(
            json(availability(&app, name).await).await["data"]["available"],
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
    // Corrected during execution: without this, a 404 (the route did not
    // exist yet) has an empty body that trivially contains none of the leak
    // substrings either — the same vacant-pass trap Task 6's red phase hit
    // (`me_never_returns_the_password_hash`), recurring here verbatim.
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let text = String::from_utf8(bytes.to_vec()).expect("utf-8");

    for leak in ["email", "@example.com", "user_id", "created_at", "reserved"] {
        assert!(!text.contains(leak), "the response leaked `{leak}`: {text}");
    }
}

// ADDED after Task 7's review (ledger 85): the Acceptance Criteria below
// already asked for "repeated lookups from one address are eventually
// refused with a 429" — this is that test, not a manual curl deferred to
// Step 7. Without it, deleting the entire rate-limit block from the handler
// left every test in this file green.
#[tokio::test]
async fn repeated_lookups_from_one_address_are_eventually_refused() {
    let app = app!();
    let peer = a_peer();

    let mut throttled = None;
    for _ in 0..70 {
        let response = availability_from(&app, &a_username(), peer).await;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            throttled = Some(json(response).await);
            break;
        }
    }

    let body = throttled.expect("70 lookups from one address were all allowed through");
    assert_eq!(body["error"]["code"], "too_many_requests");
    // The inverse of login's 429: login publishes a wait, this endpoint
    // publishes no `details` at all.
    assert!(body["error"].get("details").is_none(), "{body}");
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
- All **twelve** reserved names answer `false`. CORRECTED after Task 7's review
  (ledger 87): this said thirteen. `RESERVED` is `[&str; 12]` — `me` is two
  characters against a three-character minimum, so it is unclaimable before the
  reserved check is reached, and listing it would break the domain crate's own
  `every_reserved_name_is_itself_a_valid_username` invariant. **Iterate the
  constant rather than restating a count**, so the test cannot drift from it.
- A malformed name is a 422 naming `username`; the rules come from
  `username::canonicalise` and exist nowhere else.
- The response body contains no email, account id, timestamp, or the word "reserved".
- Repeated lookups from one address are eventually refused with a 429. CORRECTED
  after Task 7's review (ledger 85): this criterion had no test — Step 1 wrote no
  such assertion, and Step 7 deferred it to a manual curl that was never run as
  part of the gate. `repeated_lookups_from_one_address_are_eventually_refused`
  in Step 1's test block is that test now; deleting the handler's rate-limit
  block fails it.
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
  export function writeSession(value: Omit<StoredSession, "refreshPending">): Promise<void>;
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

/**
 * Replace the stored session. Always clears `refreshPending`.
 *
 * Takes no `refreshPending` — the field is unconditionally overwritten below,
 * so a signature that accepted it would invite a caller to believe passing
 * `true` kept it set. `Omit` makes that belief a type error instead.
 */
export async function writeSession(value: Omit<StoredSession, "refreshPending">): Promise<void> {
  // CORRECTED after Task 9's review (ledger 10). The original signature was
  // `writeSession(value: StoredSession)`, forcing every caller to pass
  // `refreshPending` even though the line below discards it unconditionally —
  // inviting the belief that passing `true` would be honoured. `Omit` makes
  // that belief a type error instead; every call site in this plan is updated
  // to match.
  //
  // CORRECTED after Task 9's review (ledger 6). The original called
  // `setItemAsync` with no `keychainAccessible` option, leaving the item at
  // iOS's default, `WHEN_UNLOCKED` — included in an encrypted device backup and
  // restored onto different hardware. Restore iPhone A's backup onto iPhone B
  // and both devices hold the same refresh token; the first one to refresh
  // gets the server to treat the other as a thief and revoke every session on
  // every device. `WHEN_UNLOCKED_THIS_DEVICE_ONLY` ties the Keychain item to
  // this device's Secure Enclave key, so it is simply absent from the restore.
  // Still `WHEN_UNLOCKED`, not `AFTER_FIRST_UNLOCK` — there is no background
  // refresh (Task 12), so nothing needs to read it before the first unlock.
  await SecureStore.setItemAsync(KEY, JSON.stringify({ ...value, refreshPending: false }), {
    keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
  });
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
  // CORRECTED after Task 9's review (ledger 7). The original resolved quietly
  // on any reason `readSession` returns null (`if (current === null) return;`)
  // — and it returns null for two different reasons, "no session" and "session
  // unreadable" (bad JSON, wrong shape, or the Keychain rejecting the read
  // because the screen locked mid-refresh). Conflating them is correct in
  // `readSession` itself, where a transient lock must not destroy a good
  // session. It is wrong here: resolving quietly on "unreadable" lets the
  // refresh proceed with the marker never written, and if the response is then
  // lost, the next launch replays the already-rotated token — read by the
  // server as reuse, answered by revoking every session on every device.
  // Throwing instead routes the caller (`refresh.ts`'s `run()`, which does not
  // catch this) to `signOut()`: one device asks for a password, the rest of
  // the account's sessions stay up.
  if (current === null) throw new Error("session unreadable; refresh not marked");
  await SecureStore.setItemAsync(KEY, JSON.stringify({ ...current, refreshPending: true }), {
    keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
  });
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
  export function sessionUserId(): string | null;   // added by Task 14. NOT `use*` — it is a plain read, and eslint `react-hooks/rules-of-hooks` rejects a `use*` name called outside a component.
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
  // CORRECTED after Task 12's review (ledger 23, 37 — found independently by two
  // reviewers). Captured before anything awaits. A sign-out landing between here
  // and the `writeSession` below bumps the epoch; the check immediately before
  // that write catches it and refuses to resurrect a pair the sign-out
  // transaction just wiped from storage.
  const epoch = currentEpoch();
  const stored = await readSession();
  // CORRECTED after Task 12's review (ledger 35). The original tested only
  // `stored === null`, so the pending marker was consulted at cold start and
  // NOWHERE on the in-session path — the one place the catastrophe it guards
  // against actually happens. The sequence it allowed: a refresh rotates the
  // token server-side, its response is lost, the sign-out that follows spends
  // its first awaits on an untimed logout POST over the network that just
  // failed, and any second request 401ing in that window replays the SPENT
  // refresh token. The server reads that as reuse and revokes every session on
  // every device. Multiple concurrent 401s is the exact premise single-flight
  // exists for, so the window is well populated.
  //
  // Same predicate the bootstrap already uses. Must land with ledger 36.
  if (stored === null || stored.refreshPending) {
    throw { kind: "unauthorized", message: "Sesi kamu sudah berakhir." } satisfies ApiError;
  }

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

  // CORRECTED after Task 12's review (ledger 23, 37). The server's ROTATE script
  // already refuses a rotation once sign-out has deleted the `sess:` key, so the
  // usual outcome of racing sign-out is a wasted round trip. This guards the
  // ordering where they do not line up: the rotation reaches the server before
  // the best-effort logout call does, sign-out still bumps the epoch, and without
  // this check the live pair below would land in storage right after sign-out
  // cleared it — the next cold start would then restore the account that just
  // signed out.
  //
  // KNOWN LIMIT, do not read this as more than it is (ledger 56): it catches only
  // a refresh born BEFORE the bump. One born during the sign-out captures the
  // already-bumped value and never trips, because `client.ts` does not test the
  // epoch before entering the refresh branch — only afterwards.
  if (currentEpoch() !== epoch) {
    throw { kind: "unauthorized", message: "Sesi sudah berganti." } satisfies ApiError;
  }

  // `refreshPending` is no longer accepted here — `writeSession`'s signature
  // is `Omit<StoredSession, "refreshPending">` (Task 9, ledger 10), because it
  // is unconditionally overwritten regardless of what a caller passes.
  await writeSession({ access: pair.access_token, refresh: pair.refresh_token });
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
      // CORRECTED after Task 12's review (ledger 36). The original said
      // `await signOut()` unconditionally, which discarded the credentials on a
      // transient network blip — contradicting the spec's session contract,
      // `refresh.ts`'s own comment, the bootstrap note in Task 16, and this
      // task's acceptance criterion "a network failure during refresh does not
      // clear credentials". Somebody entering a tunnel with an expired access
      // token was signed out and had to re-enter a password.
      //
      // Only a REFUSED refresh is final. An offline one leaves the credentials
      // alone; the next launch makes that call. This is only safe alongside the
      // pending-marker guard in `refresh.ts` (ledger 35) — without it, keeping
      // the credentials turns the next 401 into a replay of a spent token.
      if ((err as ApiError).kind !== "offline") await signOut();
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
  export function purgePersistedCache(userId: string): Promise<boolean>;
  export function purgeAllPersistedCache(): void;
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
- MMKV v4 is synchronous and exposes `getString` / `set` / `remove`, which is not the
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
import { createMMKV } from "react-native-mmkv";

/**
 * Server state, cached. Never tokens — those live in expo-secure-store and
 * nowhere else, and this store is plain unencrypted MMKV.
 */
const storage = createMMKV({ id: "am.query" });

/** MMKV is sync and names its methods differently; the persister wants Storage. */
const mmkvStorage = {
  getItem: (key: string) => storage.getString(key) ?? null,
  setItem: (key: string, value: string) => {
    storage.set(key, value);
  },
  removeItem: (key: string) => {
    storage.remove(key);
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

/** Namespaced per account — see `startPersistence`. Shared with the sweep below. */
const CACHE_PREFIX = "am.query.";

function cacheKey(userId: string): string {
  return `${CACHE_PREFIX}${userId}`;
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
    // CORRECTED after Task 13's review (ledger 15). No `dehydrateOptions` was
    // passed, so the default `shouldDehydrateMutation` applies, and a paused
    // mutation dehydrates with its `variables` by default. Once Plan B wires
    // login through `useMutation`, tapping sign-in offline pauses the
    // mutation, the subscriber dehydrates it, and `{email, password}` lands in
    // this plaintext MMKV store the moment the request pauses offline.
    // Nothing here calls `useMutation` yet — which is exactly why this is
    // closed now, before the first screen that does.
    dehydrateOptions: { shouldDehydrateMutation: () => false },
  });

  unsubscribe = unsub;
  await restored;
}

/** Stop writing to disk. Does not delete what is already there. */
export function stopPersistence(): void {
  unsubscribe?.();
  unsubscribe = null;
}

/**
 * Remove one account's persisted cache by id.
 *
 * Kept available for a caller that already knows the id — `signOut` itself no
 * longer calls this directly; `purgeAllPersistedCache` below is the
 * unconditional guarantee it relies on instead, because the id is not always
 * known at sign-out (see that function's doc comment).
 */
export async function purgePersistedCache(userId: string): Promise<boolean> {
  stopPersistence();
  // CORRECTED after Task 13's review (ledger 17). The original returned
  // `Promise<void>` and discarded `storage.remove`'s own boolean — its signal
  // of whether the key existed — so a failed delete was indistinguishable
  // from a no-op one. MMKV is synchronous; the Promise is for the caller's
  // benefit, so that sign-out can await this step in order with the async
  // ones around it.
  const removed = storage.remove(cacheKey(userId));
  await Promise.resolve();
  return removed;
}

/**
 * Remove every persisted query cache, regardless of whose id it is keyed
 * under. Part of the sign-out transaction — called unconditionally.
 *
 * CORRECTED after Task 14's review (ledger 22): this function did not exist
 * in the plan. `sessionUserId()` is null on a path that genuinely happens — a
 * cold start with a stored session calls `fetchMe()` before `setSignedIn`
 * ever runs; a 401 there means refresh gets refused and `signOut()` runs with
 * `store.user` still null. A purge keyed on the id cannot run on that path at
 * all, and without this sweep the account's garage — plate, VIN, service cost
 * — stays on disk in plain unencrypted MMKV after the account is signed out.
 * A prefix scan needs no id, so it is the actual guarantee; `signOut.ts` now
 * calls it unconditionally instead of behind `userId !== null`.
 *
 * ponytail: linear in the number of stored keys — `getAllKeys()` plus a
 * string-prefix check per key. Fine at this app's scale (one cache key per
 * account that has ever signed in on this device); revisit if that stops
 * being small.
 */
export function purgeAllPersistedCache(): void {
  stopPersistence();
  for (const key of storage.getAllKeys()) {
    if (key.startsWith(CACHE_PREFIX)) storage.remove(key);
  }
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
  // CORRECTED after Task 15's review (ledger 43). `signIn` is the only route
  // into a session (this task's own FROZEN CONTRACT above), so this one guard
  // covers every path that starts one: normal login, the bootstrap discarding
  // credentials on a `refreshPending` marker (Task 16), and an Android backup
  // restore where MMKV survives a reinstall but Keystore-backed SecureStore
  // does not. Without it, whichever account's id was last written to
  // `am.client` stays there, and the next account's shell opens on a car it
  // does not own — the exact failure `clearActiveVehicle`'s own doc comment
  // says it exists to prevent.
  clearActiveVehicle();

  // `refreshPending` is no longer accepted here — `writeSession`'s signature
  // is `Omit<StoredSession, "refreshPending">` (Task 9, ledger 10), because it
  // is unconditionally overwritten regardless of what a caller passes.
  await writeSession({
    access: tokens.access_token,
    refresh: tokens.refresh_token,
  });

  const user = await fetchMe();
  setSignedIn(user);
}
```

- [ ] **Step 3: Write `signOut.ts`**

```ts
import { purgeAllPersistedCache, queryClient } from "@/shared/api/queryClient";
import { clearSession, readSession } from "@/shared/session/secure";
import { bumpEpoch, setSignedOut } from "@/shared/session/store";

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

  // Tell the API the session is over. Best-effort by design: the local session
  // ends whether or not this succeeds, because a person on a plane pressing
  // sign-out must still be signed out. A bare fetch rather than apiRequest —
  // importing the client here would close the cycle client.ts -> signOut.ts.
  const stored = await readSession();
  if (stored !== null) {
    // CORRECTED after Task 14's review (ledger 24). The original request had
    // no timeout: left unbounded, a captive portal held this open for as long
    // as iOS URLSession's own default (~60s), and for that whole span
    // `setSignedOut()` below had not run yet — every gate kept rendering the
    // garage of the account that had just pressed sign-out, and every other
    // request in flight sat behind the same wait.
    //
    // `AbortSignal.timeout` is not usable here: React Native polyfills the
    // global `AbortSignal` from the `abort-controller` npm package, and that
    // package's `AbortSignal` has no static `timeout()` — calling it would
    // throw before `fetch` is ever reached, silently swallowed by the empty
    // `catch {}` below that makes this call best-effort, so the request would
    // never be sent at all while the code read as if it had a timeout. The
    // bound is built from `AbortController` + `setTimeout` instead.
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 5000);
    try {
      await fetch(`${process.env.EXPO_PUBLIC_API_URL ?? ""}/auth/logout`, {
        method: "POST",
        headers: { authorization: `Bearer ${stored.access}` },
        signal: controller.signal,
      });
    } catch {
      // Deliberately swallowed. There is nothing to tell the person and
      // nothing to retry: the server's session expires on its own.
    } finally {
      clearTimeout(timer);
    }
  }

  // CORRECTED after Task 14's review (ledger 21). Nothing below this point was
  // previously guarded, so a rejection from `purgePersistedCache` or from
  // `clearSession`'s Keychain delete (the same hazard `secure.ts` already
  // concedes on its read path) aborted `run()` early — still holding the
  // tokens, with `status` left at `"signedIn"`. The person taps *Keluar*,
  // nothing visibly happens, and the tokens stay on disk. Wrapped in
  // `try`/`finally` so the ending is unconditional: the tokens might survive
  // on disk if the Keychain delete itself fails, but the app must still show
  // as signed out.
  try {
    // In-flight requests, cancelled. Anything Query is not managing is covered
    // by the epoch above.
    await queryClient.cancelQueries();
    queryClient.clear();

    // CORRECTED after Task 14's review (ledger 22). The original purged only
    // `if (userId !== null) await purgePersistedCache(userId);` — and
    // `sessionUserId()` is null on a path that genuinely happens: cold start
    // with a stored session calls `fetchMe()` before `setSignedIn` ever runs,
    // a 401 means refresh gets refused, and `signOut()` runs with
    // `store.user` still null. On that path the purge never ran at all, and
    // the account's garage — plate, VIN, service cost — stayed on disk in
    // plain unencrypted MMKV after sign-out. `purgeAllPersistedCache` (Task
    // 13, ledger 22) needs no id, so it is called unconditionally instead.
    purgeAllPersistedCache();
  } finally {
    // Unconditional even if the try above threw. `.catch` rather than letting
    // a rejection here skip `setSignedOut()` too — the tokens might survive on
    // disk if the Keychain delete itself fails, but the app must still show as
    // signed out; a person who cannot delete a token should not be told they
    // are still logged in.
    await clearSession().catch(() => {});
    setSignedOut();
  }
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
export function sessionUserId(): string | null {
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
- **Cleared by sign-out, and by sign-in too.** A vehicle id is not sensitive on its own,
  but it is the previous account's, and the next account has no car with that id — every
  query keyed on it would 404. Task 14's `signOut` calls `clearActiveVehicle()`; add
  that call when this task lands (Step 2). `signOut()` alone is not enough, though — a
  bootstrap path that discards credentials without calling `signOut()` (Task 16) leaves
  a stale id in place, so Task 14's `signIn()` clears it too, as the only path a session
  can start through (ledger 43; Step 3).

- [ ] **Step 1: Write the module**

```ts
import { createMMKV } from "react-native-mmkv";
import { create } from "zustand";

/**
 * Client state, in its own store.
 *
 * Not in the query cache: that holds server state and is cleared, busted, and
 * keyed per account. Which car somebody is looking at is a preference, and it
 * should survive a cache bust the way a scroll position would.
 */
const storage = createMMKV({ id: "am.client" });
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
    storage.remove(KEY);
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

In `apps/mobile/src/shared/session/signOut.ts`, add the import and the call, as the last
statement of the `try` block guarding cleanup — right after `purgeAllPersistedCache()` and
before the `finally` that runs `clearSession()`. (That `try`/`finally` shape is Task 14's
own correction, ledger 21 — `clearActiveVehicle()` joins the cleanup it guards, not a bare
sequence before `clearSession()`.)

```ts
import { clearActiveVehicle } from "@/shared/vehicle/activeVehicle";
```

```ts
    purgeAllPersistedCache();
    clearActiveVehicle();
  } finally {
    await clearSession().catch(() => {});
    setSignedOut();
  }
```

- [ ] **Step 3: Wire it into sign-in**

CORRECTED after Task 15's review (ledger 43). `signOut()` is not the only enforcement
point for "the active vehicle belongs to the signed-in account" — every route to a
*new* session that bypasses it leaves the value stale, and Task 16's `useBootstrap`
does exactly that: it discards credentials on a `refreshPending` marker, and again on
a failed `fetchMe()`, without ever calling `signOut()`. Neither branch bumps the
epoch, purges the cache, or clears the vehicle. Concretely: A's app is killed
mid-refresh, relaunch finds the marker and drops to welcome with A's id still in
`am.client`, B signs in on the same device, and `signIn()` (Task 14) touched none of
it — B's shell opens on A's vehicle id and every query keyed on it 404s. Same shape
via an Android auto-backup restore, where MMKV survives but Keystore-backed
SecureStore does not.

The root-cause fix is one guard in `signIn()` rather than a patch in every branch that
starts a session — `signIn()` is declared by the FROZEN CONTRACT (Task 14) as the
*only* way a session starts, so this one guard covers the bootstrap's two branches,
the backup-restore case, and a sign-out killed after `bumpEpoch()`. No cycle:
`activeVehicle.ts` is a leaf, so `signIn.ts` can import it without importing anything
back. Accepted cost: a same-account re-auth via the `refreshPending` path loses the
selected car and opens on the default instead of a 404 — the better of the two.

In `apps/mobile/src/shared/session/signIn.ts`, add the import and the call as the
first statement of `signIn()`, before `writeSession`:

```ts
import { clearActiveVehicle } from "@/shared/vehicle/activeVehicle";
```

```ts
export async function signIn(tokens: {
  access_token: string;
  refresh_token: string;
  expires_in: number;
}): Promise<void> {
  clearActiveVehicle();

  await writeSession({
    access: tokens.access_token,
    refresh: tokens.refresh_token,
  });

  const user = await fetchMe();
  setSignedIn(user);
}
```

- [ ] **Step 4: Run the gates yourself**

```bash
bun run format
make mb-check
```

Expected: EXIT=0 on both.

**Acceptance criteria**
- Its own MMKV instance, separate from the query cache.
- The initial value is read synchronously at module load.
- `signOut` clears it.
- `signIn` clears it too, before `writeSession` (ledger 43) — the only way a session
  starts is the only place this invariant can be enforced once, for every path in.
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
- Two of the three `index.tsx` screens Step 3 writes are placeholders — `(auth)/index.tsx`
  and `(onboarding)/index.tsx` — and are exactly that. Mark each with a `ponytail:` comment
  naming the plan that replaces it; without them there is nothing to redirect to and the
  gate cannot be demonstrated. `(app)/index.tsx` is the third and is not a placeholder —
  see Step 3 below.

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
export { refreshMe } from "./api/me";
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

- [ ] **Step 3: Write the two placeholder screens, and the real `(app)` one**

  CORRECTED after Task 16's review (ledger 98): this heading said "three". Only
  `(auth)/index.tsx` and `(onboarding)/index.tsx` are placeholders — `(app)/index.tsx`
  is the real AM-14 connectivity screen, moved here rather than stubbed, as the step's
  own body already said. The heading and the body disagreed, **the reviewer brief
  inherited the heading's number, and the reviewer then had to correct the brief** —
  which is how one stale line costs three readers.

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
- [x] **Task 4** — `retry_after_seconds` on the login 429. Landed, TDD honoured (red confirmed
  first). Controller re-ran all five gates: `be-fmt`, `be-lint`, `be-test`, `be-boundary`,
  `be-sqlx-check` — every one EXIT=0, and the **14 `auth_flow` integration tests were verified to
  have genuinely run against live Postgres and Redis** rather than taking the env-guard skip path
  that ledger 20 records. `be-sqlx-check` unchanged, as predicted — no SQL touched. Reviewer in
  flight on `opus`.

  **The writer refused part of the controller's brief, and was right to.** The brief (carrying
  ledger 12) said to map `AuthError::InvalidCredentials` to the new code inside `to_api_error`.
  The writer read `usecase/auth.rs` first and found that variant is **shared** between `login`
  (wrong password / unknown email) and `refresh` (`Rotation::Invalid` / `Rotation::Reused`), and
  that `to_api_error` serves all four handlers — so the literal edit would have made an expired or
  **replayed refresh token** answer *"Email atau password salah."*, which is wrong copy for a flow
  with no password in it. It added `to_login_error`, used only by `login()`, and left
  `to_api_error` mapping `InvalidCredentials → Unauthorized`, which is correct for refresh.
  **The controller's brief was wrong; the writer checked the code instead of obeying it.** The
  reviewer has been asked to judge whether the split is complete and whether the two mappers can
  now drift.

  **The plan's red-test prediction was wrong for the second time** (see Task 1). It expected a
  missing-function error plus a runtime assertion; the real first red was
  `error[E0600]: cannot apply unary operator '!' to type 'LoginAttempt'` at `auth.rs:214`, because
  `allow_login`'s return type changed before the handler was updated. Structurally the same defect
  the step intended to force, surfacing at a different point.
- [ ] **Task 5** — register takes a username; `23505` names the field
- [x] **Task 5** — register accepts a username; `23505` names the field that collided. Landed, TDD
  honoured. Reviewer in flight on `opus` — **this task is on the structural carve-out list**, since
  it changes a public contract.

  **The controller's first gate run came back RED, against the writer's reported green** — one
  failure, `the_cursor_cannot_be_used_to_probe`. It was investigated rather than retried, and it is
  **not** a Task 5 regression: it fails in isolation, and it fails on a clean `dev` worktree. Root
  cause is a non-hermetic test (ledger 62). After `make db-drop && make db-seed` the full suite
  passed. **Final verified state: all five gates EXIT=0, 17 suites, zero failures.** This is the
  clearest example in the run of why a writer's gate report is a claim rather than evidence — and
  also of why "it went red, run it again" is the wrong reflex.

  **Two defects the writer found in the plan itself**, both recorded as ledger 63 and 64 and both
  since corrected in the plan text: the `a_username()` helper sliced the *low-entropy front* of a
  UUIDv7 and was generating genuine username collisions (`admin_flow.rs` failing 5–6 of 13,
  reproduced three times), and Task 5's file list named one test file where nine needed changing.

  **Review complete — NO structural findings; the carve-out is clean and the contract is
  byte-exact.** The reviewer verified the constraint names **against the live database** rather than
  reading them, including the one thing that could have silently routed every username collision to
  a 500: a *partial* unique index does populate `constraint_name`. It also confirmed the unmapped
  fallback leaks nothing — `PgDatabaseError::Display` prints `message()` only, **not `DETAIL`**,
  which on a 23514 would contain the row's `password_hash`. Eight findings, all `hygiene` or
  `test-integrity` (ledger 65–72), none blocking Tasks 6, 7, 16, 17 or Plan B. Two are the kind
  worth naming: a test that is pinned by *another task's* constraint rather than its own assertion
  (65), and one that **stops testing anything after its first run** against a shared database (66).
  The reviewer also corrected ledger 63's stated *reason* while confirming its fix (72).
- [ ] **Task 6** — `GET /me` and `PATCH /me`
- [x] **Task 6** — `GET /me` and `PATCH /me`. Landed, TDD honoured. Controller re-ran all five
  gates: every one EXIT=0, 18 suites, and `profile_flow` confirmed genuinely running 6/6. Two new
  `.sqlx` cache entries committed to the working tree. Reviewer in flight on `opus` —
  **structural carve-out**, since this adds a public contract Plans B, C and D are built on.

  **The writer caught a vacuous test in its own red phase, which is the behaviour these briefs keep
  asking for and rarely get.** The plan predicted every test in the new file would fail with 404.
  Five did. `me_never_returns_the_password_hash` **passed before any implementation existed** — it
  asserts only that the body lacks the substrings `"argon2"` and `"password"`, and a 404 error body
  lacks them too. It reported this rather than banking the green. Recorded as ledger 73.

  `has_vehicles` is computed in the same `query_as!` via an `EXISTS` subselect against `vehicles` —
  no stored completion flag, one query, no N+1. Ledger 65's canonical-username assertion was folded
  in as instructed: it registers `"  U2308D350E19D27DC7D1  "` — uppercased and padded — and asserts
  `GET /me` reads back the canonical lowercase form.
- [ ] **Task 7** — `GET /usernames/{username}/availability`
- [x] **Task 7** — `GET /usernames/{username}/availability`. Landed, TDD honoured (red confirmed
  empirically for the intended reason: 404, the route did not exist). All five gates re-run by the
  controller: `be-fmt`, `be-lint`, `be-test`, `be-boundary`, `be-sqlx-check` — every one EXIT=0, 19
  suites, `profile_flow` running 13/13 (6 pre-existing `GET`/`PATCH /me` + 7 new). One new `.sqlx`
  cache entry committed (`username_exists`'s `SELECT EXISTS(... WHERE username = $1::citext)`).

  **Two bugs found in the plan's own copied test text, both fixed before red was trusted.** First,
  a straightforward compile error: three assertions wrote `json(availability(...).await)["data"]`
  — the inner `.await` resolves `availability(...)` to a `Response`, but the outer `json(...)` call
  (itself `async`) was never awaited before being indexed, so it failed to compile as "cannot index
  into a value of type `impl Future`". Fixed by awaiting the outer call too:
  `json(availability(...).await).await["data"]`.

  Second, the vacant-pass trap the brief warned about (it happened on Task 6 too) recurred verbatim:
  `availability_never_mentions_an_email_or_an_account` asserted only that the body lacks certain
  substrings, with no status assertion — so it passed against the pre-implementation 404's empty
  body exactly as readily as it would against a real 200. Added `assert_eq!(response.status(),
  StatusCode::OK)` before the leak checks; confirmed it now fails 404≠200 pre-implementation and
  passes post-implementation. **The plan's Step-2 red prediction is now wrong five times running**
  (Tasks 1, 4, 5, 6, and this compile-error instance in 7) and the vacant-pass trap has now fired
  twice (Tasks 6 and 7) — worth folding into the brief template itself rather than re-catching per
  task.

  **`EXPLAIN` proved the `::citext` cast claim directly**, both directions, against the live
  database: `SELECT EXISTS(SELECT 1 FROM users WHERE username = $1::citext)` plans as `Index Only
  Scan using users_username_key on users … Index Cond: (username = '…'::citext)`; the same query
  with the cast dropped plans as `Seq Scan on users … Filter: ((username)::text = '…'::text)`.

  **Reserved and taken confirmed equally cheap** — five-request samples against a taken name (~1.0–
  1.5ms) and against `admin` (~1.0–1.2ms) landed in the same band, no order-of-magnitude gap (unlike
  registration's ~20× argon2-skip gap on a reserved name). Neither path touches argon2.

  **The 429 was verified empirically, not assumed from the unit tests**: 55 sequential lookups from
  one address, followed by one more, produced `429 too_many_requests` with the standard Bahasa
  Indonesia message and no `retry_after_seconds` (this endpoint uses the bare `too_many_requests()`,
  not the login path's `too_many_requests_in`, matching the FROZEN CONTRACT — only `/auth/login`'s
  429 carries that detail).

  `check_username` and `username_message` in `http::auth` were widened to `pub(crate)` as the brief
  required, so `profile::availability` reuses Task 5's canonicaliser wrapper rather than growing a
  second copy of the username rules. Reviewer in flight on `opus` — **not on the structural
  carve-out list**: the route is new and additive, and it changes no existing signature in the
  FROZEN CONTRACT.
- [x] **Task 8** — the mobile dependencies — DONE 2026-08-20. Writer `haiku`. Installed
  `expo-secure-store@57.0.1` and `react-native-mmkv@4.3.2` via `bun x expo install` (native,
  SDK-matched), and `@tanstack/react-query@5.101.4` + `@tanstack/react-query-persist-client` +
  `@tanstack/query-sync-storage-persister` + `zustand@5.0.15` via `bun add --cwd`. Added the
  `expo-secure-store` plugin to `app.config.ts`. `react-hook-form` and `zod` correctly NOT
  installed — Plan B installs zod, nobody installs react-hook-form. Controller re-ran the gates:
  `bun install --frozen-lockfile` EXIT=0 with `bun.lock` unchanged by the second run, no nested
  lockfile under `apps/mobile`, `make mb-check` EXIT=0. Review folded into Task 9's per the
  execution-mode verdict.

  **ENVIRONMENT FACT discovered here, add it to the card — `make mb-run-dev` needs a UTF-8
  locale.** The dev-client rebuild the two native modules require failed on `pod install`, and
  the visible error was CocoaPods crashing *inside its own error reporter*
  (`Encoding::CompatibilityError` in `error_report.rb`), which hides the real cause. The real
  cause is printed one line earlier: "CocoaPods requires your terminal to be using UTF-8
  encoding." `LANG` and `LC_ALL` are empty in a non-interactive shell here, and `locale` reports
  `LC_CTYPE="C"`. Run it as `LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8 make mb-run-dev p=ios`.

  **A second trap, recorded because it nearly cost a false green.** The first rebuild attempt was
  reported as exit 0 when it had failed: the command ended in a pipe to `tail`, and the exit code
  belonged to `tail`. `make` had exited 1. Read `make`'s own exit code, never the tail of its log.
- [x] **Task 9** — secure storage and the pending marker. Landed; gates EXIT=0. Review found two
  high-severity `correctness` defects (ledger 6, 7), both with account-wide revocation as the
  blast radius. Neither is structural, so both go to the fix pass.
- [x] **Task 10** — the session store. Landed; gates EXIT=0. Review: three `hygiene` (ledger 1–3)
  plus one hazard handed forward to T16 (ledger 5).
- [x] **Task 11** — the error taxonomy. Landed; gates EXIT=0. Review found two wrong-address error
  messages (ledger 11, 12) — the second is server-side and was **folded into Task 4's brief**
  rather than deferred, and one ordering constraint for T17 (ledger 14).
- [x] **Task 12** — the API client and single-flight refresh. Landed; gates EXIT=0. Reviewer in
  flight. Its `writeSession`-after-`clearSession` hazard was independently found by Task 14's
  reviewer (ledger 23), confirming ledger 8's prediction of a "worse twin".
- [x] **Task 13** — the query client, persisted per account. Landed. **Deviated from the plan's
  literal snippet and was right to:** `react-native-mmkv@4.3.2` is a Nitro rewrite — `MMKV` is a
  type-only export, instances come from `createMMKV(config)`, and removal is `remove(key)`, not
  `delete`. An independent reviewer verified all three against the installed package source. Gates
  re-run by the controller: `make fmt-check` and `make mb-check` both EXIT=0. Review found no
  structural defects; findings 15–18 in the ledger.
- [x] **Task 14** — `signIn`, `refreshMe`, and the sign-out epoch transaction. Landed; gates
  re-run EXIT=0. Reviewer in flight.
- [x] **Task 15** — the active vehicle (AM-18). Landed. Applied the same MMKV v4 correction
  (`createMMKV({ id: "am.client" })`, `storage.remove`) — a **separate** instance from the query
  cache's `am.query`. Exports the two FROZEN CONTRACT symbols plus an internal `clearActiveVehicle()`,
  wired into `signOut.ts` between the cache purge and `clearSession()`. Controller re-ran
  `make fmt-check` EXIT=0 and `make mb-check` EXIT=0. Reviewer in flight on `opus` — the diff edits
  sign-out ordering, which is on the floor list.
- [x] **Task 7** — `GET /usernames/{username}/availability`. Landed, TDD honoured, **and it is the
  last backend task in the plan.** Controller re-ran all five gates: every one EXIT=0, 18 suites,
  `profile_flow` now running 13 tests (6 from Task 6 + 7 here). Reviewer in flight on `opus` — an
  unauthenticated endpoint is the cheapest enumeration surface in the product.

  **It hit the same vacuous-test trap Task 6 found — and unlike Task 6's writer, it fixed it.**
  `availability_never_mentions_an_email_or_an_account` passed vacantly pre-implementation, because a
  404's empty body contains none of the leak substrings either. It added the status assertion and
  re-confirmed the red. **It also found a compile bug in the plan's own copied test text:** three
  assertions wrote `json(availability(...).await)["data"]` where the outer `json(...)` — itself
  `async` — was never awaited, so they failed with `cannot index into a value of type impl Future`
  rather than the intended 404. Both classes corrected in the plan file, matching how Tasks 5 and 6
  handled plan-text defects.

  Proved rather than asserted: `EXPLAIN` shows `Index Only Scan using users_username_key` **with**
  the `::citext` cast and `Seq Scan` **without** — ledger 33 was real. Reserved (~1.0–1.2 ms) and
  taken (~1.0–1.5 ms) sit in the same band, so there is no timing oracle here; neither path touches
  argon2, unlike registration's ~20× reserved-name gap that Task 5's reviewer flagged (ledger 68).
  And the plan's prose saying "thirteen reserved names" is stale — `RESERVED` is `[&str; 12]`, which
  the controller corrected back in Task 3.
- [ ] **Task 16** — the gates, the route groups, and the bootstrap
- [x] **Task 16** — the gates, the route groups, and the bootstrap. Landed. Controller re-ran and
  re-checked: `make fmt-check` EXIT=0, `make mb-check` EXIT=0 — **and it now runs `bun test` inside
  the gate** (4 pass, 0 fail), so the CI wiring decided earlier this run is live — plus zero deep
  imports (`grep -rn "@/shared/" src/app` → 0 hits) and `.env.development` unmodified. Reviewer in
  flight on `opus`. **Task 17's eight-state screenshot matrix is deliberately NOT done here** — that
  is Task 17's own job with its own criteria; what ran here proves Task 16's gates.

  **Verified against a live backend, seven scenarios on the iPhone 17 simulator** with three real
  accounts: cold start with no session → `(auth)`; sign in with an incomplete profile →
  `(onboarding)`; force-quit and relaunch while incomplete → **splash held, no flash of `(auth)`**;
  complete profile → `(app)`; sign out → one redirect; relaunch after sign-out → stayed out; and a
  **deep link while signed out** (`simctl openurl "anakmobil://(app)"`) → redirected rather than
  exposing the screen. Temporary debug buttons and a port override were reverted, confirmed clean.

  **One claim of mine that turned out wrong, recorded because I checked it.** The writer reported
  zero `router.replace`/`router.push` calls; my broader grep found one. It is **prose inside a doc
  comment** at `gates.tsx:29`, not a call. The criterion holds and the writer was right.

  **ENVIRONMENT FACTS discovered here, both worth the card.** Port **8080 is occupied by an
  unrelated Java service** on this machine — an HTTP check against it tests somebody else's 404
  (Task 7's reviewer hit the same thing independently). And a **stale Metro process from an earlier
  task was still serving the OLD `.env.development` value**, silently swallowing an env edit until
  it was found and killed. A dev server outliving its task is not inert; it serves stale config to
  the next one.

  **Ledger 61 was ACCEPTED, not closed, and the acceptance is written into `bootstrap.ts`** — a
  retry does not help, since the delete failed because the Keychain was locked and an immediate
  retry finds it locked too; a tombstone durable enough to survive that same failure would have to
  live outside the Keychain, reopening the same durability question one layer down. Blast radius
  argued as per-device, same-person, still bound by the server's TTL and rotation. The reviewer has
  been asked to judge that reasoning rather than the fact that a sentence exists.
- [x] **Task 17** — contract verification and the visual pass. **Landed. Plan A is 17/17.**

  **Contract: all 14 frozen symbols verified one by one**, each matching its signature exactly, via
  a scratch file importing every one of them (`make mb-check` EXIT=0, then deleted). Zero deep
  imports under `src/app`. `fetchMe` confirmed **absent** from the barrel (ledger 96's fix holds)
  and `clearActiveVehicle` confirmed internal.

  **Eleven states exercised live on the iPhone 17 simulator, light and dark**, against a real API on
  `127.0.0.1:8099`. **Zero defects.** Highlights, all with real evidence rather than description:
  - **Single-flight**: 5 concurrent `refreshMe()` against an invalidated token → **exactly 1**
    `POST /auth/refresh`, then 5 successful `GET /me` retries.
  - **Ledger 9 — force-quit mid-refresh, then relaunch**: landed on `(auth)` asking for a password,
    with **0** refresh calls on relaunch. Never run before this.
  - **Ledger 41 — the same without relaunching**: the marker was read and the refresh rejected
    immediately, **0** additional calls, and the app signed itself out locally with kind
    `unauthorized` rather than `offline`. Never run before this.
  - **Offline**: `kind=offline`, *"Tidak ada koneksi…"*, **session left intact** — the fix from
    ledger 36 doing exactly what it was written for.
  - **Rate limited**: reproduced end to end, `kind=rateLimited retryAfterSeconds=468`.
  - **Ledger 43/44 — the active vehicle**: survived a force-quit, cleared by sign-out, and
    `signIn()`'s own `clearActiveVehicle()` guard confirmed by a fresh sign-in always reading empty.

  **Two honest gaps, reported rather than papered over:**
  - **State 1 (splash mid-flight) was never captured as an isolated frame** — resolution is
    sub-second on localhost and no screenshot landed mid-transition across 8+ relaunches. Verified
    indirectly instead: 7 relaunches under four different session conditions each landed on the
    right screen with no flash of the wrong one.
  - **State 7 (per-account cache isolation) is not yet a meaningful test.** Identity isolation was
    confirmed across five accounts, but **Plan A ships no `useQuery` consumer at all**, so the
    persisted cache is verifiably empty — the writer dumped its raw keys and found zero
    `am.query.<userId>` entries. The mechanism is code-verified sound; there is simply nothing to
    leak until Plan C adds a cached screen. **Task 15's claim that this was verified here
    overclaimed, and ledger 44 already records that.**

  Every temporary affordance reverted and proved: `.env.development` byte-identical to HEAD, both
  edited untracked screens re-read and confirmed byte-identical, the scratch contract file deleted.
  Final `git status --short` matched the starting snapshot with zero residue.

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
| 1 | T10 | hygiene | `shared/session/store.ts:70` | `setSignedOut` is the only export with no doc comment and the most dangerous to call directly — it does **not** bump the epoch. A future caller reaching for it on a 401 instead of `signOut()` leaves the epoch unchanged, and an in-flight response then repopulates a cache that was just cleared: the exact cross-account leak the epoch exists to prevent. | One doc line above it saying it is the LAST statement of the sign-out transaction and that `signOut()` is what callers want. **Do not** fold `bumpEpoch()` into it — `signOut` bumps first by design. | fix pass |
| 2 | T10 | hygiene | `shared/session/store.ts:4-10` | `Me`'s five fields are mutable, so a component doing `user.displayName = "x"` mutates the store in place with no subscriber notified. | `readonly` on each field — zero runtime cost, does not break assignability, so Plans B/C/D stay typed. Amend the plan's frozen Interfaces text in the same pass. | fix pass |
| 3 | T10 | hygiene | `shared/session/store.ts:47` | `bumpEpoch(): number` returns a value no consumer uses. | `void`, if the frozen Interfaces block is being amended anyway. Otherwise leave — churning a frozen signature buys nothing. | fix pass |
| 4 | T8 | hygiene | `apps/mobile/package.json` | `react-native-nitro-modules@0.36.5` is a required peer of `react-native-mmkv@4.x` but is undeclared — it exists only in bun's isolated store. Native autolinking finds it today (proved by `ios/Podfile.lock`) and `--frozen-lockfile` pins it, so this is safe now; it breaks only if bun's peer-resolution changes. | `bun add --cwd apps/mobile react-native-nitro-modules`, which is also what MMKV's own install instructions say. | fix pass |
| 6 | T9 | **correctness (high)** | `shared/session/secure.ts:70,83` | **`keychainAccessible` is left at its default `WHEN_UNLOCKED`, which is included in an encrypted device backup and restored onto different hardware.** Restore iPhone A's backup onto iPhone B and both hold the *same* refresh token; the moment both refresh, the server sees reuse and revokes every session on every device — the exact catastrophe the single-flight promise and the pending marker exist to prevent, arriving through a door neither watches. | `keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY` on both `setItemAsync` calls. Reads need nothing — the attribute is set at write time. Android unaffected (iOS-only option; its Keystore key is already non-exportable). Nothing is stored yet, so there is no migration concern. The *tier* stays `WHEN_UNLOCKED` — T12 established there is no background refresh. | fix pass |
| 7 | T9 | **correctness (high)** | `shared/session/secure.ts:80-84` | `markRefreshPending()` resolves **successfully having written nothing** when `readSession()` returns null for a failure reason rather than "no session" — unparseable JSON, wrong shape, or `getItemAsync` rejecting (a `WHEN_UNLOCKED` item is unreadable the instant the screen locks). Sequence: refresh starts → screen locks → marker silently not written → server rotates → response lost → next launch replays the old token → **every session on every device revoked.** The conflation is right for `readSession` (a transient lock must not destroy a good session) and wrong for the write path. | Fail closed: `const current = await readSession(); if (current === null) throw new Error("session unreadable; refresh not marked");`. The existing caller does not wrap it, so the throw propagates → `signOut()` → **this one device asks for a password and the account keeps its other sessions**, which is the designed outcome. | fix pass |
| 8 | T9 | correctness (low) | `shared/session/secure.ts:81-83` | `markRefreshPending` is a read-modify-write across an `await`, and `clearSession()` can land in the middle, writing the token pair back **after** sign-out. Mostly self-heals (next launch finds the marker and clears), but `POST /auth/logout` is explicitly allowed to fail offline, so the narrow case is a live refresh token resurrected into the Keychain until the next launch. | A `ponytail:` comment naming the ceiling. A real fix costs an epoch check inside `secure.ts`, which is new coupling for a narrow case. **Its worse twin belongs to T12's review:** `writeSession` writes a *fresh, live* pair with no epoch check after a sign-out may have cleared it. | fix pass |
| 9 | T9 | test-integrity | plan T17 matrix | Task 9's `TDD: no` is justified by "corrupt the marker by hand and confirm the relaunch asks for a password" — **and Task 17's eight-state matrix has no such row.** The one behaviour that makes the module worth existing has no coverage of any kind. "Corrupt by hand" is also not runnable; a simulator Keychain item is not hand-editable. | Add row 9 to T17: `redis-cli DEL "at:<digest>"`, stop the API so the refresh hangs, force-quit mid-request, restart the API, relaunch. Expect: lands on `(auth)` asking for a password, and **no `POST /auth/refresh` in the log on relaunch**. | fix pass (plan edit) |
| 10 | T9 | hygiene | `shared/session/secure.ts:69` | `writeSession(value: StoredSession)` forces every caller to pass `refreshPending`, which it then unconditionally discards — a signature that invites the belief `true` would be honoured. | `Omit<StoredSession, "refreshPending">`. Not in the FROZEN CONTRACT, so internal. **The cheap window has closed** — T12 and T14 have landed, so this is now two landed-file edits plus two plan snippets, done as one change or the excess-property check reddens the gate. | fix pass |
| 11 | T11 | **correctness** | `shared/api/errors.ts:141` | On a 409 the banner shows `ErrorCode::Conflict`'s catalogue text — **"Data ini sudah berubah. Muat ulang, lalu coba lagi."** — regardless of the field detail. A register screen rendering `message` as a banner tells somebody with a taken email to reload the page. Executed and confirmed by the reviewer. | In the 409 branch only: `const first = Object.values(fields).at(0); message: first ?? message`. Or a rule in Plan B suppressing the banner whenever `fields` is populated. | fix pass |
| 12 | T11 | **correctness — server-side, folded into T4** | `apps/api/.../i18n.rs` + `adapter/http/auth.rs:278` | A failed login reads **"Kamu perlu masuk dulu."** — the app tells somebody mistyping a password to do the thing they are doing. It is the most-read error string in the product. Because `ApiError` deliberately carries no `code`, Plan B cannot distinguish it from a dead-session 401 without parsing prose, which `i18n.rs` forbids. | Server-side: add `ErrorCode::InvalidCredentials` (wire form `auth.invalid_credentials` — the exact example `i18n.rs` itself gives) → 401, "Email atau password salah." **This creates no enumeration oracle**: unknown email and wrong password both raise `AuthError::InvalidCredentials` and stay byte-identical; what becomes distinguishable is bad-credentials vs no-token, which says nothing about whether an account exists. **Folded into Task 4's brief** rather than deferred, because T4 owns the login error path and had not been dispatched yet. | **CLOSED by T4** — but *not* as the brief specified. `ErrorCode::InvalidCredentials` (`auth.invalid_credentials`, 401, "Email atau password salah.") was added as instructed, but mapped through a **new `to_login_error` used only by `login()`** rather than through the shared `to_api_error`, because that variant is also raised by `refresh` on a replayed token — where the password copy would be wrong. The controller's brief was mistaken on that point and the writer checked the code instead of obeying it. |
| 13 | T11 | hygiene ×3 | `shared/api/errors.ts:50,150,42` | `narrowEnvelope`'s doc says "answer null" but any object returns `{}`; a 503 with a valid envelope discards the server's own message for the generic constant, contradicting the module's "server wins" docstring for one status class; `RATE_LIMITED` differs from the server's string by one word — the two-copies-drifting shape the precedence rule exists to prevent. | Docstring clauses for the first two; delete the third's local copy or accept it as fallback-only and say so. | fix pass |
| 14 | T17 | **sequencing — must not be forgotten** | plan T17 | `conflict_on` and `retry_after_seconds` do not exist on the server yet (T4 and T5 unwritten at the time of T11's review). Against today's server a taken email is a 409 with no details and maps to `kind: "server"`, and a 429 carries no countdown. **T17's live verification must run after T4 and T5**, or it fails Task 11 for somebody else's missing work. | Ordering constraint, not a code change. | T17 |
| 15 | T13 | **correctness** | `shared/api/queryClient.ts:73-80` | No `dehydrateOptions` is passed, so the default `shouldDehydrateMutation` applies — `mutation.state.isPaused` — and a dehydrated mutation carries its **`variables`**. Once Plan B wires login through `useMutation`: tap sign-in offline → the mutation pauses → the subscriber dehydrates it → `{email, password}` lands in **plaintext MMKV** and survives until buster/maxAge. Nothing is exposed today (the plan contains zero `useMutation`), so the acceptance criterion's "asserted by inspection" is currently true — but it is a promise, not a boundary, and Plan B is the thing that breaks it. | One line: `dehydrateOptions: { shouldDehydrateMutation: () => false }`. | fix pass |
| 16 | T13 | correctness | `shared/api/queryClient.ts` | A concurrent `startPersistence` can hydrate the previous account into the new one's key. `persistQueryClient`'s internal guard stops subscription leaks and double-writes, but `unsubscribe()` does **not** abort an in-flight restore: `persistQueryClientRestore` still calls `hydrate()` into the shared singleton, and `hydrate` merges rather than replaces. Path: bootstrap starts persistence for A → a 401 forces `signOut()` → `signIn(B)` → A's restore resolves and hydrates A's data → B's subscriber writes the merged cache into `am.query.B`. The per-account key does not save this, because the write goes to B's key. Latent, not live — one sequential call site today. | A module generation counter checked inside a wrapped `restoreClient`. | fix pass |
| 17 | T13 | correctness (low) | `shared/api/queryClient.ts:97` | `purgePersistedCache` can be undone by an already-scheduled throttled write: the persister throttles at 1000 ms and the timer is not cancellable, and `stopPersistence()` only drops the subscription. In sign-out's order (`cancelQueries` → `clear()` → purge), `clear()` schedules a save, the delete runs, and ~1 s later the timer rewrites the key. **Honest impact: small** — the throttle keeps only the latest params, which after `clear()` is an *empty* cache, and `buster`/`maxAge` still gate it. It is not a leak; it is the docstring being not-quite-true. Separately, `storage.remove()` returns `boolean` and line 97 discards it, so Task 14 cannot tell a failed delete from a successful one. | `return storage.remove(...)`, plus a `ponytail:` comment naming the throttle ceiling. | fix pass |
| 18 | T13 | **gap, folded into T16** | plan T16 / `shared/api/queryClient.ts` | `startPersistence` has exactly **one** call site in the entire plan — `useBootstrap` — and **`signIn` never calls it**. So the session in which somebody actually signs in runs entirely unpersisted: not merely the restore is skipped (correct — there is nothing to restore) but the *subscribe* too, so nothing is written for that whole session and the next launch's bootstrap finds an empty cache. The file's own comment cites AM-18 reading from cache as the reason for its `staleTime`/`gcTime` values, so this reads as an omission rather than a decision. | `startPersistence(userId)` after a successful `signIn`. **Folded into Task 16's brief** rather than deferred — T16 owns bootstrap and the route gates and had not been dispatched. | T16 |
| 19 | T1 | test-integrity ×3 | `apps/api/.../tests/auth_flow.rs:427` | The test named `a_logout_against_a_dead_session_answers_like_a_live_one` asserts the property **the spec retracted**; the corrected property (a dead token is indistinguishable from any other dead-token request) is left unpinned; and the intermediate logout at line 427 is unasserted, so the test passes without the session ever having been alive. | Rename to the property that survived, assert the intermediate logout's status, and pin the corrected uniformity claim. | fix pass |
| 20 | T1 | observation, pre-existing | `apps/api/.../tests/auth_flow.rs:57-78` | The `DATABASE_URL`/`REDIS_URL` guards `eprintln!` and `return`, so the integration tests report **passing** when the environment is absent — despite `apps/api/CLAUDE.md` stating otherwise. Not introduced by Plan A and not in its scope, but it is the mechanism by which a whole suite can silently not run. | Out of scope here; worth its own ticket. Recorded so it is not rediscovered a third time. | note only |
| 21 | T14 | **correctness (high)** | `shared/session/signOut.ts:46-54` | The sign-out transaction is **not failure-atomic**. Nothing between `bumpEpoch()` and `setSignedOut()` is guarded, so a rejection from `purgePersistedCache` (46) or `clearSession` (53) aborts `run()` **still holding the tokens** and leaves `status: "signedIn"`. `clearSession` is the likely one — `SecureStore.deleteItemAsync` rejects when the Keychain is unavailable, the same hazard `secure.ts:43` already concedes and try/catches on the read path. The person taps *Keluar*, nothing visibly happens, and the tokens stay on disk. | Wrap 25–51 in `try` and make the ending unconditional: `finally { await clearSession().catch(() => {}); setSignedOut(); }` — the same best-effort treatment the logout POST already gets, and what the spec's "must still be signed out" intends. | fix pass |
| 22 | T14 | **correctness (high)** | `shared/session/signOut.ts:19,46` | `sessionUserId()` is null on a **real** path, and the cache purge is skipped there. Cold start with a stored session → `useBootstrap` calls `fetchMe()` *before* `setSignedIn` → 401 → refresh refused → `client.ts:80` calls `signOut()` while `store.user` is still null → `purgePersistedCache` never runs → account A's garage stays on disk at `am.query.<A>` in **plain unencrypted MMKV** after A is signed out. Not the cross-account leak (the key is namespaced, so B cannot read it), but it is plate, VIN, and service-cost data surviving sign-out at rest — exactly the material the product rules protect. | MMKV exposes `getAllKeys()`, so a prefix purge of every `am.query.*` closes it in ~3 lines with no new state; call it unconditionally instead of behind `userId !== null`. **Traces to the plan (line 4241) and the spec** — correct both. | fix pass |
| 23 | T14 | **correctness (high)** | `shared/api/refresh.ts:81` | The epoch guards `apiRequest` and **nothing else**. A refresh completing *during* the sign-out transaction calls `writeSession` with a fresh pair, and if that lands after `clearSession()`, the device is signed out **carrying valid credentials** — which the next cold start restores straight back into the previous account. The window is the logout round trip plus teardown, and the logout POST and the refresh race each other on the server. `markRefreshPending` is already safe here (it re-reads and bails on null); `writeSession` is not. This is the "worse twin" ledger row 8 predicted. | Capture `currentEpoch()` at the top of `refresh.ts`'s `run()` and skip the write if it moved. No import cycle — `store.ts` imports only zustand. | fix pass |
| 24 | T14 | correctness | `shared/session/signOut.ts:28` | The best-effort logout has **no timeout and is awaited**. Swallowing a *rejection* does not cover a *hang*: on a captive portal iOS URLSession's default is ~60 s, and for that whole time `setSignedOut()` has not run, so the gates keep rendering the garage of the account that just pressed sign-out — and every failing request in `client.ts:80` awaits it too. | Folds into 21: capture the token, do the local teardown, fire the logout without awaiting (`void fetch(…).catch(() => {})`), or give it an abort signal. | fix pass |
| 25 | T14 | **gap, folded into T16** | plan T16 (`useBootstrap`) | `useBootstrap`'s `alive` flag guards unmount, not sign-out. If `signOut()` runs between `startPersistence` and `setSignedIn`, then `setSignedIn(user)` lands **after** `setSignedOut()` and resurrects `signedIn` with a live user object and an empty Keychain — a garage flash, then 401s until the next `signOut()` cleans up. | Capture `currentEpoch()` at the start of the bootstrap effect and skip `setSignedIn` if it moved. The epoch already exists for exactly this. **Folded into Task 16's brief**, alongside row 18. | T16 |
| 26 | T14 | **test-integrity — accepted residual risk, stated rather than implied** | `make mb-check` | The claimed verification is real but narrow. `mb-check` is `fmt-check → tsc --noEmit → expo lint`; there is no test runner, by design. It goes red on a `MeWire` field typo, an explicit `any`, or the reinstated `use*` hook name (the reviewer proved that one by running eslint against a probe file). It goes **green** on: `bumpEpoch()` moved to the end of `run()`, `setSignedIn` moved before `fetchMe`, `clearSession()` deleted outright, or `purgePersistedCache` never called. Every ordering property in this task's acceptance criteria — i.e. all of its risk — is verified by inspection and Task 17's manual pass and by nothing automatic, while Plans B/C/D will edit around this file with no regression net. | No cheap fix; recorded as accepted risk so it is a decision rather than an oversight. Revisit if Plan B or C touches the ordering. | note |
| 27 | T14 | hygiene | `shared/api/me.ts:15` | `fetchMe` casts the envelope unchecked (`envelope.data as T`) while both siblings narrow — `refresh.ts` has `narrowPair`, `secure.ts` narrows the stored JSON. A `/me` missing `has_vehicles` yields `hasVehicles: undefined` → `needsFirstVehicle` true → somebody trapped in the wizard forever; and `username: undefined !== null`, so `needsProfile` reads false. The server is ours and Task 17 would catch a live mismatch, so this is consistency more than exposure — but the route gates decide on exactly these three fields. | ~8 lines mirroring `narrowPair`. | fix pass |
| 28 | T14 | hygiene | `shared/session/signIn.ts:34-41` | The doc comment explains the ordering and not the failure. If `/me` fails offline **after** `writeSession`, `signIn` rejects with valid tokens already on disk; the next launch's bootstrap finds them, `refreshPending` is false, and the person is signed in without logging in again. That is the right behaviour — but Plan B's login `onError` needs to know it, because a retry mints a second server session that lives to TTL. | One sentence in the existing comment. Mention it in Plan B. | fix pass |
| 29 | T14 | hygiene | `shared/session/signOut.ts:28` | Third inline copy of the base URL; `client.ts:13` and `refresh.ts:9` both hoist it to a const. Functionally identical after babel inlining. | Hoist for consistency. | fix pass |
| 30 | T12 | hygiene, controller-side | plan `## Execution status` | Noted by T14's reviewer: the status block showed Tasks 9–15 unticked and the ledger empty while all seven had landed — so the resume map read as if nothing after Task 8 existed. This is the exact failure the status block is meant to prevent, and it was the controller's, not a writer's. | Ticked in the same turn the finding arrived. | closed |
| 31 | T2 | hygiene — **recommended NOT to apply** | `20260820132101_username_and_display_name.up.sql:17` | `display_name` has no database floor, unlike every other text column in this table: `users_email_shape` and `users_username_shape` both exist as "cheap sanity, not validation", while `display_name` — the only unbounded `TEXT` a user controls — gets nothing. Task 6's `MAX_DISPLAY_NAME = 60` at the application boundary is the only thing between the column and a multi-megabyte value. | `ADD CONSTRAINT users_display_name_shape CHECK (display_name IS NULL OR length(display_name) BETWEEN 1 AND 80)`. **The reviewer recommends against applying it**, and I agree: a second copy of the length is free to drift from `MAX_DISPLAY_NAME`, which is the same argument this migration uses to justify not duplicating the username rules. Recorded because the inconsistency is real, not because it blocks. Owner may overrule while the migration is unmerged. | decision, not fix |
| 32 | T2 | hygiene | `…username_and_display_name.down.sql:1-4` | The only down migration in the repo with no comment. `platform_role.down.sql` explains its drop order; `users.down.sql` explains why `citext` is not dropped. | One line saying the explicit index and constraint drops are belt-and-braces because `DROP COLUMN` cascades to both — so the next reader knows the order was reasoned rather than guessed. | fix pass |
| 33 | T7 | **environment fact, folded into T7's brief** | plan line 2848 | Not a defect — the plan already writes `username = $1::citext` — but it is the one line that would quietly undo Task 2's entire value if a writer "tidied" it. Without the cast PostgreSQL demotes the **column** to `text` (`citext → text` is implicit; `text → citext` is only an assignment cast), which turns the index seek into a sequential filter **and** makes the comparison case-sensitive, so the availability endpoint would answer "available" for a name that is taken. Proved by `EXPLAIN`: `Index Cond: (username = 'budi'::citext)` with the cast, `Filter: ((username)::text = …)` without. | Carry verbatim into Task 7's brief as an environment fact, matching `user_repo.rs:35` and `:263`. | T7 |
| 34 | — | note for a future ticket | — | Username rename does not exist in this plan — the frozen contract has `PATCH /me {display_name?}` only, there is no `set_username`, and no `UPDATE users SET username` anywhere. That is why Task 2 correctly ships **no** audit record. The moment a rename lands this reopens genuinely: a released `budi` reclaimed by a stranger silently repoints every existing `/@budi` link at a different person, which is an impersonation surface. `role_changes` is already this repo's worked example of the append-only-with-trigger shape for exactly that. | Out of scope here. Recorded so the rename ticket inherits the reasoning instead of rediscovering it. | note only |
| **35** | T12 | **correctness (CRITICAL — top of the fix pass)** | `shared/api/refresh.ts:38-40` | **`refreshPending` is read at cold start and NOWHERE on the in-session path — `run()` never looks at `stored.refreshPending`.** Sequence: access token expires → R1 fires with RT0 → server rotates RT0→RT1 → **response lost** → `fetch` rejects → `inFlight` nulled → `client.ts:80` calls `signOut()`, whose first awaits include an **untimed `fetch` to `/auth/logout` on the network that just failed** (ledger 24), so `clearSession()` has not run yet. Any second request 401ing in that window calls `ensureRefreshed()` → `run()` reads the still-present record → **POSTs `/auth/refresh` with the spent RT0** → `Rotation::Reused` → `revoke_all` → **every session on every device revoked.** Multiple concurrent 401s is the exact premise single-flight exists for, so the window is well-populated — and ledger 24 makes it wide. The pending marker exists precisely to prevent this and is not consulted where it matters. | One clause, reusing T16's own predicate: `if (stored === null \|\| stored.refreshPending) throw { kind: "unauthorized", … }`. No new machinery. **Must land together with 36.** **This is a defect in the plan's design, not the writer's transcription — correct the plan text too.** | **CLOSED** — see the note below the table |
| **36** | T12 | **correctness (high) — a stated acceptance criterion is FALSE as shipped** | `shared/api/client.ts:77-82` | The catch calls `signOut()` for **any** `ensureRefreshed()` rejection, **including `offlineError()`** — so `clearSession()` discards credentials on a transient network blip. Three documents say the opposite: the spec's session contract, `refresh.ts:60-63` ("the credentials are NOT discarded here"), and T16's own bootstrap note ("the tokens stay put when the cause was the network"). Somebody entering a tunnel while their access token happens to be expired is signed out and must re-enter a password. Task 12's acceptance criterion *"a network failure during refresh does not clear credentials"* is false, and nothing in T17 would have caught it. | `if ((err as ApiError).kind !== "offline") await signOut();` before the rethrow. **Only safe together with 35** — keeping credentials without the marker guard converts this into the replay of 35 on the next 401. | **CLOSED** — see the note below the table |
| 37 | T12 | correctness (moderate) | `shared/api/refresh.ts:81-85` | The same epoch defect as ledger 23, found independently by a second reviewer. **Honest narrowing the reviewer added:** the server's `ROTATE` Lua script already refuses a rotation whose `sess:` key is gone, and rotation preserves the session id, so a logout that *reached the server* kills the pair — the usual outcome is a wasted round trip and a welcome-screen flash. The security case is the flaky ordering where the rotation lands and the best-effort logout does not: sign-out did not sign out, and a live pair sits on the device. | As ledger 23. Two reviewers converging on it independently is the strongest signal in this ledger. | fix pass |
| 38 | T12 | correctness (low) | `shared/api/client.ts:73` | Refresh-and-retry fires on **every** path, `/auth/*` included. Plan B's login screen calls `apiRequest("/auth/login", …)` and the server returns 401 for a wrong password. With a stale record in storage — reachable once 36 is fixed and offline credentials survive — a mistyped password fires a spurious `POST /auth/refresh` with a possibly-spent token (35's hazard again) **and retries the login, costing two attempts against a limiter that counts before verifying the password.** | `&& !path.startsWith("/auth/")` in the condition. One clause; removes a surprise Plan B would otherwise inherit. | fix pass |
| 39 | T12 | correctness (low) | `shared/api/client.ts:67, 85-87` | Both catches swallow `AbortError` and report `kind: "offline"` — "Tidak ada koneksi." A screen aborting on unmount tells the person their network is down when they simply navigated away, and it makes `offline` mean two things against its own taxonomy entry. | `if (init.signal?.aborted === true) throw err;` at the head of each catch. | fix pass |
| 40 | T12 | correctness (low) | `shared/api/client.ts:100-107` | Two frozen-contract clauses unimplemented: a 2xx body carrying `error` is never inspected, and `{"data": null}` satisfies `"data" in body` so `null as T` is returned and a screen dies on `me.username`. **Neither is producible by this server** (`Wire` skips `None`, `ApiResponse` always sets `data: Some`); the trigger would be a proxy or CDN — the same middlebox `narrowEnvelope`'s own docstring cites. Deliberately **not** ranked structural: nothing in the typed surface moved. | One widened guard throwing `toApiError` when `error` is present and `serverError` otherwise. | fix pass |
| 41 | T12 | **test-integrity** | plan T12 AC / T17 matrix | Of six acceptance criteria: **one genuinely checkable** ("exactly one `POST /auth/refresh` for N concurrent 401s" — it goes red if `??=` becomes unconditional, if `ensureRefreshed` becomes `async` with an `await` before the assignment, or if the retry's 401 re-entered the refresh branch); **two unfalsifiable** (the `refreshPending` write ordering is unobservable from outside, and ledger 9 already records that a simulator Keychain item is not hand-inspectable); **one false and uncaught** (36). | Fold into ledger 9's proposed T17 row a *without-relaunch* variant: keep the app alive, restore the API, trigger a second request, assert **no second `POST /auth/refresh`**. That single row covers 35 and 36 together. | fix pass (plan edit) |
| 42 | T12 | hygiene ×3 | `client.ts:13,33`, `refresh.ts:9`, `signOut.ts:28` | (i) `process.env.EXPO_PUBLIC_API_URL ?? ""` written three times — the duplicated-literal rule the Rust gate states and the TS gate omits. (ii) An unset base URL yields `""`, so every request becomes a relative fetch that rejects and is reported forever as "Tidak ada koneksi" — a misconfigured build is indistinguishable from a dead network. (iii) `path` is interpolated with no encoding, and `GET /usernames/{username}/availability` is in the frozen contract; the server's canonicaliser is the authority so this is not a hole, but B/C/D will assume otherwise. | One exported `BASE_URL`; a dev-only throw or a `ponytail:` comment naming the empty-string ceiling; one doc line saying callers encode their own path segments. | fix pass |
| 43 | T15 | **correctness** | `shared/session/signIn.ts:34-41` (the defect is a design gap in T15's invariant) | The invariant "the active vehicle belongs to the signed-in account" has exactly **one** enforcement point, `signOut()` — and every route to a new session that bypasses it leaves the value stale. The concrete one is already written into the plan: Task 16's `useBootstrap` discards credentials on a `refreshPending` marker **without calling `signOut()`**, and its `catch` branch does the same on a failed `fetchMe()`; neither bumps the epoch, purges the cache, or clears the vehicle. So: A's app is killed mid-refresh → relaunch finds the marker, drops to welcome with A's id still in `am.client` → B signs in on the same device → `signIn()` touches none of it → **B's shell opens on A's vehicle id** and every query keyed on it 404s. Verbatim the failure `clearActiveVehicle`'s own doc comment says it exists to prevent. Same shape via an Android auto-backup restore, where MMKV survives but Keystore-backed SecureStore does not. | One line at the top of `signIn()`, before `writeSession`: `clearActiveVehicle();`. **This is the root-cause fix, not a per-branch patch** — `signIn()` is declared by the FROZEN CONTRACT as the only way a session starts, so one guard covers bootstrap's two branches, the backup-restore case, and a sign-out killed after `bumpEpoch()`. Patching `useBootstrap` needs two edits and still misses the other two. No cycle: `activeVehicle.ts` is a leaf. Accepted cost: a same-account re-auth via the `refreshPending` path loses the selected car and opens on the default instead of a 404 — the better of the two. | fix pass |
| 44 | T15 | **test-integrity** | plan T15 note / T17 Step 3 | Task 15 claims "verified in Task 17: set a value, force-quit, relaunch, confirm it survives; sign out and confirm it does not." **Task 17's table has no such row, and Plan A ships no way to set a value** — no Plan A file reads `useActiveVehicleId()`; the first consumer is Plan C/D. Task 17 would pass green with this module never exercised past its module-load `getString`. Its Step 4 cache-isolation check drives a *successful* `signOut()`, which exercises the one path that already works and none of the paths in row 43. | Either add the two rows with a temporary `setActiveVehicleId("test")` in the `(app)` placeholder (removed after), or move the verification honestly to Plan C's first task where a real consumer exists. **Do not leave the Task 15 note claiming a check that is not scheduled.** | fix pass (plan edit) |
| 45 | — | **note for Plan C/D — decide before a vehicle detail is ever cached** | `shared/api/queryClient.ts` | Surfaced by T15's reviewer while answering an unrelated lens, and it outlives Plan A. `am.query` is **plaintext MMKV** holding the whole persisted server-state cache. The project rule is that private vehicle data is filtered server-side but *is* legitimately served to its owner — so the first time a vehicle-detail response is cached, **plate and VIN land in plaintext on disk**. Not reachable today: Plan A caches nothing but `/me`. | A decision, not a fix: either an `encryptionKey` for `am.query` (whose key would itself have to live in SecureStore), or a `shouldDehydrateQuery` filter excluding vehicle-detail keys — which composes with ledger 15's mutation filter. **Owner's call, and it belongs before Plan C caches a garage.** | note, Plan C |
| 46 | fix of 7 | **correctness (low)** — raised from hygiene by the batch reviewer | `shared/session/secure.ts` (`markRefreshPending`) | **It also violates `ApiError.message`'s own written contract** at `errors.ts:20` — *"Already in Bahasa Indonesia and ready to show. Never a raw error string."* The moment a Plan B screen renders `err.message`, an Indonesian user reads `session unreadable; refresh not marked`. That is what moves it off the hygiene pile; the fix below already handles it. Original finding: | The fail-closed throw is a bare `Error`, while **every other exit from `refresh.ts` throws an `ApiError`-shaped object**. Functionally correct — `client.ts` tests `kind !== "offline"` and `undefined` is not `"offline"`, so `signOut()` runs as intended — but it is the odd one out, and a future reader adding a `switch (err.kind)` would silently not match it. Flagged by the writer who made the change rather than found later, which is the right instinct. | Throw `{ kind: "unauthorized", message: "Sesi tidak terbaca. Masuk lagi, ya." } satisfies ApiError` instead, so the taxonomy holds end to end. | fix pass |
| 47 | T4 | **test-integrity — the one worth working** | `tests/auth_flow.rs:541-543` | `a_throttled_login_says_how_long_to_wait` uses a **fresh `an_email()` every iteration**, so the account limiter never trips — its key is always newly created with TTL exactly 900. `refusal` therefore returns `max(ip_ttl≈898, 900) = 900` on every run. Both assertions (`wait > 0`, `wait <= 15*60`) are satisfied by the constant 900, so the test stays **green** under `retry_after_seconds: WINDOW.as_secs()` (ignoring the TTL entirely) **and** under `Some(by_ip.retry_after_seconds)` — *the exact oracle this task exists to prevent*. Only the unit test catches the latter; the integration test's own comment claims more than it verifies. | `assert_eq!(wait, WINDOW.as_secs());` — deterministic here, and it **would** fail on the oracle mutation because `ip_ttl` is ~898 by attempt 21. `WINDOW` is already `pub`. One line converts the aggregation rule from unit-tested-only to end-to-end-pinned. Also subsumes the hardcoded `15 * 60`. | fix pass |
| 48 | T4 | test-integrity | `tests/auth_flow.rs:577-578` | `a_successful_login_carries_no_retry_hint` **does not test its own name** — it asserts only that `error` is absent and the access token is a string, so adding `retry_after_seconds` to the *success* payload leaves it green. Today it duplicates "login succeeds". | `assert!(!body.to_string().contains("retry_after"));` | fix pass |
| 49 | T4 | hygiene | `adapter/redis/rate_limit.rs:126-134` | `allow` and `Attempt { pub retry_after_seconds }` are `pub` with **zero callers outside `allow_login`**. The aggregation discipline is therefore a convention rather than a type boundary — a future handler can call `allow` directly and publish one limiter's own TTL, which is precisely the oracle. | Drop `pub` from `fn allow` and `struct Attempt`; keep `LoginAttempt` public. **Deletes public surface instead of adding a guard** — the ponytail-correct fix. | fix pass |
| 50 | T4 | hygiene — **the pattern has already failed once** | `i18n.rs:203,230` + `errors.rs:245` | Three hand-maintained "every code" arrays, and **all three already omit `PartsDailyLimit`** — pre-existing, not caused by this diff. The mechanism meant to catch a forgotten code has itself forgotten one. | One `const ALL: [ErrorCode; N]` in `i18n.rs` consumed by all three tests — one place to remember instead of three. Add `PartsDailyLimit` while there. | fix pass |
| 51 | T4 | hygiene | `tests/auth_flow.rs:581-600` | `repeated_failures_from_one_address_are_throttled` is now **fully subsumed** by `a_throttled_login_says_how_long_to_wait` — same 40-attempt loop, same peer shape, strictly weaker assertion — and costs ~20 redundant argon2 verifications per suite run. | Delete it. | fix pass |
| 52 | T4 | **gap, out of scope — later ticket** | `adapter/http/auth.rs:216-220` | A limiter refusal emits **nothing** — no log line, no counter — so a sustained credential-stuffing campaign leaves no trace at all, while the adjacent `Rotation::Reused` path does emit `tracing::warn!` with `user_id`. The spec does not ask for this, so it is a gap rather than a defect. | One `tracing::info!` carrying the bare fact (and arguably the IP). **The global constraint forbids logging the email or its digest.** Later ticket. | note only |
| 53 | T4 | **accepted residual, recorded so it is a decision** | `adapter/redis/rate_limit.rs` (`refusal`) | The reviewer attacked the aggregate and it holds for the realistic cases, but one residual survives: an attacker who created their own IP key at a known moment knows their own `ip_ttl`, so any reported value *greater* than it is necessarily the account counter's remaining window — disclosing that **this address has been attempted recently**. It does **not** distinguish registered from unregistered (the account key counts `token_digest(email)` before any user lookup), and "which limiter refused" is already inferable from the attacker's own request count. The zero-leak alternative is reporting a constant `WINDOW`, which costs the countdown its meaning. Spec mandates `max`; implementation matches spec. | None. Recorded, not fixed. Named cost: a legitimate user behind NAT whose IP window trips at 30 s remaining is told to wait the account key's ~880 s — safe direction, annoying. | decision |
| 54 | fix of 22 | hygiene | `shared/api/queryClient.ts` | `purgePersistedCache(userId)` is still exported but now has **zero callers** — `purgeAllPersistedCache()` replaced it at the only call site. Its return type also changed from `Promise<void>` to `Promise<boolean>`, a breaking signature change with nobody to break. Confirmed by grep across `apps/mobile/src`. | Delete it — `ponytail` says an export kept for a caller that does not exist is dead flexibility. If a per-account purge is genuinely wanted later, it is three lines to write back. | fix pass |
| 55 | fix of 21 | correctness (low), pre-existing | `shared/api/client.ts:90` | Flagged by the writer who made the sign-out fix, and correctly **not** fixed by them since it sits outside their three files. `if ((err as ApiError).kind !== "offline") await signOut();` has no try/catch of its own, so if `signOut()` rejects, that `await` throws before reaching `throw err;` and `apiRequest` surfaces the raw purge error instead of the intended `ApiError`. **Pre-existing** — the old code had no guard around `purgePersistedCache`/`clearSession` either, so it could already reject the same way; the fix neither introduces nor worsens it. | `await signOut().catch(() => {});` — the caller wants the original error, not the teardown's. | fix pass |
| 56 | fix of 23/37 | **correctness (moderate) — and the controller's own note overclaimed it** | `shared/api/refresh.ts:43,107` + `shared/api/client.ts:73` | The epoch guard compares the refresh's **own** captured epoch against the current one, so it catches only a refresh **born before** `bumpEpoch()`. A refresh born *during* sign-out captures the already-bumped value and never trips. Reachable: `signOut()` bumps at line 17, then spends up to 5 s on the logout POST plus teardown before `clearSession()`. Any request 401ing in that window calls `ensureRefreshed()` — and **`client.ts:73` does not test the epoch before entering the refresh branch, only afterwards at :102.** The fresh `run()` captures the bumped epoch, `readSession()` still finds the record (not yet cleared), the guard passes, it fetches, and `currentEpoch() === epoch` at :107 — so `writeSession` lands. If `clearSession()` ran in between, **a fresh live pair sits on a device that just signed out, and the next cold start restores that account.** Fix 24 narrows the window from ~60 s to ~5 s; it does not close it. | One clause at `client.ts:73`: `if (response.status === 401 && stored !== null && currentEpoch() === epoch)` — an `apiRequest` already overtaken by a sign-out should not refresh at all. The genuine residual after that (a brand-new `apiRequest` issued mid-sign-out) is narrow enough to sit under ledger 8's accepted `ponytail:` ceiling — **but say so rather than implying it is closed.** | fix pass |
| 57 | controller | **process — the recipe was not corrected, only the instance** | plan lines 3121, 3214, 3220, 3233, 3940, 4056, 4265 | Only fixes 35 and 36 got their plan snippets updated. **Fixes 6, 7, 10, 15, 17, 21, 22, 24 and 43 did not.** Counted by the reviewer: `keychainAccessible` appears twice in the whole plan, both inside ledger prose, **never** in Task 9's code snippet; `purgeAllPersistedCache`, `dehydrateOptions`, and `AbortController` appear once each, all ledger-only. This matters because **Plans B, C and D are written against this document**: an author reading Task 9 at line 3220 sees `SecureStore.setItemAsync(KEY, …)` with no accessibility option and copies the ledger-6 catastrophe into any second secure key, and one reading line 3214 writes a `writeSession(value: StoredSession)` call that no longer compiles. Ledger 10 scoped itself as "two landed-file edits **plus two plan snippets**"; ledger 22 as "traces to the plan and the spec — correct both". **This is the controller's failure, not a writer's.** | Update the nine snippets in one pass, each with the same one-line `CORRECTED` comment already used for 35/36. **Also confirmed by the reviewer: the spec has zero hits for `keychainAccessible`, `WHEN_UNLOCKED`, `purgePersistedCache`, `purgeAllPersistedCache`, or `getAllKeys`** — so ledger 22's "correct the spec too" has nothing to correct; close that clause rather than leaving it open. | **CLOSED.** All nine corrected, plus 63 and 64, each carrying a `CORRECTED after Task N's review (ledger M)` comment in the ledger-35/36 voice. Spec grep re-run and confirmed: zero hits, nothing to correct there. **Two consequential fixes the writer had to make to keep the plan internally consistent**, which is the sign it read rather than pattern-matched: Task 12's `writeSession({..., refreshPending: false})` no longer compiles under the `Omit` signature, and Task 15's "wire it into sign-out" instruction still said "immediately before `clearSession()`", which stopped being true once ledger 21 introduced the `try`/`finally`. **And one deliberate deviation from the literal brief, correctly reasoned:** rather than inlining `clearActiveVehicle()` into Task 14's `signIn.ts` as instructed — which would make Task 14 reference a module Task 15 has not created yet, a real ordering break on a strict re-run — it added a **Task 15 Step 3**, mirroring the plan's own existing pattern for the identical situation in `signOut.ts`. The controller's instruction was the sloppier of the two. Task 12's own epoch-guard snippets (ledger 23/37/56) were outside its brief and were **fixed by the controller in the same turn**. |
| 58 | fix of 21/24 | hygiene | `shared/session/signOut.ts:39-40` | `new AbortController()` and `setTimeout` sit **above** the `try` that ledger 21 added. Ledger 21's invariant is "nothing between `bumpEpoch()` and `setSignedOut()` may abort `run()` early", and `new AbortController()` is the one line in that span not provably safe — it triggers RN's lazy `require` of the polyfill on first access. If it threw: epoch bumped, status still `signedIn`, tokens on disk. **Honest likelihood: very low** — Metro bundles the package, and a failure there means the app is already broken. Ironically the `AbortSignal.timeout` form would have been *inside* the try. | `let timer: ReturnType<typeof setTimeout> \| undefined;` before the try, construct both inside it, `clearTimeout(timer)` in the existing `finally`. | fix pass |
| 59 | fix of 15 | hygiene | `shared/api/queryClient.ts:87` | `shouldDehydrateMutation` is on the **call site** rather than on the client. `query-core/hydration.js:58` resolves `options.shouldDehydrateMutation ?? client.getDefaultOptions().dehydrate?.shouldDehydrateMutation ?? default`, so putting it on the `QueryClient`'s `defaultOptions.dehydrate` makes it a property of the client and covers any *future* `dehydrate`/persister call site instead of only this one. Same single line, strictly stronger placement for a guard whose failure mode is plaintext credentials on disk. What shipped is correct today. | Move it to `defaultOptions.dehydrate`. | fix pass |
| 60 | fix of 24 | hygiene | `shared/session/signOut.ts:42` | Fourth copy of `process.env.EXPO_PUBLIC_API_URL ?? ""`, and now the odd one out — `refresh.ts:10` and `client.ts:13` both hoist it to a `const BASE_URL`. Folds into ledger 42. | One line to match. | fix pass |
| 61 | — | **note, handed forward to T16** | `shared/session/signOut.ts:87` | The deliberate `clearSession().catch(() => {})` means a failed Keychain delete leaves the app showing `signedOut` **with tokens still on disk**. Correct for *this* session — staying `signedIn` is worse. But **the epoch is in memory and resets to 0 on relaunch, so the next cold start reads those tokens and signs the account straight back in.** Neither ledger 21 nor the code comment addresses the relaunch. Not a defect in this diff. | Task 16's bootstrap either closes it (retry the delete, or write a tombstone) or accepts it explicitly in writing. **Folded into Task 16's brief** alongside rows 18, 25 and 5. | T16 |
| 62 | — | **pre-existing, outlives Plan A — worth its own ticket** | `crates/runtime/tests/build_list_flow.rs:847` (`the_cursor_cannot_be_used_to_probe`) | **A security test that silently stops testing anything as the database fills up.** It asserts a stranger paging past a *real private* build id gets a byte-identical answer to paging past an id nobody issued — i.e. that the cursor cannot be used to probe for private content. The controller's re-run of Task 5's gates caught it failing: the two pages differed in one build's `modifications` array. **Investigated rather than retried.** It fails **in isolation** (so not concurrent interference) and it fails on a **clean `dev` worktree** (so not a Task 5 regression), and the suite went green immediately after `make db-drop && make db-seed`. Conclusion: the test is **not hermetic** — it depends on the shared development database holding little enough data, and **CI only passes because CI gets a fresh database every run.** So the probe-resistance property is unverified on any long-lived database, and nobody would notice. | Not Plan A's to fix. Its own ticket: make the assertion independent of accumulated rows — scope the listing to the fixture's own users, or assert on the specific ids the test created rather than on whole-page equality. **Do not "fix" it by resetting the database in CI; that hides it.** | note only |
| 63 | T5 | **plan defect — was generating real flaky failures** | plan's `a_username()` helper, nine call sites | The plan shipped `format!("u{}", Uuid::now_v7().simple())[..20]` — slicing the **first** 20 hex characters of a UUIDv7, which are **mostly the deterministic millisecond timestamp**. Almost no entropy survives, so concurrent tests generated colliding usernames: the writer reports `admin_flow.rs` failing 5–6 of 13 with `left: 409, right: 201`, reproduced three times. Fixed at all nine sites by slicing the high-entropy **suffix** instead: `format!("u{}", &Uuid::now_v7().simple().to_string()[13..])`, then verified with three clean runs. | Already applied in the code. **The plan text carries the defective form and must be corrected too**, or a re-run reintroduces a flaky-test generator. Handed to the plan-snippet correction pass alongside ledger 57. | fix pass (plan edit) |
| 64 | T5 | plan gap | plan Task 5, Step 7 | Task 5's file list named only `auth_flow.rs`, but making `username` required on `RegistrationRequest` breaks **every** caller of `/auth/register`. `grep -rl '"/auth/register"' tests/` found **eight more** test files — `admin_flow`, `build_flow`, `build_list_flow`, `catalog_flow`, `garage_flow`, `parts_flow`, `service_history_flow`, `service_summary_flow` — each with one register-helper call site. The writer found and patched all eight; the plan would have sent a re-run into eight compile errors it did not predict. | Add the eight files to Task 5's file list in the plan. **A required field added to a shared request type is never a one-file change** — worth stating as a general note, not just this instance. | fix pass (plan edit) |
| 65 | T5 | **test-integrity — folded into T6** | `tests/auth_flow.rs:694` | `a_username_is_canonicalised_before_it_is_stored` **does not test what its name says.** It never reads the stored value — it only proves `BUDI…` and `budi…` collide, which `citext` alone would produce even if the raw uppercase form had been stored. What actually makes it fail today is the `users_username_shape` CHECK **from Task 2**, not its own assertion: storing `BUDI…` raw raises 23514 → 500 → the first `assert_eq!(…, CREATED)` fires. So drop the `::text` cast from that constraint — the exact hazard the spec names — and this test goes green while the column holds `BUDI…`. | Task 6's `GET /me` is the first thing in the plan that can read a username back out, which makes it the cheapest place to pin this: register a non-canonical form, read it back, `assert_eq!(me["username"], canonical)`. **Sent to Task 6's writer while in flight.** `auth_flow.rs`'s `app!()` does not yield the pool, so asserting the row directly there is not available. | T6 |
| 66 | T5 | **test-integrity — non-hermetic, same family as 62** | `tests/auth_flow.rs:761` | `a_reserved_username_answers_exactly_like_a_taken_one` **stops testing anything after its first run.** Delete the `is_reserved` short-circuit at `http/auth.rs:227-229`: run 1 registers `admin` and the test fails once; **every run afterwards passes**, because `admin` is now genuinely taken in the shared development database. The reviewer verified the latent condition — `SELECT count(*) FROM users WHERE username='admin'` is `0` today against 307 rows carrying usernames — so the trap is armed and unsprung. | Asserting the 409 does **not** close it. Assert that no `users` row exists for the reserved name. | fix pass |
| 67 | T5 | **test-integrity** | `http/auth.rs:193` (`username_message`) | Five Bahasa Indonesia product strings with **zero coverage**. `a_malformed_username_is_a_field_level_validation_failure` asserts only `details["username"].is_string()`. Collapse all five arms to one string, or transpose `TooShort` and `TooLong`, and every test stays green while the product tells somebody who typed two characters *"Maksimal 30 karakter."* | One `#[test]` in the existing `mod tests` asserting the five variants map to five distinct expected strings. **The reviewer's pick for the one finding not to ship without.** | fix pass |
| 68 | T5 | hygiene — **correct the comment, not the code** | `http/auth.rs:227` | The reserved-name short-circuit returns **before** `security::hash_password`, whose own doc puts argon2id at the OWASP floor and ~20 ms. So a reserved name answers in well under a millisecond while a taken or available one costs argon2 plus a database round trip: byte-identical bodies, ~20× apart in latency. This repo's own `CLAUDE.md` treats exactly this shape as a defect, and the code comment claims the identical response *is* the defence. **Severity genuinely low** — what leaks is membership of a 12-entry public constant of common words, and Task 7's availability endpoint is the *designed* way to learn the same thing, where reserved and taken are equally cheap. | Fix the comment, not the code: name the timing gap and why it is acceptable here, so the next reader does not mistake the claim for complete. | fix pass |
| 69 | T5 | hygiene | `apps/api/README.md:52` | The repository's own API contract table still reads `POST /auth/register \| 201 \| email is free, password ≥ 8 characters` — it now omits that a username is required, well-formed, free, and not reserved. | Update the row. | fix pass |
| 70 | T5 | hygiene | `i18n.rs:160` (`ErrorCode::Conflict`) | The 409's **top-level** `message` is *"Data ini sudah berubah. Muat ulang, lalu coba lagi."* — right for an optimistic-concurrency conflict, nonsense for "this email is already registered". The useful text lives only in `details`. Harmless while Plan B renders `details` as the contract says, but any generic toast falling back to `error.message` tells somebody whose address is taken to reload the page. **Same family as ledger 11**, which is the client half of this. | Either a registration-specific code, or make Plan B's 409 branch prefer the first `details` value — which is exactly ledger 11's proposed fix, so one change closes both. | fix pass |
| 71 | — | **contract note for Plan B — not a defect** | `http/auth.rs` | Two facts Plan B must not assume away. **(a)** `/auth/register` returns **at most one key** in `error.details`, ever: password shape is checked first, then username shape, then reserved, then the database — and when email *and* username both collide, **Postgres deterministically reports `users_email_key`** (index OID order, verified). The FROZEN CONTRACT types `fields?: Record<string, string>` as a map, so `errors.ts` must not assume it enumerates every invalid field, and the client must not infer the other field is free. **(b)** A **missing** `username` hits axum's default `JsonRejection` — a 422 with a **plain-text body, no envelope, no `details`** — which Plan B's parser cannot read as an envelope. Pre-existing (a missing `password` behaves identically today), not a T5 regression. | Carry both into Plan B. (b) may deserve its own ticket: a custom `JsonRejection` handler would put every 422 in the envelope. | note, Plan B |
| 72 | T5 | **correction to ledger 63's stated reason** — the fix was right, the explanation was not | plan `a_username()` | Ledger 63 said the old `[..20]` slice failed because the leading hex is "mostly the deterministic millisecond timestamp, leaving almost no entropy". **That is not the mechanism.** In `uuid 1.24.1`, `now_v7()` uses `ContextV7`: a 42-bit counter reseeded with 41 random bits **once per millisecond**, then incremented within it. So the old slice held `u` + timestamp (12 hex) + version + the counter's **top ~22 bits** — which are *constant for every UUID minted in the same millisecond by the same process*. The failure was not thin entropy in general but **zero variation within a millisecond**, which is exactly why it was reproducible inside one test binary and why `admin_flow` lost 5–6 of 13. The new `[13..]` keeps all 42 counter bits (strictly incrementing within a millisecond) plus 32 random tail bits ≈ **74 bits**, and the counter alone guarantees intra-process uniqueness. Length is `u` + 19 = 20 chars, inside 3–30; `simple()` emits lowercase hex so the shape rule holds, and no `u`+19-hex string can be reserved. | No code change — the fix is correct. **Correct the reasoning in the plan comment**, so a future reader does not "improve" the slice back toward the front believing entropy is the only axis. | fix pass (plan edit) |
| 73 | T6 | **test-integrity — found by the writer, in its own red phase** | `tests/profile_flow.rs` (`me_never_returns_the_password_hash`) | The test **passed before a single line of the endpoint existed.** It asserts only that the response body lacks the substrings `"argon2"` and `"password"` — and a 404 error body lacks them just as thoroughly as a correct 200 does. So a test named for the product's most sensitive leak was, at that moment, verifying that a route which did not exist did not leak. It now exercises a real 200, confirmed by the writer and re-confirmed by the controller's gate run. | Already exercising a live response. **The general lesson is the one to keep:** an assertion phrased as *absence* passes trivially against an error response, so any absence test must first pin the status code it is asserting absence within. The reviewer has been asked to apply the same suspicion to the other five tests in the file. | closed by T6 |
| 74 | — | **test-integrity — the accepted residual is now COMPOUNDING, and this is the recommendation to act on** | `client.ts`, `refresh.ts`, `secure.ts`, `signOut.ts` | Ledger 26 accepted "no runnable check" as a residual scoped to **Task 14**. After two fix batches it covers four files and the entire session layer, and **every one of the sixteen fixes across both batches is silently revertible with both gates green** — `mb-check` is `expo customize → tsc --noEmit → expo lint`, with no test runner by design. The two that carry the real risk, the refresh-branch ordering (56) and the sign-out failure atomicity (21), are **ordering properties, which is exactly what a type checker cannot see.** Plans B, C and D will edit around all four files with no net. The layer's failure mode is every session on every device being revoked. | The batch reviewer's own words: *one runnable check with a fake `currentEpoch`/`readSession`/`fetch` and three assertions would go red on the fix-1 revert **and** the fix-6 revert both — the cheapest single thing to buy before Plan B starts.* **Dispatched rather than deferred**, because deferring it past Plan B is what makes it permanent. Precedent for a runner exists in the repo: `packages/tokens` runs `node:test` under Bun. | **IN PROGRESS** |
| 75 | fix batch 2 | hygiene | `client.ts:77-80` | The comment conflates two different guards: "the epoch check further below" is `client.ts:124`, which guards the *apiRequest*, while the refresh's own guard is `refresh.ts:106`, in another file. A reader chasing that sentence looks in the wrong place. The reviewer also found a **narrowing the comment does not claim and could**: `run()` re-reads storage, so the hazard additionally requires that read to beat `clearSession()` — if clearing already happened, it throws and nothing is written. The dangerous window is therefore smaller than the 5 s the comment implies. | Name `refresh.ts:106` explicitly and add the narrowing. **Note the direction: the residual is narrower than we documented, not wider.** | fix pass |
| 76 | fix batch 2 | hygiene | `queryClient.ts:123-127` | Ledger 17's **second clause never landed.** `grep -rni throttl apps/mobile/src` returns nothing. The `ponytail:` comment names the O(n) key scan but not the persister's ~1000 ms uncancellable throttled write — which is what makes the docstring's "Remove every persisted query cache" not-quite-true: `stopPersistence()` drops the subscription only, so a save scheduled by `queryClient.clear()` can rewrite the key ~1 s after the purge. Impact stays small for ledger 17's own reason (post-`clear()` the payload is an empty cache, and `buster`/`maxAge` gate it). Deleting `purgePersistedCache` was the last chance to notice. | One sentence appended to the existing `ponytail:` block. | fix pass |
| 77 | fix batch 2 | hygiene — **leave as a decision, not an oversight** | `refresh.ts:57,95` vs `errors.ts:43` | `refresh.ts:95` is character-for-character the `UNAUTHORIZED` constant at `errors.ts:43`; `:57` is its prefix. Fix 2 added a fifth session-expiry string beside them without folding any — the same rule ledger 42 applied to `BASE_URL`. The const is module-private, so the duplication is structural rather than lazy. | The reviewer would leave it, and so would I — **but recorded so it is a decision rather than something nobody noticed.** | decision |
| 78 | T6 | **test-integrity (high) — the most valuable finding in this review** | `tests/profile_flow.rs`; `user_repo.rs` (`profile_of`) | **`has_vehicles === true` is never exercised anywhere in the repo.** `/me` appears in no other test file, and no test in `profile_flow.rs` creates a vehicle — only the `false` half is asserted. **One-line mutation that leaves all 18 suites green:** replace `EXISTS(SELECT 1 FROM vehicles v WHERE v.owner_id = u.id) AS "has_vehicles!"` with `FALSE AS "has_vehicles!"`. And `has_vehicles` is the **sole** input to Plan D's onboarding gate — plan line 4752 is literally `return !user.hasVehicles;` — so that mutation traps **every user who already owns a car permanently inside the first-vehicle wizard**, with CI green. The reviewer confirmed against live data that the query itself is correct, so this is a missing test, not a wrong query. | One test, ~15 lines: register → `POST /vehicles` (body shape is in `garage_flow.rs`) → `GET /me` → assert `has_vehicles == true`. **Close it together with ledger 27**, which is the same defect from the client end — `fetchMe` casting the envelope unchecked, so a missing `has_vehicles` yields `hasVehicles: undefined` and springs the same wizard trap. One resolution, not two half ones. | fix pass |
| 79 | T6 | **test-integrity** | `tests/profile_flow.rs:231-249` | **The vacuous test the writer found is still vacuous.** The discovery was correct and reported honestly — but the *fix* was never applied. `me_never_returns_the_password_hash` reads the body without ever asserting the status, so it passes today only because the route happens to return 200; **any** change making `/me` answer 401, 404 or 500 leaves it green, since neither an empty body nor an error envelope contains `"argon2"` or `"password"`. Substring-absence is also the wrong assertion for a carve-out endpoint: it catches an argon2 hash and nothing else — not a bcrypt hash, not a session token, not `platform_role`, not `created_at`. | Two lines: `assert_eq!(response.status(), StatusCode::OK);`, then assert `body["data"]` has **exactly** those five keys. That converts it from "lacks two magic strings" to "is the contract" — which is what Plans B, C and D actually need pinned. | fix pass |
| 80 | T6 | **correctness — empirically demonstrated, not argued** | `adapter/http/profile.rs:60-77` (`check()`) | A display name can be **invisible, multi-line, or bidi-spoofed**. The migration deliberately ships no CHECK on the column, so `check()` is the only guard, and `trim()` strips `char::is_whitespace` only. The reviewer compiled the exact predicate and ran it: 60× U+200B zero-width space → `Ok(60)`; U+FEFF BOM → `Ok(1)`; embedded newlines → `Ok(14)`; `"Budi\u{202E}Santoso"` (RTL override) → `Ok(12)`; embedded NUL → `Ok(5)`. **Concrete failure:** `display_name` is one of the two facts Plan D's onboarding gate reads (`user.displayName === null`), so sixty zero-width spaces is non-null, satisfies the gate, and renders as *nothing* in every byline — a person who looks anonymous but is past onboarding, with no way for the client to tell. The RTL override reverses the rest of a rendered string: the classic display-name spoof. | Two lines in `check()` after the trim, killing all six cases with one predicate and no new dependency: `if trimmed.chars().any(char::is_control) \|\| !trimmed.chars().any(char::is_alphanumeric) { return Err(field_error("display_name", "Nama harus berisi huruf atau angka.")); }` | fix pass |
| 81 | T6 | hygiene — **decision, recorded so it is visible rather than accidental** | `adapter/http/profile.rs:69` | The length bound is unstable across Unicode normalisation forms. Measured: 60 NFC e-acute → `Ok(60)`; the same name in NFD (e + U+0301) → 120 chars → refused; a nine-glyph family ZWJ emoji → 63 chars → refused. `chars()` counts scalar values, not grapheme clusters, so the same visual name is accepted or refused depending on the keyboard's normalisation form. | **`chars()` is the right unit here and should not change.** Indonesian orthography is Latin and unaccented, and every iOS and Android keyboard emits NFC by default, so the NFD path is essentially unreachable for this audience; the byte ceiling is bounded at 240 either way. The honest fix if it ever matters is `unicode-normalization` + NFC before counting — adding that dependency today for a case Indonesian names do not produce is over-building. The unit test pinning 60/61 exactly is what stops the constant drifting. | decision |
| 82 | T6 | hygiene | `tests/profile_flow.rs:214-215` | No test pins that `/me` answers the **caller's** row — only `id.is_string()` and `email.is_string()`, neither compared to the account that registered. Ownership is structurally sound (the id comes from the authenticated session; the DTO has one field, so mass assignment is closed by construction), and ledger 65's assertion incidentally catches a wholesale wrong-row swap. | ~8 lines: register A and B, read `/me` with A's token, assert A's id. **It is the assertion that would survive a future refactor adding a `?user_id=` parameter "for admin convenience"** — which is exactly the kind of change that looks harmless in review. | fix pass |
| 83 | — | **note for T16's reviewer** | `usecase/profile.rs` (`ProfileError::NotFound`) | `NotFound` maps to **401**, so a session outliving its account triggers the client's unauthorized path. That is the right choice — it signs out rather than showing an error. But it means Task 16 must confirm the single-flight refresh has a **retry cap**: a 401 that survives a *successful* refresh must not loop. Unreachable today, since no account-deletion endpoint exists. | Note, not a finding. **Folded into Task 16's brief** alongside rows 5, 18, 25 and 61. | T16 |
| 74b | — | **ledger 74 CLOSED — the net exists. But read the caveat, it is the whole point.** | `apps/mobile/test/session.test.ts` (new), `apps/mobile/package.json`, `Makefile` | Four `bun:test` assertions, faking `readSession`/`currentEpoch`/`fetch` via `mock.module`. **Each was verified by the reverse of TDD: delete the guard, watch it go red, restore it, watch it go green** — (1) deleting `&& currentEpoch() === epoch` from `client.ts:89` made `apiRequest` answer `kind: "offline"` from the extra refresh fetch the guard normally prevents; (2) deleting `\|\| stored.refreshPending` from `refresh.ts:56` hit the mock's "unexpected fetch" throw; (3) flattening `signOut.ts`'s `try`/`finally` left `setSignedOutCalls` at 0 instead of 1. A fourth pins single-flight. Controller re-ran: `make mb-test` → **4 pass, 0 fail**, `fmt-check` and `mb-check` both EXIT=0. No production file was reshaped to make it testable; no new runtime dependency (only `bun-types` as a devDependency). | **`mb-test` is a SEPARATE Make target and is deliberately NOT in `mb-check` or the aggregate `check`** — the writer's call, and the right one to surface rather than take: folding a brand-new runner into a gate CI already trusts changes what "green" means, and that is the owner's decision. **The consequence is that the net does not currently run in CI, so it protects nothing until it is wired.** That is precisely the shape ledger 74 was raised about, one level up. **A decision the owner owes.** — **ANSWERED, 2026-08-20: wire it into `mb-check` and CI.** Done by the controller inline: `mb-check` now runs format → type-check/lint → tests, and `.github/workflows/mobile.yml` gained a **Session tests** step. Per §31, the CI job lands in the **same change** rather than as a follow-up ticket — a gate that exists only in a plan is not a gate. Both carry a comment saying *why* they are there: the layer's guards are ordering properties, which `tsc --noEmit` and `expo lint` cannot see at all, and before this ran in the gate all sixteen hardening fixes could be deleted one line at a time with CI green. Composition confirmed by `make -n mb-check` (dry run, because Task 16's writer is editing those files live). | **CLOSED** |
| 84 | T7 | **correctness — the client half must land BEFORE Plan B consumes it** | `adapter/redis/rate_limit.rs:56` (`PER_IP_LOOKUP = 60` / 15 min) | **The doc comment reasons about one person trying a dozen names — right reasoning, wrong denominator, on an Indonesia-first product.** Indonesian mobile carriers run heavy CGNAT: thousands of Telkomsel / Indosat / XL subscribers share one public address. Concrete failure: two dozen people on one carrier NAT open the register screen in the same 15-minute window, and the live-validation field starts 429ing users who did nothing wrong. **And the limit is not buying anti-enumeration anyway** — the reviewer costed it from the attacker's side: 240/hour/IP means a 10k wordlist is ~42 h from one address but **~25 minutes from a 100-address proxy pool**. So it pays a real availability cost for a benefit it does not deliver; it is a *cost bound* on Redis round-trips, which it does well, and should not be described as an anti-enumeration control. | Two halves, and **the second is the one that matters**: (1) raise `PER_IP_LOOKUP` substantially (300+) — one index probe per request, the cost bound still holds. **Owner's judgement, not a defect.** (2) **Pin the client contract now:** a `429` or `5xx` from this endpoint must render as *"tidak bisa diperiksa"*, **never** as *"sudah dipakai"*, and must never block submit — the server-side `409` on register is the real guard. Getting this backwards turns a shared-NAT throttle into "that name is taken" shown against a **free** name. | fix pass — **(2) into Plan B** |
| 85 | T7 | **test-integrity — structurally untestable as written, and the plan is complicit** | `tests/profile_flow.rs:337-347` | **Delete the entire `allow_lookup` block from the handler and all seven tests still pass.** It is the only security control on the endpoint. Worse than untested: the `availability()` helper calls `a_peer()` **inside itself**, so every request in every test comes from a fresh random address and no counter ever accumulates — even `every_reserved_name_answers_unavailable`'s twelve calls. **The plan asked for the assertion and never wrote it:** its Acceptance Criteria says *"Repeated lookups from one address are eventually refused with a 429"* while Step 1 lists no such test, deferring it to a manual curl in Step 7. That is the recipe, not the writer. | The exact template is two files over at `auth_flow.rs:527`. Hoist the peer to a parameter, then `let peer = a_peer(); for _ in 0..70 { availability_from(&app, &a_username(), peer).await }`. Assert the 429 carries `error.code == "too_many_requests"` **and no `details`** — the inverse of login's assertion. **That asymmetry is the contract and nothing pins it today.** Fix the plan's Step 1 too. | fix pass, **first** |
| 86 | T7 | hygiene — **and the spec is the more important half** | `adapter/http/auth.rs:193,206` | `check_username` and `username_message` were widened to `pub(crate)`, one notch wider than needed — they are consumed only by `crate::adapter::http::profile`, a sibling, which `pub(super)` reaches exactly. `pub(crate)` additionally exposes them to `usecase/`, `adapter/postgres/` and `platform/`. **The concrete risk that opens:** a use case calling `check_username` and receiving an **`ApiError`** — an HTTP type in the application layer, a layering inversion `make be-boundary` cannot catch, because both live in `runtime`. `apps/api/CLAUDE.md` asks directly for the narrowest form. **The plan specified `pub(crate)`, so the writer complied** — fix the plan text alongside the code. | `pub(super)`. | fix pass |
| 87 | T7 | **documentation defect — in the SPEC, not just the plan** | spec §Username; plan Task 7 AC | Both still say **thirteen** reserved names; `RESERVED` is `[&str; 12]`. The reviewer verified *why* twelve is right rather than just counting: `GET /usernames/me/availability` answers `422 {"username":"Minimal 3 karakter."}`, because `MIN_LEN = 3` makes `me` unclaimable **before `is_reserved` is ever reached** — so listing it would be dead weight *and* would break the `every_reserved_name_is_itself_a_valid_username` invariant at `username.rs:220`. This is the same defect the controller found and fixed in Task 3; it survived in two documents. | Correct both. **The spec matters more than the plan** — it is the document the next reader trusts, and it outlives this plan. | **CLOSED** — both corrected by the controller in the same turn the finding arrived. The spec now carries the *reason* twelve is right, not just the count, so the next reader cannot "helpfully" add `me` back. The plan's AC now says twelve **and instructs iterating the constant rather than restating a count**, which is what stops the number drifting a third time. |
| 88 | T7 | test-integrity (minor) | `tests/profile_flow.rs:417-420, 480` | Two narrowness notes. `a_reserved_name_answers_byte_for_byte_like_a_taken_one` **compares no headers**, so a future handler adding e.g. `x-reserved: true` slips past — the property holds today (verified live: 144/144 bytes, identical header set) but the test name claims more than it checks. And `availability_never_mentions_an_email_or_an_account` is a **substring blocklist**, so it would pass a handler leaking a field not on its five-item list. | Compare the header maps minus `x-request-id`; assert the key set is exactly `{meta, data}` and `data`'s exactly `{available}`. | fix pass |
| 89 | T7 | **correction to the writer's self-assessment — recorded because the nuance is the point** | `tests/profile_flow.rs` | The writer audited its own seven tests and reported four as satisfiable by a naive stub. **The count was exactly right; the attribution was not.** Too harsh: it credited only two tests with forcing the real `taken \|\| reserved` logic, when three do — `a_taken_username_is_unavailable` forces the taken half, `every_reserved_name_answers_unavailable` forces the reserved half, and `availability_ignores_case_the_way_the_column_does` forces case folding. Too generous: `a_reserved_name_answers_byte_for_byte_like_a_taken_one` forces **nothing on its own** — it passes under *both* constants, since both sides move together. Its unique contribution is the indistinguishability of the two answers, which nothing else pins, and it only becomes discriminating alongside the free-name test. | No change. **Net: no test is vacuous, none is redundant, and the suite as a whole does force the real logic.** Only the attribution needed correcting. | closed |
| 90 | — | **evidence-path note, worth keeping** | — | The writer's curl proof was taken against port **8080** — which on this machine is occupied by **an unrelated Micronaut service** (its 404 body is `{"_links":…,"_embedded":…}`). The reviewer re-ran everything on `BIND_ADDR=127.0.0.1:8099`. The writer's numbers happen to be right, but its evidence path was not. | Nothing to fix in the code. **The lesson generalises:** a curl against a port you did not start is not evidence about your service. Worth stating in the environment card for any later task that verifies by HTTP. | note only |
| 91 | T16 | **correctness — an unreadable cache signs out a VALID session** | `shared/bootstrap.ts:63-72` | `await startPersistence(user.id)` sits inside the `try` whose `catch` calls `setSignedOut()`. **Verified against the installed package**, not assumed: `persistQueryClientRestore` calls `removeClient()` and then **rethrows**, and `persistQueryClient` wraps it as `restore().then(subscribe)`, so the rejection reaches the await. Scenario: the app is killed while `persistQueryClientSave` is mid-write (it writes on every cache event), leaving truncated JSON at `am.query.<id>`. Next cold start: tokens valid, `/me` returns 200 — **and the person lands on the welcome screen anyway.** It self-heals next launch (the persister deleted the bad key), but they were spuriously logged out once, and signing back in mints a fresh server session. The bare `catch {}` makes it undiagnosable. | `await startPersistence(user.id).catch(() => {});` — with the ceiling named: `.then(subscribe)` never runs on that path, so nothing persists for the rest of that session. Acceptable; the next launch re-arms it. | fix pass |
| 92 | T16 | **correctness — the same await can fail a sign-in** | `shared/session/signIn.ts:67` | A restore rejection makes `signIn` reject **after** `writeSession` already put a live pair on disk and `/me` already succeeded, so `status` never flips. The function's own doc comment already warns that a caller retrying *"mints a second server session that lives to the refresh token's full TTL"* — but it names only step 2, `fetchMe`. This is a **third** rejection point it does not cover. **Persisting is a cache optimisation and must never be able to fail a sign-in.** | Same one-liner, plus extend the doc comment to name this third point. | fix pass |
| 93 | T16 | **correctness — contradicts a decision the plan already made** | `shared/bootstrap.ts:63` | `fetchMe()` is unbounded, and `_layout.tsx:58` returns `null` until `done`, which is set only after that await — so **a captive portal or a hung API leaves the app looking frozen on the splash for iOS URLSession's full ~60 s default**, with no recovery. The plan already ruled this exact thing unacceptable at the sibling call site: ledger 24 bounded sign-out's logout POST to 5 s **in these words** — *"a captive portal held this open for as long as iOS URLSession's own default (~60s)"*. The same hazard, on the more visible surface, was left unbounded. | The `AbortController` + `setTimeout` shape `signOut.ts:50-63` already uses (the plan documents why `AbortSignal.timeout` is unusable in RN), plumbed through an optional `signal` on `fetchMe` — additive, breaks no contract. An aborted fetch lands in `apiRequest`'s `catch → offlineError()` → bootstrap's catch → `setSignedOut()`: welcome screen, **tokens kept**, next launch signs back in. Correct outcome. | fix pass |
| 94 | T16 | **correctness — private vehicle data left at rest** | `shared/bootstrap.ts:58-60` | The `refreshPending` discard branch calls `clearSession()` and `setSignedOut()` but **not** `purgeAllPersistedCache()`. That branch is a definitive credential discard — the event `signOut` treats as requiring a purge, for the reason ledger 22 recorded verbatim: *"the account's garage — plate, VIN, service cost — stayed on disk in plain unencrypted MMKV."* Here it stays **indefinitely**: nothing later removes it, since another account's `startPersistence` writes a different key and only `signOut` sweeps the prefix. Not a cross-account bleed (persistence is keyed per account), but a **data-at-rest exposure of exactly the fields the project rules say cannot be recalled.** | `purgeAllPersistedCache()` beside `clearSession()`. Accepted cost is the one the plan already took on this same path for `clearActiveVehicle` (ledger 43): a same-account re-auth loses its warm cache. | fix pass |
| 95 | T16 | **test-integrity — and one item is aimed at Plan C** | `apps/mobile/test/` | `mb-test`'s four tests cover `client.ts`, `refresh.ts` and `signOut.ts`; **none touches Task 16.** `tsc` and `expo lint` see none of the following, so all of it can be deleted or broken silently: **(a) removing `<AppGate>` from `(app)/_layout.tsx`** — the exact defect the component-not-inline design exists to prevent, **and Plan C is instructed to rewrite that file's body**; (b) inverting or dropping any gate predicate; (c) removing `if (!ready) return null` from `_layout.tsx`, reintroducing the pre-gate frame; **(d) adding a new screen at `src/app/foo.tsx` instead of `src/app/(app)/foo.tsx`** — ungated, and nothing anywhere catches it; (e) the epoch guard; (f) the splash-holds-until-session wiring. | (a) and (d) are the two worth a cheap net **before Plan C**: a render test asserting each layout's tree contains its gate, and a check that every route file outside `catalog.tsx`/`index.tsx` lives under a group. | fix pass |
| 96 | T16 | hygiene — **plan text, not a writer slip** | `shared/index.ts:13` | The barrel exports `fetchMe`, which is **not in the FROZEN CONTRACT**, and Step 1b's justification for widening the barrel names only `useBootstrap` and `queryClient`. A Plan B/C/D screen calling `fetchMe()` gets a `Me` **without updating the store** — precisely the bug `refreshMe` exists to prevent. **The plan's own barrel snippet contains it**, so correct the plan too. | Drop it from the barrel and from the plan snippet. | fix pass |
| 97 | T16 | hygiene | `shared/session/store.ts:52-58` | `useSession`'s doc explains why two selectors are used but never states the consequence — **the returned object is a new identity every render**. The plan handed this forward as "a trap for its consumer" (ledger 5), and the barrel is the only place Plans B/C/D will look. | One sentence closes it permanently. | fix pass |
| 98 | T16 | hygiene — **plan disagrees with itself** | plan lines 4779 vs 5015 | Line 4779 says "three placeholder `index.tsx` screens"; Step 3 at line 5015 says `(app)/index.tsx` is **not** a placeholder. The writer followed the specific instruction and shipped two placeholders, correctly. **My own reviewer brief inherited the wrong number**, which is how a stale line propagates. | Correct line 4779. | **CLOSED.** The writer fixed the Facts bullet and **deliberately did not widen its edit** to Step 3's heading, which the ledger row had not named — then flagged it rather than leaving it silent. Correct discipline; the controller swept the heading in the same turn. Both now say two placeholders plus the real `(app)` screen. | |
| 99 | T16 | **residual, traced and unreachable — do NOT fix** | `shared/bootstrap.ts` | `setDone(true)` is unconditional while `setSignedIn` is epoch-guarded, so an epoch mismatch would release the splash with `status` still `"loading"` → `index.tsx` renders `null` → blank ground for up to ~5 s. **Unreachable today:** the only `signOut()` that can run during the bootstrap window is the one `client.ts:112` fires on bootstrap's own `fetchMe`, and that call is awaited, so `fetchMe` cannot reject until `setSignedOut()` has already run. No UI is mounted, so the *Keluar* button cannot reach it. | None. **Recorded because it becomes live the moment anything calls `signOut()` without awaiting it.** | note only |
| 100 | T16 | **strength worth recording, not a finding** | `shared/index.ts` | `setSignedIn` / `setSignedOut` / `setUser` are **not** barrel-exported, so Plans B/C/D cannot forge a `Me` — the only exported ways to move session state are `signIn`, `signOut`, `refreshMe`, all server-derived. And `me` is never in the query cache, so the plaintext MMKV store cannot influence a redirect. **The gate's input is structurally server-owned.** | Nothing. Recorded so a later refactor does not widen the barrel without knowing what that buys today. | note |
| **101** | branch gate | **high** — found only by reading the whole diff at once | `shared/api/refresh.ts:87-97, 106` | **`clearSession()` is the one storage write in that file with no epoch guard, and its harm runs backwards.** The epoch check sits at line 106, *after* the failure branch has already returned, so the failure branch never reaches it. Ledger 23/37/56 closed this for `writeSession`; nobody checked `clearSession`. A **stale** refresh, born long before a sign-out, reaches in and wipes the Keychain record a **later** sign-in wrote. One account, one device: (1) access token expires, the refresh POST hangs; (2) user taps *Keluar* — epoch bumped, teardown completes; (3) user signs back in, `signIn()` writes a **fresh live pair**, app working normally; (4) at ~60 s the hung refresh answers 401 (its token was revoked at step 2) → `pair === null` → `clearSession()` **deletes step 3's pair** → `client.ts:112` signs out the new session. **Somebody who just signed in is silently signed out with their credentials wiped.** Reachability depends on ledger 102, which widens the window from ~300 ms to ~60 s. | **Deletes code rather than adding it:** move the check from 106 to just after 85, before the `if (pair === null)` block — it then guards both the clear and the write — and **delete the one at 106**, which it subsumes. `markRefreshPending()` is the same class but its window is two adjacent awaits; traced and left with a comment, in ledger 99's style. | fix in flight |
| **102** | branch gate | **medium** — ledger 93's fix does not do what ledger 93 says it does | `shared/bootstrap.ts:86-90` × `client.ts:89-92` × `refresh.ts:72-76` | `useBootstrap` wraps `fetchMe(controller.signal)` in a 5 s abort, added specifically so a captive portal cannot freeze the splash for iOS URLSession's ~60 s default. **The signal reaches `apiRequest`'s first `send()` and stops there** — it never reaches `ensureRefreshed()`, and `refresh.ts:72`'s own fetch takes **no signal and no timeout at all**. And this is the **ordinary** cold start, not an edge: `ACCESS_TTL` is one hour, so any launch more than an hour later 401s and refreshes. `/me` 401s fast → the refresh hangs unbounded → the 5 s timer fires against nothing → `done` stays false → `_layout.tsx:58` returns `null` for ~60 s. **Frozen splash: the exact symptom ledger 93 was written to remove.** Ledger 93 was reviewed against `fetchMe`; `refresh.ts` was reviewed as a single-flight; **the gap is between them**, which is why no per-task review saw it. | Bound `refresh.ts`'s own fetch at **10 s**, with the `AbortController` pattern already written twice here. **One bound for all callers, not a per-caller signal** — it is a shared single-flight promise, so a per-caller abort would let one caller's unmount cancel everybody's refresh. | fix in flight |
| **103** | branch gate | **medium** — and the spec asserts the opposite | `adapter/http/auth.rs:193-244`; spec §Username | **`POST /auth/register` has no rate limiter.** There are exactly two limiter call sites in the codebase — `allow_login` and `allow_lookup` — and no global layer; `mod.rs:104` applies only `request_id::middleware`. **The enumeration half is NOT the finding** — the spec deliberately makes register's field-named 409 confirm email existence and Plan B carries that email across to login. What is unexamined is that nothing bounds the **rate**, turning three individually-accepted behaviours into a scriptable one: **argon2 runs before the uniqueness check** (`usecase/auth.rs:57`), so every probe including every destined-to-409 one costs 19 MiB and ~20 ms — *verbatim the cost profile `rate_limit.rs`'s own module doc gives as the reason login needed a limiter*; there is no email verification (deliberately AM-53's), so every 201 in a sweep is a real account on somebody else's address **and now burns their chosen username too**; and it leaves no trace (ledger 52's shape). **The spec sentence "The availability endpoint is rate-limited like any other unauthenticated endpoint" is false as shipped** — availability was the only unauthenticated endpoint that was. | `allow_register(ip)` collapsing one `allow()` to a bool exactly as `allow_lookup` does, called **before `check_password_shape`** so a throttled probe pays nothing. Limit chosen with Indonesian carrier CGNAT in mind (ledger 84's argument), with a test matching `repeated_lookups_from_one_address_are_eventually_refused`. **Correct the spec sentence in the same pass** — make it true rather than deleting it. | fix in flight |
| **104** | branch gate | **delivery note — would redden CI on its own** | `apps/api/.sqlx/` | The cache has **four untracked query files and one deleted**. `backend.yml:106` runs `cargo sqlx prepare --check --workspace` with `SQLX_OFFLINE: "true"`, so a commit that misses the untracked entries reddens the backend **lint, test *and* sqlx** steps. **Untracked files are exactly what a `git add -u`-shaped commit drops.** | Name it explicitly in the `haiku` commit brief: stage `apps/api/.sqlx/` deliberately, including the untracked entries and the deletion. | carry to commit |
| **105** | fix of 88 | **test-integrity — a test that fails against CORRECT code, caught by the final gate** | `tests/profile_flow.rs` (`headers_without_request_id`) | Ledger 88's header comparison included **`content-length`, which is not stable**. The envelope carries an RFC3339 `meta.timestamp` whose fractional seconds serialise with **trailing zeros trimmed**, so the same body is 143 or 142 bytes depending on the microsecond it was produced in. Measured directly against a running server: eight identical requests returned `143 143 143 143 143 143 143` and one **`142`**. The final `make check` went red on `a_reserved_name_answers_byte_for_byte_like_a_taken_one` — and **the code was correct**: both responses said `available: false`; one timestamp was a character shorter. **Diagnosed rather than retried:** the endpoint was probed live on a freshly built binary (`admin` → `false`, a free name → `true`, both correct) before anything was changed. | Exclude `content-length` alongside `x-request-id`. **Nothing is lost** — the indistinguishability property is bodies being identical, which the caller already asserts separately and exactly; this helper exists for the *other* half, that no future handler adds a distinguishing header like `x-reserved: true`, and that still holds. **A test that cries wolf gets ignored, and this one guards the endpoint's central safety property** — the same disease Task 16's fix batch caught in its own test. | **CLOSED** |
| 5 | T16 | hazard, handed forward | — | `useSession()` returns a new object identity every render (correctly — the two-selector form avoids React's *"getSnapshot should be cached"* infinite-loop guard). A gate written as `const s = useSession(); useEffect(..., [s])` re-fires that effect on **every** render. | Destructure and depend on the fields: `const { status, user } = useSession()`. Not a defect in T10 — a trap for its consumer. | T16 + its reviewer |

### Which ledger rows were closed during the run, and which still wait

Closed early: **12** (by Task 4, though not as its brief specified — see the Task 4 status entry),
**35** and **36** (below), **42, 46, 54, 55, 58, 59, 60** and **56 in part** (second batch — see its
own section), and **6, 7, 10, 15, 17, 21, 22, 23, 24, 28, 37, 43** — the session-layer
hardening, applied by two concurrent writers on disjoint files while the ready-queue was empty.
Gates re-run by the controller on the final tree: `make fmt-check` EXIT=0, `make mb-check` EXIT=0.
An independent `opus` reviewer is reading the whole batch. Everything else waits for the fix pass.

**The catch worth recording, because it is the kind that looks right in a diff and does nothing.**
Fix 24 wanted a timeout on the best-effort logout POST. The obvious write is
`AbortSignal.timeout(5000)` — and it would have been **silently inert here**. React Native
polyfills the global `AbortSignal` from the `abort-controller` npm package (via
`react-native/Libraries/Core/setUpXHR.js`), and that polyfill has **no static `timeout()` method**.
So the call would throw synchronously while building the fetch options, land inside the existing
`try`, be swallowed by the empty `catch {}` that makes the logout best-effort — and no timeout would
apply at all, while the diff read as if one did. The writer checked the package before writing and
hand-built `new AbortController()` + `setTimeout` with `clearTimeout` in a `finally` instead, at a
5 s budget. The reviewer has been asked to verify that claim against `node_modules` rather than
accept it.

The five session-layer fixes in writer 1's two files:

- **6** — `keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY` on both `setItemAsync`
  calls. The default is included in an encrypted device backup and restored onto other hardware;
  `_THIS_DEVICE_ONLY` ties the item to this device's Secure Enclave key so it is simply absent from
  the restore. Tier deliberately kept at `WHEN_UNLOCKED`, not `AFTER_FIRST_UNLOCK` — Task 12
  established there is no background refresh.
- **7** — `markRefreshPending()` now fails closed. `readSession` keeps its own null-conflation,
  which is correct for a read; the write path no longer shares it.
- **10** — `writeSession` narrowed to `Omit<StoredSession, "refreshPending">`. **This is what put
  the gate red at `signIn.ts:37`**, exactly as the brief predicted, and the fix belongs to the
  concurrent writer that owns that file.
- **23 / 37** — the epoch is captured before any await in `refresh.ts`'s `run()` and re-checked
  immediately before `writeSession`.

  **CORRECTED — this note previously claimed the pair "cannot" be resurrected, and that was
  overclaiming.** An independent reviewer caught it (ledger 56). The guard compares the refresh's
  *own* captured epoch against the current one, so it catches only a refresh **born before**
  `bumpEpoch()`. A refresh born *during* the sign-out captures the already-bumped value and never
  trips — and `client.ts` does not test the epoch *before* entering the refresh branch, only after.
  Fix 24's 5 s bound narrows the window from ~60 s to ~5 s plus teardown; it does not close it.
  **Half of ledger 8's "worse twin" survives, and it is tracked as ledger 56.**

**The trace that shows fix 7 does what it claims**, since nothing here is assertable from a type
check: `run()` reads the session and passes its guard; `markRefreshPending()` then does its own
*second, later* Keychain read; if the screen locks in that gap, that read rejects, `readSession`
returns null, and `markRefreshPending` throws **before the `fetch` fires** — so the refresh token is
never presented to the server. The throw propagates to `client.ts`, whose `kind !== "offline"` test
treats a bare `Error` as not-offline, so `signOut()` runs. **This device asks for a password and no
other device is touched**, because reuse detection never had anything to detect.

One inconsistency the writer flagged and correctly did not fix: that throw is a bare `Error` rather
than the `ApiError`-shaped throw every other exit from `refresh.ts` uses. Functionally right —
`kind` is `undefined`, which is not `"offline"` — but it is the odd one out. Ledger it as hygiene.

### Second fix batch — ledger 42, 46, 54, 55, 56, 58, 59, 60, all CLOSED

Applied while Task 6 held the backend chain and the mobile queue was empty. Controller re-ran and
re-read: `make fmt-check` EXIT=0, `make mb-check` EXIT=0 (all three stages — `expo customize`,
`tsc --noEmit`, `expo lint` — chained with `&&`, so lint genuinely ran), the guard is present at
`client.ts:89`, the Bahasa Indonesia message at `secure.ts:124`, `purgePersistedCache` has **zero**
references anywhere under `apps/mobile/src`, and `shared/api/baseUrl.ts` exists.

**Ledger 56 is closed for the case it was raised for, and the remainder is stated rather than
implied.** The guard is now `response.status === 401 && stored !== null && currentEpoch() === epoch`,
where `epoch` is the value `apiRequest` captured **before** the bump — so a request that predates
the sign-out can no longer enter the refresh branch at all, and the whole write-then-race-with-clear
chain never starts. **What survives:** a *brand-new* `apiRequest` begun during the sign-out window
captures its own epoch at its own top, which already equals the bumped value, so `currentEpoch() ===
epoch` is true for it too and it can still race `clearSession()`. That is narrow, it is written into
the code comment rather than left to be rediscovered, and it is accepted under ledger 8's `ponytail:`
ceiling. **Do not read row 56 as fully closed.**

**Ledger 46's behavioural equivalence was verified rather than assumed**, which mattered because
the whole point was to change a thrown value that `client.ts` branches on. Before: a bare `Error`,
so `(err as ApiError).kind` reads `undefined` at runtime — a type-only cast has no runtime effect —
and `undefined !== "offline"` is true, so `signOut()` runs. After: `kind` is `"unauthorized"`, still
not `"offline"`, so `signOut()` runs. Identical behaviour; what changed is that an Indonesian user
can no longer be shown `session unreadable; refresh not marked`.

**One honest note from the writer, worth keeping.** The house comment form is
`CORRECTED after Task N's review (ledger M)`, but these rows came from a **batch** review rather
than any one task's writer. Instead of inventing a task number to match the pattern, it wrote
`CORRECTED after the fix-batch review (ledger N)`. That is the right call — the comment form exists
to tell a future reader where a decision came from, and a fabricated task number would have made it
lie in exactly the way it is meant to prevent.

### Task 16's fix batch — ledger 91–98, all CLOSED

Controller re-ran: `make fmt-check` EXIT=0, `make mb-check` EXIT=0, **`make mb-test` now 6 pass /
0 fail** (up from 4), and `apps/api/` untouched by this writer throughout.

**Ledger 95's two tests were shippable, but not the way the brief imagined — and the writer found
out by probing rather than guessing.** Importing `expo-router` under `bun:test` **fails before any
test body runs**: React Native's own entry file uses Flow syntax (`import typeof * as
ReactNativePublicAPI from './index.js.flow'`) that bun's transpiler rejects outright. So there is no
renderer available here, and the brief's fallback ("say so and ship the second alone") was not
needed — both properties are checkable without one:

- **each group layout wraps its `<Stack>` in the matching gate** — read as text, checking the gate's
  open and close tags bracket the `<Stack` usage. Reverse loop: stripped `<AppGate>` from a backup
  copy → red → restored, `diff` byte-identical → green.
- **every route file outside `catalog.tsx` and the entry `index.tsx` lives inside a route group** —
  a directory walk. Reverse loop: created `src/app/foo.tsx` → red → deleted → green.

**And it caught a false positive in its own test before shipping it.** The first pass searched for
`<Stack` from the start of the file — but every layout carries a comment above the gate reading
*"Plan C replaces the `<Stack>` body…"*, which contains that same literal. The test went red against
**correct** code. Fixed by searching from the gate's opening tag onward. A test that fails on good
code trains people to ignore it, which is the same disease as one that passes on bad code.

### The backend fix pass — ledger 47–51, 66–69, 78–80, 82, 85, 86, 88, all CLOSED

Controller re-ran all five gates on the restored tree: `be-fmt`, `be-lint`, `be-boundary`,
`be-sqlx-check`, `be-test` — every one EXIT=0, 18 suites. No SQL changed, which
`be-sqlx-check` independently confirms.

**The fix specified for ledger 80 did not work, and the writer proved it instead of shipping it.**
The reviewer proposed — and the controller's brief passed along verbatim — this predicate:

```rust
if trimmed.chars().any(char::is_control) || !trimmed.chars().any(char::is_alphanumeric) { … }
```

`char::is_control()` covers Unicode category **Cc only**. **U+202E, the right-to-left override, is
category Cf.** So `"Budi\u{202E}Santoso"` contains real letters (passing the `is_alphanumeric` half)
and no Cc characters (passing the `is_control` half) and goes straight through — **the single most
exploitable of the six cases the finding was raised about.** The writer discovered it by writing all
six named test cases and running them, confirmed `'\u{202E}'.is_control() == false` against two
independent sources, and added an explicit `is_invisible_or_bidi_control` over the Cf ranges that
matter (ZWSP/ZWNJ/ZWJ, LRM/RLM, LRE/RLE/PDF/LRO/RLO, the isolates, BOM) — no new dependency, which
keeps ledger 81's ruling against `unicode-normalization` intact.

**Two reviewers and the controller all signed off on a predicate that did not close the defect it
was written for.** The thing that caught it was executing the six cases rather than reasoning about
them. Worth remembering the next time a fix looks obviously right.

**Mutation verification was done for nearly every test**, and one result is worth keeping. On ledger
66, removing the `is_reserved` short-circuit fails run 1 on `409 vs 201` — and on **run 2**, with
`admin` now genuinely registered in the shared database, the *old* assertions pass by coincidence
(`409 == 409`) while the *new* database-row check correctly fails. That is the "worse than untested"
scenario the ledger predicted, reproduced on demand.

**One thing was NOT mutation-verified and the writer said so** rather than letting the report imply
otherwise: ledger 88's header-comparison half, which would have needed `ApiResponse` restructured to
attach a synthetic conditional header. The test is written, compiles and passes; only the
adversarial round-trip was skipped, on a row the ledger itself marks minor.

**Environment note for the card:** a bare `cargo test` or `cargo clippy` **without**
`SQLX_OFFLINE=true` hits a pre-existing live-schema mismatch in `vehicle_repo.rs` / `build_repo.rs`
(nullable-column drift) against the shared dev database. Irrelevant to the real gate — `make be-*`
exports `SQLX_OFFLINE=true` — but it will mislead anyone who invokes cargo directly.

### Ledger consolidation — run after Task 17, and it found a real gap in this ledger's own bookkeeping

A consolidation pass over all 102 rows flagged **three rows whose closure this ledger never
recorded**: 18, 61 and 65. Each had been "folded into Task N's brief" or "sent to Task N's writer
while in flight", each writer reported it done — and **the ledger row was never updated**, so a
reader working only from this table would have counted three open items that are not open.

**The controller verified all three against the code**, which is the point of the distinction:

| Row | Claim | Verified at |
|---|---|---|
| 18 | `signIn` never calls `startPersistence` | **CLOSED** — `signIn.ts:72`, `await startPersistence(user.id).catch(() => {})`, with the ledger-92 catch already applied |
| 61 | the failed-Keychain-delete acceptance must be written down | **CLOSED** — `bootstrap.ts:33-35`, the acceptance is in the doc comment, in the terms the row demanded |
| 65 | the canonicalisation test is pinned by another task's constraint | **CLOSED** — `profile_flow.rs:197,217`, registers `"  {UPPERCASE}  "` and asserts `body["data"]["username"]` reads back canonical |

**The lesson is about this document, not the code.** A finding folded into a *later task's brief*
leaves the ledger, and nothing brings it back — the writer reports to the controller, the controller
reports to the owner, and the row sits there looking open forever. Rows handed forward need their
closure written back the same way a deferred row does. Three of them survived this run; on a longer
one they would have accumulated.

**One row the consolidation correctly refused to treat as closed, and so should any reader:
row 56.** Its own text says *"Do NOT read row 56 as fully closed."* The epoch guard stops a refresh
born **before** `bumpEpoch()`; a brand-new `apiRequest` begun *during* the sign-out window still
captures the already-bumped epoch and is not caught. That residual is documented in the code and
accepted under ledger 8's `ponytail:` ceiling — it is not eliminated.

**And the honest headline the consolidation produced, which belongs in the completion report:** the
process caught its **own** wrong fix (row 80's `is_control()` gap — Cc only, while U+202E is Cf),
its own **unapplied** fix (row 79 — the vacuous test row 73 found was reported and then never
actually fixed until a second reviewer caught it), its own **overclaim** (row 56's "cannot be
resurrected", walked back), and its own **stale cross-reference** (row 98). That self-correction is
the evidence the 17/17 is worth something. It is also why the count of closed rows is the least
interesting number in this document.

### The honest risk surface: which of these fixes could be silently reverted

The reviewer was asked which fixes could be undone, one line at a time, with **every gate still
green** — and answered it with evidence, running the repo's own `tsc@5.9.3` against an isolated
probe rather than asserting. This extends ledger 26 and 41's accepted residual risk from Task 12 to
the whole fix batch, and it is the thing a completion report should not omit.

| Fix | Silently revertible? |
|---|---|
| 6 — `keychainAccessible` ×2 | **yes** — the option object is optional |
| 7 — the fail-closed null guard | **yes** — probe confirmed `{...possiblyNull}` raises no error |
| **10 — `Omit<StoredSession, …>`** | **no** — probe: `TS2345, 'refreshPending' is missing`. **The only fix the type system holds.** |
| 15 — `dehydrateOptions` | yes |
| 17 — `Promise<boolean>` | yes (zero callers) |
| 21 — the `try`/`finally` | yes |
| 22 — the unconditional sweep | yes |
| 23/37 — the epoch check | partly: deleting only the `if` leaves `epoch` unused → eslint red; deleting both lines → green |
| 24 — the 5 s abort bound | partly: same shape (unused `controller`/`timer`) |
| 35 — the `stored.refreshPending` clause | **yes** |
| 43 — `clearActiveVehicle()` in `signIn` | partly (unused import) |
| — `bumpEpoch()` moved off line 17 | **yes** |

**Ten of twelve are silently revertible, and the two "partly" rows fall to green with a two-line
deletion.** This is not an argument that the fixes are wrong — it is a statement of what `mb-check`
can and cannot defend, on a layer whose failure mode is every session on every device being revoked.
The layer has no test runner by design, so the guard is code review and Task 17's live pass, and
nothing else. Say that plainly rather than letting eleven green gates imply more.

### Ledger 35 + 36 — pulled forward out of the fix pass, and why

**Closed during the run rather than deferred, as one change across two files.** Both guards are in
the source and both gates re-run by the controller: `make fmt-check` EXIT=0, `make mb-check` EXIT=0.

Three reasons, and the first is the one that makes it legitimate rather than a process deviation:

1. **The ready-queue was empty.** Tasks 5, 6, and 7 are serial behind Task 4 on
   `adapter/http/auth.rs` and the shared `.sqlx` cache; Tasks 16 and 17 need a live `GET /me`.
   The fix pass is deferred so it does not interrupt work in flight — there was none to interrupt.
2. **Task 16 builds directly on both files.** Leaving a known replay path in `refresh.ts` while
   writing the bootstrap that consults the same marker would have meant writing T16 against code
   already known to be wrong — the same reasoning the structural carve-out uses.
3. **They are one change, not two.** Applying 36 alone widens 35 from a race window into the
   ordinary path.

**The trace that shows the two guards compose** (read-through, since `mb-check` has no runner —
ledger 41 records that as accepted residual risk):

> Request A 401s → `run()` passes the guard (`refreshPending` false) → `markRefreshPending()`
> persists the marker **before** the network call → `fetch` rejects offline → `run()` throws
> `offlineError()` → `inFlight` nulls → **`client.ts` skips `signOut()` because the kind is
> `offline`, so the credentials survive (36 closed).** Request B 401s immediately after → `inFlight`
> is null so a fresh `run()` starts → it reads the same record, `refreshPending` is **true**, and
> the widened guard throws **before any fetch** — RT0 is never resent, so `revoke_all` is never
> triggered by this path (35 closed). B's error is `unauthorized`, not `offline`, so `signOut()`
> does run and the person is routed to sign-in through the normal path. The client fails closed
> locally instead of failing open against the server.

**No separate reviewer was dispatched for this fix, and that is a decision, not an omission.** The
change is the verbatim application of a prescription an independent `opus` reviewer wrote, and the
controller re-ran both gates and re-read both guards in the source. **It is folded into Task 16's
review** — T16 imports and builds on both files, so its reviewer reads them anyway — and the
branch-level `security-review` in the finishing sequence covers the whole diff. Recorded here so a
completion report can say plainly which changes had an independent second read and which did not.

### Plan corrections made during execution

These are defects in the plan text itself, fixed at the source so a re-run does not reintroduce them. Recorded because "correct the recipe, not only the instance" is the rule.

| Task | What the plan said | What is true |
|---|---|---|
| T3 | `RESERVED` held 13 entries including `"me"` | `"me"` is two characters and `MIN_LEN` is 3, so `canonicalise("me")` returns `Err(TooShort)` — failing the plan's **own** test that every reserved name is a valid username. Dropped to 12. Nothing is lost: the length floor already makes `"me"` untypeable, so `/@me` can never collide with an account. Found by TDD going red for the right reason. |
| T9 | `writeSession` was `JSON.stringify(value)` | That writes whatever `refreshPending` the caller passed, contradicting both the function's own doc comment and its acceptance criterion ("always lands `refreshPending: false`"). Corrected in the plan to spread-and-override. |
| T13 | MMKV used as `new MMKV({ id })` and `storage.delete(key)` | `react-native-mmkv@4.3.2` is a Nitro rewrite: `MMKV` is a **type-only** export, instances come from `createMMKV(config)`, and the removal method is `remove(key)` — `delete` does not exist on the interface. Three call sites corrected by the writer. |
| T1 | Step 2 predicted the red would land on the final assertion (logout returning 200 without revoking) | It landed on the **first** logout call with a 422, because the old handler still demanded `Json<RefreshRequest>` and posting `{}` fails deserialisation before the handler body runs. The test is right; the prediction was wrong. Verified empirically rather than trusted. |
| T8 | — | **`make mb-run-dev` needs a UTF-8 locale.** `pod install` fails, and the visible error is CocoaPods crashing *inside its own error reporter* (`Encoding::CompatibilityError`), which hides the cause printed one line above: "CocoaPods requires your terminal to be using UTF-8 encoding." `LANG`/`LC_ALL` are empty in a non-interactive shell here. Run `LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8 make mb-run-dev p=ios`. |
| — | — | **A false green the controller nearly reported.** The first dev-client rebuild was read as exit 0 when `make` had exited 1 — the command ended in a pipe to `tail`, and the exit code belonged to `tail`. Read the command's own exit code, never the tail of its log. |
