# Plan B — Auth screens Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `executing-plans-hybrid` to run this plan
> task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the three screens that get a person into an account — welcome, register, login —
plus the sign-out trigger, built entirely from the AM-15 design system and typed against the
session contract Plan A establishes.

**Architecture:** Three route files under `app/(auth)/` (the group and its layout come from Plan A),
backed by one feature folder `src/features/auth/` holding the request functions, the zod schemas,
and four small hooks. No screen touches secure storage, the query cache, or the router's auth gate —
those belong to Plan A, and this plan only calls into them.

**Tech Stack:** Expo SDK 57 · expo-router (typed routes) · React Native 0.86 · TypeScript strict ·
TanStack Query + zustand (installed by Plan A) · **zod, installed by this plan's Task 1** · the AM-15 `Am*` primitives.

**Spec:** [`docs/superpowers/specs/2026-08-20-am-17-auth-session-onboarding-design.md`](../specs/2026-08-20-am-17-auth-session-onboarding-design.md)

**Closes:** AM-50, AM-51, AM-57, AM-59, AM-60, AM-61.

**Plan of four.** Plan A (session foundation) lands before task 1 of this plan starts. Plan C (app
shell) and Plan D (onboarding) follow. Where this plan hands off to a screen that does not exist
yet, it says so rather than building a stub.

---

## Global Constraints

Every task's requirements implicitly include this section.

- **No new design values.** No hex, font size, spacing, radius, or duration literal anywhere in this
  plan's code. Everything through `useTheme()`. `theme.space` is `{1:4, 2:8, 3:12, 4:16, 5:20, 6:24,
  8:32, 10:40, 12:48, 16:64, 20:80, 24:96}`, `theme.radius` is `{xs:6, sm:8, md:12, lg:16, xl:20,
  "2xl":28, pill:999}`, `theme.touchTargetMin` is `44`, `theme.pagePadding` is `16`. Referring to a
  number here is fine; writing one into a component is not.
- **A missing primitive is a finding, not a licence to inline.** Where the design system genuinely
  lacks something, this plan either extends the primitive (Task 1's `AmTextField` props) or builds a
  local component from theme tokens only, and records the gap in the ledger.
- **Product strings are Bahasa Indonesia.** Code, comments, commit messages, and this plan are
  English.
- **≥44pt touch targets, enforced by the primitive.** `AmButton` puts `minHeight`/`minWidth` *after*
  the caller's `style` precisely so a caller cannot defeat it. Never pass a `height` that fights it.
- **Never `allowFontScaling={false}`.** Large system text must reflow.
- **Never distinguish an unknown email from a wrong password** — not in copy, not in a hint, not in
  a log, not in analytics, not in a "did you mean to register?" nudge on the login screen.
- **Exactly one thing is installed: `zod`, by Task 1** (`bun add --cwd apps/mobile zod`). `bun.lock`
  and `apps/mobile/package.json` change once, in that task, and **nowhere else**. An earlier draft of
  this line read "Nothing new is installed; `bun.lock` must be unchanged", which contradicts this
  plan's own "Dependency ownership" section below and would make Task 1's own gate look like a
  violation. `react-hook-form` and `@hookform/resolvers` remain uninstalled by anyone.
- **No social login, no email verification, no password reset, no biometrics.** AM-52, AM-53, AM-54,
  AM-77 keep those.
- **The client never decides a username is acceptable.** It mirrors the regex for instant feedback;
  the server's canonicaliser is the authority.

---

## The frozen contract from Plan A

Code against this. Do not re-derive it, do not re-implement any part of it.

```
POST /auth/register   {email, username, password}   -> 201 {access_token, refresh_token, token_type, expires_in}
POST /auth/login      {email, password}             -> 200 (same)
POST /auth/logout     (Authorization only, no body) -> 200 {signed_out: true}
GET  /me                                            -> 200 {id, email, username, display_name, has_vehicles}
GET  /usernames/{username}/availability             -> 200 {available: bool}
429 on login: error.details carries {retry_after_seconds: <number>}
```

```ts
export interface Me { id: string; email: string; username: string | null; displayName: string | null; hasVehicles: boolean }
export type ApiErrorKind = "offline" | "validation" | "rateLimited" | "unauthorized" | "server";
export interface ApiError { kind: ApiErrorKind; message: string; fields?: Record<string, string>; retryAfterSeconds?: number }
export function apiRequest<T>(path: string, init?: { method?: string; body?: unknown; signal?: AbortSignal }): Promise<T>;
export type SessionStatus = "loading" | "signedOut" | "signedIn";
export function useSession(): { status: SessionStatus; user: Me | null };
export function signOut(): Promise<void>;
```

Plan A also delivers the `(auth)` route group and its layout, TanStack Query, zustand, and the
error taxonomy.

**Dependency ownership, corrected 2026-08-20 after all four plans were written.** An earlier draft
of this section said Plan A installs `react-hook-form` and `zod`. It does not, and it is right not
to: Plan A renders no input, and its only runtime parsing is one envelope narrowing. So —

- **`zod` is installed by THIS plan, in Task 1** (`bun add --cwd apps/mobile zod`). Plan D runs
  after this one and inherits it.
- **`react-hook-form` is not installed at all**, by anyone. This plan decided against it (see the
  deviation note below), and no later plan asked for it. `@hookform/resolvers` is a separate
  package again and is likewise absent.

A writer who finds `zod` already present should not re-install it; a writer who finds it missing
in any task after Task 1 has found a real defect, not a setup step.

### The three gaps are CLOSED — verified against Plan A's shipped code, 2026-08-21

The controller read Plan A's merged code before task 1 was dispatched. All three gaps below are
answered. **Task 1 confirms these file:line facts; it does not re-investigate them, and none of the
fallbacks below is needed.**

| Gap | Answer | Evidence |
|---|---|---|
| CG-1 | `signIn(tokens: { access_token: string; refresh_token: string; expires_in: number }): Promise<void>` — exported from `@/shared`. Takes the wire shape **snake_case, and `expires_in` is required**, so hand it the parsed response object as-is. | `apps/mobile/src/shared/session/signIn.ts:62`, `apps/mobile/src/shared/index.ts` |
| CG-2 | **No new property, and no `code` field.** The backend answers a taken email or username with 409 + `details: {email\|username: "<pesan>"}`, and Plan A's `toApiError` already routes *a 409 that names a field* to `kind: "validation"` with `fields` populated. `registerConflictOf` therefore reads `error.fields`. | backend `apps/api/crates/runtime/src/adapter/http/auth.rs:370-372` via `ApiError::conflict_on`; client `apps/mobile/src/shared/api/errors.ts`, the `status === 409 && fields !== undefined` branch |
| CG-3 | `AuthGate` renders `<Redirect>` the moment `status === "signedIn"`, and `app/(auth)/_layout.tsx` already wraps its `<Stack>` in it. Nothing to add. | `apps/mobile/src/shared/gates.tsx`, `apps/mobile/src/app/(auth)/_layout.tsx` |

**Where AuthGate actually sends somebody, which changes what "success" looks like on these screens.**
It is not one destination. `needsProfile(user) || needsFirstVehicle(user)` sends them to
`/(onboarding)`; only a complete profile with a vehicle reaches `/(app)`. A newly registered account
has `display_name === null`, so **registration always lands on the onboarding group** — whose screen
is still Plan D's placeholder. That is correct and expected. Do not "fix" it by redirecting
somewhere else, and do not build a stub to make it look finished.

**A 401 on the login screen is a wrong password, NOT an expired session.** The backend answers bad
credentials with 401 + "Email atau password salah." (`i18n.rs:176`), and `toApiError` maps every 401
to `kind: "unauthorized"`. On `login.tsx` that kind is an ordinary inline credentials error. Never
treat it as a session-expiry signal, never call `signOut()` on it, never redirect on it.
`apiRequest` will not attempt a refresh for it either — its refresh branch requires a stored
session, and there is none while signed out.

---

### Three gaps in that contract, and what Task 1 does about each

These are not re-derivations — they are things the frozen contract does not say, and each one
blocks a screen. **Task 1 resolves all three by reading Plan A's shipped code first.** If Plan A
resolved one differently, adopt Plan A's answer and record the correction in this file; the
fallbacks below exist so no task is blocked waiting for a decision.

**CG-1 — there is no way to hand a token pair to the session layer.** `useSession()` reads, and
`signOut()` ends, but nothing in the frozen contract starts a session from a `TokensResponse`.
Register and login both need it.

- *Expected:* `signIn(tokens: AuthTokens): Promise<void>` exported from Plan A's session module —
  writes the pair to secure storage, clears the pending marker, sets status to `signedIn`, and
  fetches `/me`.
- *Task 1 action:* read Plan A's session module and use whatever it actually exports. If nothing
  equivalent exists, **stop and raise it** — a screen must not write secure storage itself, and
  implementing token persistence here would duplicate the one piece of Plan A this plan most
  depends on being singular.

**CG-2 — a taken email cannot be recognised.** `ApiErrorKind` has no `conflict`, and `ApiError`
carries no `code` or HTTP status. The server's `AuthError::EmailTaken` becomes `ApiError::conflict()`
(HTTP 409, wire code `"conflict"`), which under the frozen mapping can only fall into `kind:
"server"` — and "Ada gangguan di server" for a taken email breaks AM-50 AC3 outright.

- *Recommended resolution (smallest, and no change to the frozen union):* Plan A's register mapping
  reports a taken email or username as `kind: "validation"` with `fields.email` / `fields.username`
  set. That is literally what the spec asks the backend for — "the handler matches on the constraint
  name and reports the field that actually collided" — and it makes AM-57's "message under the field
  that failed" fall out of the same code path as every other field error.
- *Fallback if Plan A kept 409:* add one optional property to Plan A's `ApiError` —
  `readonly code?: string`, copied verbatim from the envelope's `error.code` — and key on
  `code === "conflict"`. Additive, breaks nothing.
- Either way, Task 1 exports one function so no screen encodes the choice:
  `registerConflictOf(error: ApiError): "email" | "username" | null`.

**CG-3 — nothing says the `(auth)` layout redirects out when a session starts.** The spec's gate is
described in the launch direction ("no session → welcome"). After a successful login the session
status flips to `signedIn` and the person must leave `(auth)`.

- *Expected:* Plan A's `app/(auth)/_layout.tsx` renders `<Redirect />` when
  `useSession().status === "signedIn"`. That is the declarative gate the spec describes, and it is
  what keeps "exactly one redirect" true.
- *Task 1 action:* confirm by reading the layout. If it is absent, add the redirect **to the layout**
  — not to the screens. A `router.replace()` in a mutation's `onSuccess` is the thing the spec's
  "exactly one redirect" rule exists to prevent, because register, login, and the bootstrap gate
  would then each own a copy.

---

## Decisions this plan makes

**No react-hook-form on these three screens.** Login has two fields, register has four. A
`useState` value plus `schema.safeParse` is about fifteen lines per screen, and it avoids assuming a
package the frozen contract does not actually name: `zodResolver` lives in `@hookform/resolvers`, a
*separate* install from `react-hook-form`. If the controller prefers RHF for consistency with Plan
D's wizard, only the field-error mechanism changes — the schemas, the hooks, and the screen layouts
are untouched.

**Password strength is measured by length alone.** The backend already states the reasoning in
`check_password_shape`: composition rules push people toward `Password1!` and NIST dropped them in
2017. The meter shows four bands from character count and never demands a symbol. Character count,
not byte count, matching the backend's own test (`length_is_counted_in_characters_not_bytes`).

**The availability check never blocks the button on a failure.** `taken` disables the register
button (AM-50 AC2). A network failure, a 5xx, or a 429 on the availability endpoint resolves to
`unknown`, which shows an honest line and leaves the button enabled — the server rejects a collision
at submit time anyway, and a person unable to register because a *hint* endpoint is down is a worse
outcome than a rejected submit.

**The dead "recover password" path: one honest sentence, no control.** AM-50 AC3 asks the
already-registered path to offer signing in *or* recovering the password. Password reset is AM-54
and does not exist — no endpoint, no screen, no email sender. This plan ships the "masuk" half as a
real action carrying the typed email across, and states the other half in one line of copy
(`"Pengaturan ulang kata sandi belum tersedia di aplikasi."`) with **no button, no link, and no
invented support address**. A control whose only behaviour is to say "not available" is a dead
button with extra steps, and inventing a support email would be fabricating a fact.

> **AC3 is therefore partially closed, and that goes in the ticket rather than being papered over.**
> When AM-54 lands, the sentence becomes a button and nothing else on the screen changes.

**The welcome screen drops the guest-preview link.** `docs/mobile-feature-breakdown.md` E1-1 lists
"guest-preview link" alongside the two buttons. There is no browse-without-account surface in Plans
A–D, so the link would go nowhere. Omitted, recorded here.

**The ToS consent names both documents as text, not links.** No terms page and no privacy page
exists at any URL this app knows. Same reasoning as the recovery path.

**E1-1 has no Jira story.** Welcome appears in the feature breakdown and in the spec's route tree,
but no AM ticket covers it. It is kept deliberately minimal — a value proposition, two buttons, and
nothing else — so that no product decision is smuggled in under a ticket that does not exist.

---

## File structure

```
apps/mobile/src/
  components/input/
    AmTextField.tsx              MODIFY — five input-behaviour props (Task 1)
  features/auth/
    api.ts                       request functions + mutation hooks + AuthTokens (Task 1)
    schemas.ts                   zod schemas, the username mirror regex, fieldErrorsOf (Task 1)
    conflict.ts                  registerConflictOf — CG-2 lives in exactly one place (Task 1)
    useCountdown.ts              wall-clock countdown for the 429 (Task 2)
    useUsernameAvailability.ts   debounce + AbortController (Task 3)
    PasswordStrength.tsx         four-band meter (Task 3)
    ConsentCheckbox.tsx          local checkbox — the design system has none (Task 3)
    SignOutConfirm.tsx           confirmation sheet + signOut() trigger (Task 6)
  app/(auth)/
    index.tsx                    Welcome (Task 5)
    login.tsx                    Login (Task 2, extended Task 4)
    register.tsx                 Register (Task 3, extended Task 4)
```

`features/auth/` holds five files that change together and one screen-shaped component. The three
route files stay thin: they own layout and copy, and every rule lives in the feature folder where it
can be read in one sitting.

---

## Execution mode

**Run shape verdict (§28).**

**1. What runs in parallel, what is serialised on what.**
Task 1 and Task 6 start together — Task 6 consumes only `signOut()` from Plan A and touches no file
Task 1 touches. When Task 1 lands, **Task 2 and Task 3 run concurrently**: they share `schemas.ts`
and `api.ts` as *readers* only, and write `login.tsx` and `register.tsx` respectively. Task 4
modifies both screens, so it is serialised behind both. Task 5 creates `app/(auth)/index.tsx` and
links to `/login` and `/register`, so it is serialised behind Task 2 and Task 3 (typed routes will
not compile a link to a route file that does not exist) but runs **concurrently with Task 4** —
different files, no shared interface.

```
T1 ─┬─> T2 ─┬─> T4
    └─> T3 ─┴─> T5
T6 (independent, any time)
```

**2. What the writers cannot discover for themselves.**
Everything in the ENVIRONMENT block below, pasted verbatim into every brief. Plus, for Task 1
specifically: the three contract gaps above are the reason Task 1 exists as a separate task, and its
first step is reading Plan A's shipped code rather than trusting this document's expectation of it.

**3. Where the risk concentrates.** Task 1. It is the only task that touches a shipped
design-system file (`AmTextField`), and it is where all three contract gaps are resolved — every
later task is built on its answers. Task 2's uniform-error requirement is the other concentration:
it is a security property, not a copy preference, and it is easy to "improve" into a leak.

**4. What the plan is missing.** Nothing blocking. Two things are underspecified *by the spec* and
are called out in "Open questions" at the foot of this file.

**Reviewer tiers.** Task 1 and Task 2 carry a public-contract and an authentication-surface diff
respectively → reviewer on `opus`, no downgrade. Tasks 3–6 are screen work → reviewer on `opus`
default, `sonnet` acceptable on Task 5 (welcome) alone, which is layout and copy.

**Nothing here is mechanical.** No task folds into another's review.

---

## ENVIRONMENT — paste verbatim into every task brief

```
1. Every make target runs from the REPOSITORY ROOT.
2. Mobile gate: `make mb-check` (fmt-check prerequisite -> expo typed routes ->
   tsc --noEmit -> expo lint). Whole repo: `make check`.
3. Bun, never npm. `bun add --filter` DOES NOT EXIST — use
   `bun add --cwd apps/mobile <pkg>` or `bun x expo install <pkg>`.
   `bun install --frozen-lockfile` must stay EXIT=0 with bun.lock unchanged.
4. Prettier runs from the ROOT only (`bun run format`). Markdown excluded.
5. ** expo-router SDK 56+ VENDORS ITS OWN NAVIGATION. ** Do NOT install
   @react-navigation/native — the router throws "no longer compatible with
   react-navigation". ThemeProvider/DarkTheme/DefaultTheme come FROM
   "expo-router".
6. _layout.tsx already overrides the navigation container background to
   transparent so AmGround shows through. Do not undo it.
7. NEVER put a changing `key` on a View wrapping {children} at the app root.
8. apps/mobile HAS a test runner as of Plan A: `bun test test/`, run by
   `make mb-check` and by CI. One suite exists, test/session.test.ts, which
   mocks modules with `mock.module` from "bun:test". There is NO React
   renderer and no @testing-library — a component cannot be rendered in a
   test. Pure functions (schemas, mappers, countdown math) CAN be tested and
   a task whose TDD verdict is `yes` puts its test in apps/mobile/test/.
   tsconfig is strict; `@typescript-eslint/no-explicit-any: error`.
9. The AM-15 design system is complete and MUST be used — see
   apps/mobile/src/app/catalog.tsx for a worked example of every primitive.
10. Never set allowFontScaling={false}. Large system text must reflow.
11. Root .env belongs to the BACKEND; apps/mobile reads only EXPO_PUBLIC_*.
12. CI workflows are path-filtered per app.
13. Backend gates go through `make` ONLY. The Makefile loads and exports the root
    `.env`; a bare `cargo build`/`cargo test` from a shell does not, so sqlx
    silently switches between the committed `.sqlx` offline cache and a live
    database check and the two can disagree. Plan B touches no Rust, so this
    matters only if you find yourself running cargo at all — which you should
    not.
14. Piping a command through `tail`/`head` REPLACES its exit code with the
    pipe's. `make mb-check | tail -5` reports success on a failing gate. Read
    `$?` from the unpiped command, or write to a file and grep it.
```

## Quality gate — paste verbatim into every task brief (this repo runs NO Sonar)

```
TypeScript (apps/mobile):
- strict on; NO explicit any (eslint error); no @ts-ignore without a one-line reason.
- React props readonly, exported as `<Component>Props`.
- No raw design values — everything via useTheme().
- Never allowFontScaling={false}.
- Prefer ?? and ?., arr.at(-1), real elements over ARIA roles, no nested ternaries.
- Product strings Bahasa Indonesia; identifiers/comments English.
- Verify: bun run format -> make mb-check (read exit codes, not piped output).
```

---

## Task 1: Auth request layer, schemas, and the AmTextField input props

**Brief opener:** Invoke `frontend-design` first. The font/palette gate does **not** apply here —
this surface has a committed design system (`docs/design.md`, `packages/tokens`, and the AM-15 `Am*`
primitives), so `impeccable` refines within it and does not re-open the typeface or the palette.

**Files:**
- Modify: `apps/mobile/src/components/input/AmTextField.tsx` (props interface at :13-24, the
  `TextInput` at :60-87)
- Create: `apps/mobile/src/features/auth/api.ts`
- Create: `apps/mobile/src/features/auth/schemas.ts`
- Create: `apps/mobile/src/features/auth/conflict.ts`
- Read first (do not modify): Plan A's `src/shared/api/*` and `src/shared/session/*`, and
  `app/(auth)/_layout.tsx`

**Interfaces:**
- Consumes: `apiRequest`, `ApiError` from Plan A's api module; `signIn` from Plan A's session module
  (CG-1); `useMutation` from `@tanstack/react-query`; `z` from `zod`.
- Produces:
  - `AmTextFieldProps` gains `autoCapitalize?: "none" | "sentences" | "words" | "characters"`,
    `autoCorrect?: boolean`, `autoComplete?: TextInputProps["autoComplete"]`,
    `textContentType?: TextInputProps["textContentType"]`, `maxLength?: number`
  - `interface AuthTokens { readonly access_token: string; readonly refresh_token: string; readonly token_type: string; readonly expires_in: number }`
  - `interface RegisterInput { readonly email: string; readonly username: string; readonly password: string }`
  - `interface LoginInput { readonly email: string; readonly password: string }`
  - `function usernameAvailability(username: string, signal: AbortSignal): Promise<{ available: boolean }>`
  - `function useRegister(): UseMutationResult<AuthTokens, ApiError, RegisterInput>`
  - `function useLogin(): UseMutationResult<AuthTokens, ApiError, LoginInput>`
  - `const USERNAME_PATTERN: RegExp`
  - `const loginSchema: ZodType<LoginInput>` · `const registerSchema` (adds `consent: true`)
  - `function fieldErrorsOf(error: ZodError): Record<string, string>`
  - `function registerConflictOf(error: ApiError): "email" | "username" | null`

**TDD: no** — `apps/mobile` has no test runner and this work does not add one (ENV 8). `schemas.ts`
is the one file here that is pure logic worth covering, and the honest reason not to write a test
file for it is that **nothing would run it**: `make mb-check` is `tsc --noEmit` and `expo lint`, so a
`schemas.test.ts` would sit in the tree passing by never executing — which is exactly the false
green the backend's own `CLAUDE.md` records as a defect worth a section. Verified instead by the
boundary table in Step 3, typed into the register field on a simulator. When a runner arrives,
`USERNAME_PATTERN` is the first thing to cover, alongside the spec's own nomination of the refresh
state machine.

**Acceptance criteria:**
- Every task after this one compiles against these signatures without inventing a name.
- `USERNAME_PATTERN` accepts and rejects exactly the table in Step 3 — 3–30 chars, `a-z0-9._`, no
  leading/trailing dot **or underscore**, no consecutive dots.
- The three contract gaps are answered against Plan A's shipped code, and any divergence from this
  document is written back into this file.
- An email field on iOS no longer capitalises its first letter (the defect the `AmTextField` change
  exists to fix).
- `bun.lock` is unchanged.

- [ ] **Step 1: Read Plan A's shipped code and resolve CG-1, CG-2, CG-3**

Before writing anything. Use `graphify query "session store secure storage signIn tokens"` and
`graphify query "api client error mapping ApiError"` to orient, then read the files it names.

Answer, in writing, in this task's notes:
1. The exact export that starts a session from a token pair, and its module path.
2. What a 409 from `POST /auth/register` becomes — `kind` and whether `fields` is populated.
3. Whether `app/(auth)/_layout.tsx` redirects out when `status === "signedIn"`.

Adopt Plan A's actual answers. Where they differ from the expectations above, edit the "Three gaps"
section of this plan file in the same step — a stale contract note is how the next task ships against
a signature that does not exist.

- [ ] **Step 2: Add the five input-behaviour props to `AmTextField`**

The design system's text field exposes `secureTextEntry` and `keyboardType` but nothing that
controls autocapitalisation, autocorrection, or password-manager integration. On iOS that means the
email field capitalises `Budi@…` and the username field autocorrects, so the register form is
actively broken without this. These are input *behaviour*, not design values — no token is involved
and no visual rule changes.

In the props interface, after `keyboardType`:

```ts
  readonly autoCapitalize?: TextInputProps["autoCapitalize"];
  readonly autoCorrect?: boolean;
  /** Password-manager and keyboard hints. "email" | "username" | "new-password" | "current-password". */
  readonly autoComplete?: TextInputProps["autoComplete"];
  readonly textContentType?: TextInputProps["textContentType"];
  readonly maxLength?: number;
```

Add `TextInputProps` to the existing `react-native` type import, destructure the five names in the
function signature, and pass them straight through to `<TextInput>`. Nothing else in the file
changes — not the styles, not the 52pt height, not the error branch.

Record in the ledger: *the design system's `AmTextField` shipped without input-behaviour props; five
were added by Plan B for the auth forms.*

- [ ] **Step 3: Write `schemas.ts`**

```ts
import { z } from "zod";

/**
 * The client-side mirror of the server's username canonicaliser.
 *
 * `a-z0-9._`, 3-30 characters, no leading or trailing dot or underscore, no
 * consecutive dots. The first and last character classes are alphanumeric,
 * which is what enforces the leading/trailing rule; `{1,28}` between them is
 * what makes the total 3-30.
 *
 * This exists for instant feedback and NOTHING ELSE. The server's
 * canonicaliser is the authority and the only thing that decides a name is
 * acceptable — see the spec's "Tidak boleh ada".
 */
export const USERNAME_PATTERN = /^(?!.*\.\.)[a-z0-9][a-z0-9._]{1,28}[a-z0-9]$/;

/** The server's floor, in characters rather than bytes. */
export const MIN_PASSWORD = 8;

export const loginSchema = z.object({
  email: z.email("Format email belum benar."),
  password: z.string().min(1, "Kata sandi belum diisi."),
});

export const registerSchema = z.object({
  email: z.email("Format email belum benar."),
  password: z.string().min(MIN_PASSWORD, `Minimal ${MIN_PASSWORD} karakter.`),
  username: z
    .string()
    .min(3, "Minimal 3 karakter.")
    .max(30, "Maksimal 30 karakter.")
    .regex(USERNAME_PATTERN, "Awali dan akhiri dengan huruf atau angka. Tanpa titik ganda."),
  consent: z.literal(true, "Setujui dulu syarat layanan dan kebijakan privasi."),
});

/**
 * The first message per field, keyed by field name.
 *
 * First rather than all: AM-57 puts one message under the field that failed,
 * and a stack of three under one input is noise, not help.
 */
export function fieldErrorsOf(error: z.ZodError): Record<string, string> {
  const out: Record<string, string> = {};
  for (const issue of error.issues) {
    const key = issue.path.at(0);
    if (typeof key === "string" && !(key in out)) out[key] = issue.message;
  }
  return out;
}
```

> **Zod version note:** `z.email(...)` is the zod v4 form. If Plan A pinned v3, this is
> `z.string().email("Format email belum benar.")` and `z.literal(true, { message: "…" })`. Check
> `apps/mobile/package.json` rather than guessing — this is a one-line difference that `tsc` catches
> immediately either way.

The boundary table `USERNAME_PATTERN` must satisfy — verify by typing each into the register field
in Task 3's visual pass, and eyeball it now by reading the regex against it:

| Input | Expected | Why |
|---|---|---|
| `ok` | reject | 2 characters |
| `oks` | accept | exactly 3 |
| `a`×30 | accept | exactly 30 |
| `a`×31 | reject | 31 |
| `.oksa` | reject | leading dot |
| `_oksa` | reject | leading underscore |
| `oksa.` | reject | trailing dot |
| `oksa_` | reject | trailing underscore |
| `ok..sa` | reject | consecutive dots |
| `ok__sa` | accept | consecutive underscores are not banned by the spec |
| `ok.sa` | accept | single interior dot |
| `Oksa` | reject | uppercase — the field lowercases as you type, so this is only reachable by paste |
| `oks a` | reject | space |
| `oksa-1` | reject | hyphen is not in the set |

- [ ] **Step 4: Write `api.ts`**

```ts
import { useMutation, type UseMutationResult } from "@tanstack/react-query";

import { apiRequest, type ApiError } from "@/shared/api";
import { signIn } from "@/shared";

/**
 * The wire shape, snake_case, exactly as the frozen contract states it.
 *
 * `Me` is camelCase in the same contract, which means the client does not
 * blanket-camelise — so these keys stay as the server sends them rather than
 * being tidied into a shape the parser would not produce.
 */
export interface AuthTokens {
  readonly access_token: string;
  readonly refresh_token: string;
  readonly token_type: string;
  readonly expires_in: number;
}

export interface RegisterInput {
  readonly email: string;
  readonly username: string;
  readonly password: string;
}

export interface LoginInput {
  readonly email: string;
  readonly password: string;
}

export function registerAccount(input: RegisterInput): Promise<AuthTokens> {
  return apiRequest<AuthTokens>("/auth/register", { method: "POST", body: input });
}

export function loginWithPassword(input: LoginInput): Promise<AuthTokens> {
  return apiRequest<AuthTokens>("/auth/login", { method: "POST", body: input });
}

/**
 * `signal` is required rather than optional: every caller of this endpoint is
 * a keystroke-driven check that MUST be cancellable, and an optional signal is
 * one that eventually gets omitted.
 */
export function usernameAvailability(
  username: string,
  signal: AbortSignal,
): Promise<{ available: boolean }> {
  return apiRequest<{ available: boolean }>(
    `/usernames/${encodeURIComponent(username)}/availability`,
    { signal },
  );
}

/**
 * Success hands the pair to Plan A and stops.
 *
 * No navigation here. The (auth) group's layout redirects when the session
 * status flips, which is what keeps the spec's "exactly one redirect" true —
 * a replace() in this callback would be a second owner of the same decision.
 */
export function useRegister(): UseMutationResult<AuthTokens, ApiError, RegisterInput> {
  return useMutation<AuthTokens, ApiError, RegisterInput>({
    mutationFn: registerAccount,
    onSuccess: signIn,
  });
}

export function useLogin(): UseMutationResult<AuthTokens, ApiError, LoginInput> {
  return useMutation<AuthTokens, ApiError, LoginInput>({
    mutationFn: loginWithPassword,
    onSuccess: signIn,
  });
}
```

Adjust the two import paths to whatever Plan A actually exports (Step 1). `signIn` may need
wrapping if its signature differs — wrap it here, in one place, rather than at each call site.

- [ ] **Step 5: Write `conflict.ts`**

```ts
import type { ApiError } from "@/shared/api";

export type RegisterConflict = "email" | "username" | null;

/**
 * Which field a failed registration collided on, if any.
 *
 * One function so that CG-2's resolution lives in exactly one place. Under the
 * recommended resolution a collision arrives as a field-keyed validation
 * error; under the fallback it arrives as a 409 whose wire code is "conflict",
 * which can only mean the email, because a username collision is reported per
 * field. Both readings are handled, and neither is a guess the screens make.
 */
export function registerConflictOf(error: ApiError): RegisterConflict {
  if (error.fields?.username) return "username";
  if (error.fields?.email) return "email";
  // The 409 fallback. Read off a widened view rather than off `ApiError`, so
  // this file compiles whether or not Plan A carries a `code` — the property
  // is not invented here, only tolerated if it is there.
  if ((error as { readonly code?: string }).code === "conflict") return "email";
  return null;
}
```

If Step 1 found that Plan A's `ApiError` **does** carry `code`, drop the cast and read `error.code`
directly — a cast that is no longer needed is a cast that hides the next change. If Plan A returns a
field-keyed validation error for both collisions, delete the third branch entirely and record in the
ledger that the fallback was not needed.

- [ ] **Step 6: Verify**

```bash
bun run format
make mb-check
```

Both must exit 0. Read the exit codes, not the piped output. `git diff --stat bun.lock` must be
empty.

---

## Task 2: Login screen — uniform error and the rate-limit countdown

**Closes:** AM-51 AC1, AM-51 AC2, AM-60, AM-61.

**Brief opener:** Invoke `frontend-design` first. The font/palette gate does **not** apply here —
this surface has a committed design system (`docs/design.md`, `packages/tokens`, and the AM-15 `Am*`
primitives), so `impeccable` refines within it and does not re-open the typeface or the palette.

**Files:**
- Create: `apps/mobile/src/app/(auth)/login.tsx`
- Create: `apps/mobile/src/features/auth/useCountdown.ts`

**Interfaces:**
- Consumes: `useLogin`, `LoginInput` (Task 1) · `loginSchema`, `fieldErrorsOf` (Task 1) ·
  `AmButton`, `AmTextField`, `AmCard`, `useTheme`, `useSafeAreaInsets`
- Produces: `function useCountdown(): { readonly remaining: number; start: (seconds: number) => void }`
  · `function formatCountdown(seconds: number): string` (`95` -> `"1:35"`) · the route `/login`,
  which Task 4 and Task 5 link to and Task 4 extends to read an `email` param.

**TDD: yes for `useCountdown`'s pure math, no for the screen.** A runner exists (corrected ENV 8):
`bun test test/`, in `mb-check` and CI. It has **no React renderer**, so `login.tsx` genuinely cannot
be render-tested — but a wall-clock countdown is arithmetic over two timestamps, it is exactly the
kind of thing that is wrong at a boundary and invisible on screen, and it is testable today.
Extract the remaining-seconds computation as a plain exported function, write its failing tests
first (zero, one, expiry exactly now, expiry in the past, a fractional second, a clock that jumps
backwards), then build the hook on it. The screen itself is verified by
running it: the states listed below, on a simulator, in both themes.

**Acceptance criteria:**
- **AM-51 AC1** — correct credentials sign in. The screen does **not** navigate; the session status
  flips and the `(auth)` layout redirects (CG-3). "Beranda dengan kendaraan aktif terakhir" is
  Plan C's screen — this plan's obligation ends when the layout takes over, and that is stated in
  the ticket comment rather than claimed as done.
- **AM-51 AC2 / AM-60** — an unknown email and a wrong password produce a **byte-identical**
  message. Only one string exists in the file for this case, so they cannot drift apart. The message
  is the server's own (`error.message` from the uniform 401) with a local fallback.
- **AM-61** — a 429 starts a countdown from `retryAfterSeconds`; the submit button is disabled while
  it runs and **re-enables at zero** without a reload.
- Nothing anywhere distinguishes the per-IP limiter from the per-account limiter, and nothing
  reports attempts remaining.
- Email field does not capitalise, does not autocorrect, and offers the password manager
  `current-password`.

- [ ] **Step 1: Write `useCountdown.ts`**

```ts
import { useCallback, useEffect, useState } from "react";

/**
 * Seconds remaining until a rate limit lifts.
 *
 * Driven from a wall-clock deadline rather than by decrementing a number,
 * because a phone throttles timers the moment the app goes to the background —
 * a decrementing counter comes back thirty seconds behind and the button stays
 * dead long after the server would accept the attempt.
 *
 * Deliberately NOT persisted. A restart clears it, the next attempt gets a
 * fresh 429 with a fresh number, and the server stays the authority on the
 * wait. Persisting it would be a second copy of the limiter's state on a
 * device that cannot be trusted with it anyway.
 */
export function useCountdown(): { readonly remaining: number; start: (seconds: number) => void } {
  const [deadline, setDeadline] = useState<number | null>(null);
  const [remaining, setRemaining] = useState(0);

  useEffect(() => {
    if (deadline === null) return;
    const tick = () => {
      const left = Math.max(0, Math.ceil((deadline - Date.now()) / 1000));
      setRemaining(left);
      if (left === 0) setDeadline(null);
    };
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [deadline]);

  const start = useCallback((seconds: number) => {
    setDeadline(Date.now() + Math.max(0, seconds) * 1000);
  }, []);

  return { remaining, start };
}

/** `95` -> `"1:35"`. */
export function formatCountdown(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  return `${minutes}:${rest.toString().padStart(2, "0")}`;
}
```

- [ ] **Step 2: Write `app/(auth)/login.tsx`**

```tsx
import { Link } from "expo-router";
import { useState } from "react";
import { KeyboardAvoidingView, Platform, ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmCard } from "@/components/display";
import { AmButton, AmTextField } from "@/components/input";
import { useLogin } from "@/features/auth/api";
import { fieldErrorsOf, loginSchema } from "@/features/auth/schemas";
import { formatCountdown, useCountdown } from "@/features/auth/useCountdown";
import { useTheme } from "@/theme";

/**
 * The one string for a refused credential.
 *
 * A constant rather than two literals, because AM-51 AC2 is a security
 * measure: an unknown email and a wrong password must be indistinguishable,
 * and two literals in two branches is how they quietly stop being. The server
 * already answers both with one uniform 401; this is the client half of the
 * same discipline.
 */
const CREDENTIALS_REFUSED = "Email atau kata sandi salah.";

export default function Login() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const login = useLogin();
  const countdown = useCountdown();

  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<string | null>(null);

  const waiting = countdown.remaining > 0;
  const busy = login.isPending;

  const submit = () => {
    setFormError(null);
    const parsed = loginSchema.safeParse({ email: email.trim(), password });
    if (!parsed.success) {
      setErrors(fieldErrorsOf(parsed.error));
      return;
    }
    setErrors({});
    login.mutate(parsed.data, {
      onError: (error) => {
        if (error.kind === "rateLimited") {
          countdown.start(error.retryAfterSeconds ?? 60);
          return;
        }
        // Every other failure that is not a field problem gets one line. The
        // 401 branch is not special-cased on purpose — see CREDENTIALS_REFUSED.
        setFormError(error.kind === "unauthorized" ? CREDENTIALS_REFUSED : error.message);
      },
    });
  };

  const buttonLabel = waiting ? `Coba lagi dalam ${formatCountdown(countdown.remaining)}` : "Masuk";

  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === "ios" ? "padding" : undefined}
      style={{ flex: 1 }}
    >
      <ScrollView
        keyboardShouldPersistTaps="handled"
        contentContainerStyle={{
          padding: theme.pagePadding,
          paddingTop: insets.top + theme.space[8],
          paddingBottom: insets.bottom + theme.space[10],
          gap: theme.space[6],
        }}
      >
        <View style={{ gap: theme.space[2] }}>
          <Text accessibilityRole="header" style={[theme.type.h1, { color: theme.color.textPrimary }]}>
            Masuk
          </Text>
          <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
            Garasi kamu menunggu.
          </Text>
        </View>

        <AmCard role="working">
          <View style={{ gap: theme.space[4] }}>
            <AmTextField
              label="Email"
              value={email}
              onChangeText={setEmail}
              placeholder="nama@email.com"
              error={errors.email}
              keyboardType="email-address"
              autoCapitalize="none"
              autoCorrect={false}
              autoComplete="email"
              textContentType="emailAddress"
            />
            <AmTextField
              label="Kata sandi"
              value={password}
              onChangeText={setPassword}
              error={errors.password}
              secureTextEntry
              autoCapitalize="none"
              autoCorrect={false}
              autoComplete="current-password"
              textContentType="password"
            />
            {formError ? (
              <Text
                accessibilityLiveRegion="polite"
                accessibilityRole="alert"
                style={[theme.type.caption, { color: theme.color.semanticText.danger }]}
              >
                {formError}
              </Text>
            ) : null}
            {waiting ? (
              <Text
                accessibilityLiveRegion="polite"
                style={[theme.type.caption, { color: theme.color.semanticText.warning }]}
              >
                Terlalu banyak percobaan. Coba lagi dalam {formatCountdown(countdown.remaining)}.
              </Text>
            ) : null}
            <AmButton
              label={buttonLabel}
              onPress={submit}
              size="lg"
              loading={busy}
              disabled={waiting}
            />
          </View>
        </AmCard>

        <View style={{ gap: theme.space[2], alignItems: "center" }}>
          <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
            Belum punya akun?
          </Text>
          <Link href="/register" style={[theme.type.label, { color: theme.color.accentText }]}>
            Daftar sekarang
          </Link>
        </View>
      </ScrollView>
    </KeyboardAvoidingView>
  );
}
```

Two things in that file are load-bearing and easy to undo:

- `CREDENTIALS_REFUSED` is a module constant with one call site. Do not "helpfully" add a second
  branch for a not-found account. There is no such response, and inventing one would be the exact
  leak AM-51 AC2 exists to prevent.
- The 429 branch returns before `setFormError`, so a rate limit never renders as a credential error.
  The countdown line says only how long — never which limiter, never attempts remaining.

- [ ] **Step 3: Verify the gate**

```bash
bun run format
make mb-check
```

- [ ] **Step 4: Visual verification (§27 — mandatory, this is frontend)**

Run the app and open `/login`. Screenshot **every state below, in light and dark**:

1. **Empty** — nothing typed, button enabled, no error text anywhere.
2. **Typing** — email half-entered; confirm iOS does not capitalise it and does not autocorrect.
3. **Invalid** — `bukan-email` + empty password → per-field messages under both fields.
4. **Submitting** — the button shows its spinner and its label at once (`AmButton loading`).
5. **Credentials refused** — a wrong password on a real account, then an email with no account.
   **Compare the two screenshots pixel for pixel.** They must be identical below the fields.
6. **Rate limited** — fail login repeatedly until the server answers 429. The button label becomes a
   countdown, the button is disabled, the warning line shows the same number, and **watch it reach
   zero and re-enable itself** without touching anything.
7. **Offline** — turn off the network, submit, and confirm the offline message rather than the
   credential one.

Also verify with the system font size at its largest that nothing clips or overlaps.

---

## Task 3: Register screen — live validation, debounced availability, consent gate

**Closes:** AM-50 AC1, AM-50 AC2, AM-50 AC4, AM-57.

**Brief opener:** Invoke `frontend-design` first. The font/palette gate does **not** apply here —
this surface has a committed design system (`docs/design.md`, `packages/tokens`, and the AM-15 `Am*`
primitives), so `impeccable` refines within it and does not re-open the typeface or the palette.

**Files:**
- Create: `apps/mobile/src/app/(auth)/register.tsx`
- Create: `apps/mobile/src/features/auth/useUsernameAvailability.ts`
- Create: `apps/mobile/src/features/auth/PasswordStrength.tsx`
- Create: `apps/mobile/src/features/auth/ConsentCheckbox.tsx`

**Interfaces:**
- Consumes: `useRegister`, `usernameAvailability` (Task 1) · `registerSchema`, `fieldErrorsOf`,
  `USERNAME_PATTERN`, `MIN_PASSWORD` (Task 1) · `AmButton`, `AmTextField`, `AmCard`, `useTheme`
- Produces:
  - `type Availability = "idle" | "checking" | "available" | "taken" | "unknown"`
  - `function useUsernameAvailability(username: string, enabled: boolean): Availability`
  - `function strengthOf(password: string): 0 | 1 | 2 | 3 | 4`
  - `function PasswordStrength(props: PasswordStrengthProps)` with
    `interface PasswordStrengthProps { readonly password: string }`
  - `function ConsentCheckbox(props: ConsentCheckboxProps)` with
    `interface ConsentCheckboxProps { readonly checked: boolean; readonly onChange: (checked: boolean) => void; readonly label: string }`
  - the route `/register`, which Task 4 extends and Task 5 links to.

**TDD: yes for the pure parts, no for the screens.** A runner exists (corrected ENV 8) and it has
no React renderer, so the components cannot be render-tested — but the plan already identified the
pure logic here, and "no runner" was the only reason it went untested. Write failing tests first for
`strengthOf` (every band boundary, and **character count not byte count** — an emoji password must
score by characters, matching the backend's own `length_is_counted_in_characters_not_bytes`) and for
the availability state machine's transitions (idle → checking → available/taken/unknown, and every
failure resolving to `unknown`, never `taken`). The rest of the pure part (`strengthOf`, the
availability state machine) is
verified by the boundary table in Task 1 Step 3 and by the states in Step 5 below.

**Acceptance criteria:**
- **AM-50 AC1** — a valid email, a password of ≥8 characters, and a free username create the account
  and start a session. Onward routing is the `(auth)` layout's (CG-3); the add-first-car wizard is
  Plan D and must not be linked from here.
- **AM-50 AC2** — the availability answer appears **after the person stops typing**, not on every
  keystroke, and `taken` disables the submit button.
- **AM-50 AC4** — the submit button cannot be pressed until the consent control is checked.
- **AM-57** — every message renders **under the field it belongs to**, and the button stays inactive
  until the whole form is valid.
- Changing the username while a check is in flight **cancels that request**. Verify it in the
  network log, not by reasoning about it.
- An availability check that fails leaves the button enabled.

- [ ] **Step 1: Write `useUsernameAvailability.ts`**

```ts
import { useEffect, useState } from "react";

import { usernameAvailability } from "./api";

export type Availability = "idle" | "checking" | "available" | "taken" | "unknown";

/**
 * AM-50's technical note asks for "a few hundred milliseconds". 400ms is long
 * enough that a normal typist does not fire a request per character and short
 * enough that the answer feels like it belongs to what was just typed.
 */
const DEBOUNCE_MS = 400;

/**
 * Debounced availability, with cancellation as a property of the cleanup
 * rather than as a flag somebody has to check.
 *
 * One AbortController per attempt: the effect's cleanup aborts it and clears
 * the timer, so a keystroke that lands mid-flight kills the request outright
 * instead of leaving a late answer to overwrite a newer one.
 *
 * TanStack Query would also cancel here, but only as a consequence of the last
 * observer detaching — a library behaviour to trust rather than a line to
 * read. Cancellation is a stated requirement of this ticket, so it is written
 * down. There is also nothing worth caching: an availability answer goes stale
 * the moment somebody else registers.
 */
export function useUsernameAvailability(username: string, enabled: boolean): Availability {
  const [state, setState] = useState<Availability>("idle");

  useEffect(() => {
    if (!enabled) {
      setState("idle");
      return;
    }
    setState("checking");
    const controller = new AbortController();
    const timer = setTimeout(() => {
      usernameAvailability(username, controller.signal)
        .then((result) => setState(result.available ? "available" : "taken"))
        .catch(() => {
          // An abort is this effect being replaced, not a failure to report.
          if (controller.signal.aborted) return;
          // Offline, 5xx, or a 429 on the hint endpoint. The person is not
          // blocked by any of them — the server rejects a real collision at
          // submit time, and a dead hint endpoint must not stop registration.
          setState("unknown");
        });
    }, DEBOUNCE_MS);

    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [username, enabled]);

  return state;
}
```

- [ ] **Step 2: Write `PasswordStrength.tsx`**

```tsx
import { StyleSheet, Text, View } from "react-native";

import { useTheme } from "@/theme";
import { MIN_PASSWORD } from "./schemas";

export interface PasswordStrengthProps {
  readonly password: string;
}

/**
 * Length, and only length.
 *
 * The backend states the reasoning where it enforces the floor: composition
 * rules — a digit, a symbol, a capital — push people toward `Password1!` and
 * NIST dropped them in 2017. So this meter never asks for a character class,
 * and it counts CHARACTERS rather than bytes, matching the server's own test.
 */
export function strengthOf(password: string): 0 | 1 | 2 | 3 | 4 {
  const length = [...password].length;
  if (length === 0) return 0;
  if (length < MIN_PASSWORD) return 1;
  if (length < 12) return 2;
  if (length < 16) return 3;
  return 4;
}

const LABEL: Record<0 | 1 | 2 | 3 | 4, string> = {
  0: "",
  1: "Terlalu pendek",
  2: "Cukup",
  3: "Bagus",
  4: "Kuat",
};

export function PasswordStrength({ password }: PasswordStrengthProps) {
  const theme = useTheme();
  const score = strengthOf(password);
  if (score === 0) return null;

  // A bar is a FILL, which is what `semantic` is for; the label is words,
  // which is what `semanticText` is for. Swapping them is the mistake the
  // theme's own comments warn about.
  const fill =
    score === 1
      ? theme.color.semantic.danger
      : score === 2
        ? theme.color.semantic.warning
        : theme.color.semantic.success;
  const words =
    score === 1
      ? theme.color.semanticText.danger
      : score === 2
        ? theme.color.semanticText.warning
        : theme.color.semanticText.success;

  return (
    <View style={{ gap: theme.space[2] }}>
      <View style={[styles.track, { gap: theme.space[1] }]}>
        {[1, 2, 3, 4].map((segment) => (
          <View
            key={segment}
            style={[
              styles.segment,
              {
                height: theme.space[1],
                borderRadius: theme.radius.xs,
                backgroundColor: segment <= score ? fill : theme.color.border,
              },
            ]}
          />
        ))}
      </View>
      <Text accessibilityLiveRegion="polite" style={[theme.type.caption, { color: words }]}>
        {LABEL[score]}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  track: { flexDirection: "row" },
  segment: { flex: 1 },
});
```

The ternary chains pick a value and have no nesting inside a branch, so they are the flat form the
quality gate allows; do not collapse them into a nested conditional expression.

- [ ] **Step 3: Write `ConsentCheckbox.tsx`**

The design system has **no checkbox** — `components/display` ships Card, Chip, Badge, Avatar, and
BottomSheet; `components/input` ships Button, TextField, and Select. React Native has no checkbox
element either. This is a local component built from theme tokens, following `AmSelect`'s precedent
of using `accessibilityRole="radio"` for a control the platform does not provide.

**Record in the ledger:** *the design system has no checkbox primitive; Plan B built a local one for
the ToS consent. Promote it to `AmCheckbox` when a second consumer appears — not before.*

```tsx
import { Pressable, StyleSheet, Text, View } from "react-native";

import { useTheme } from "@/theme";

export interface ConsentCheckboxProps {
  readonly checked: boolean;
  readonly onChange: (checked: boolean) => void;
  readonly label: string;
}

export function ConsentCheckbox({ checked, onChange, label }: ConsentCheckboxProps) {
  const theme = useTheme();
  return (
    <Pressable
      accessibilityRole="checkbox"
      accessibilityState={{ checked }}
      accessibilityLabel={label}
      onPress={() => onChange(!checked)}
      // The whole row is the target, not the 24pt box — a legal consent that
      // takes three attempts to tick is a consent nobody read.
      style={({ pressed }) => [
        styles.row,
        {
          minHeight: theme.touchTargetMin,
          gap: theme.space[3],
          opacity: pressed ? 0.82 : 1,
        },
      ]}
    >
      <View
        style={[
          styles.box,
          {
            width: theme.space[6],
            height: theme.space[6],
            borderRadius: theme.radius.xs,
            borderWidth: 1,
            borderColor: checked ? theme.color.accent : theme.color.borderStrong,
            backgroundColor: checked ? theme.color.accent : "transparent",
          },
        ]}
      >
        {checked ? (
          <Text style={[theme.type.label, { color: theme.color.onAccent }]}>{"✓"}</Text>
        ) : null}
      </View>
      <Text style={[theme.type.body, styles.label, { color: theme.color.textSecondary }]}>
        {label}
      </Text>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center" },
  box: { alignItems: "center", justifyContent: "center" },
  label: { flex: 1 },
});
```

- [ ] **Step 4: Write `app/(auth)/register.tsx`**

```tsx
import { Link } from "expo-router";
import { useMemo, useState } from "react";
import { KeyboardAvoidingView, Platform, ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmCard } from "@/components/display";
import { AmButton, AmTextField } from "@/components/input";
import { useRegister } from "@/features/auth/api";
import { ConsentCheckbox } from "@/features/auth/ConsentCheckbox";
import { PasswordStrength } from "@/features/auth/PasswordStrength";
import { fieldErrorsOf, registerSchema, USERNAME_PATTERN } from "@/features/auth/schemas";
import { useUsernameAvailability, type Availability } from "@/features/auth/useUsernameAvailability";
import { useTheme } from "@/theme";

const CONSENT_LABEL = "Saya setuju dengan Syarat Layanan dan Kebijakan Privasi AnakMobil.";

const AVAILABILITY_HINT: Record<Availability, string> = {
  idle: "3–30 karakter. Huruf kecil, angka, titik, dan garis bawah.",
  checking: "Memeriksa ketersediaan…",
  available: "Username ini tersedia.",
  taken: "Username ini sudah dipakai.",
  unknown: "Belum bisa memeriksa sekarang. Kamu tetap bisa lanjut.",
};

export default function Register() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const register = useRegister();

  const [email, setEmail] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [consent, setConsent] = useState(false);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<string | null>(null);

  // The availability endpoint is only worth calling for a name the server
  // could accept — a syntax check first saves a request per malformed keystroke.
  const shapeOk = USERNAME_PATTERN.test(username);
  const availability = useUsernameAvailability(username, shapeOk);

  const values = useMemo(
    () => ({ email: email.trim(), username, password, consent }),
    [email, username, password, consent],
  );
  const parsed = registerSchema.safeParse(values);
  const canSubmit = parsed.success && availability !== "taken" && !register.isPending;

  const submit = () => {
    setFormError(null);
    if (!parsed.success) {
      setErrors(fieldErrorsOf(parsed.error));
      return;
    }
    setErrors({});
    const { consent: _agreed, ...input } = parsed.data;
    register.mutate(input, {
      onError: (error) => {
        if (error.fields) {
          setErrors(error.fields);
          return;
        }
        setFormError(error.message);
      },
    });
  };

  const usernameError = availability === "taken" ? AVAILABILITY_HINT.taken : errors.username;

  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === "ios" ? "padding" : undefined}
      style={{ flex: 1 }}
    >
      <ScrollView
        keyboardShouldPersistTaps="handled"
        contentContainerStyle={{
          padding: theme.pagePadding,
          paddingTop: insets.top + theme.space[8],
          paddingBottom: insets.bottom + theme.space[10],
          gap: theme.space[6],
        }}
      >
        <View style={{ gap: theme.space[2] }}>
          <Text accessibilityRole="header" style={[theme.type.h1, { color: theme.color.textPrimary }]}>
            Buat akun
          </Text>
          <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
            Mulai garasi digital kamu. Gratis, selamanya.
          </Text>
        </View>

        <AmCard role="working">
          <View style={{ gap: theme.space[4] }}>
            <AmTextField
              label="Email"
              value={email}
              onChangeText={setEmail}
              placeholder="nama@email.com"
              error={errors.email}
              keyboardType="email-address"
              autoCapitalize="none"
              autoCorrect={false}
              autoComplete="email"
              textContentType="emailAddress"
            />

            <AmTextField
              label="Username"
              value={username}
              // Lowercased on the way in, matching the server's canonicaliser.
              // Doing it here rather than at submit means what the person sees
              // is what the server will store.
              onChangeText={(value) => setUsername(value.toLowerCase())}
              placeholder="oksa.satya"
              hint={usernameError ? undefined : AVAILABILITY_HINT[availability]}
              error={usernameError}
              maxLength={30}
              autoCapitalize="none"
              autoCorrect={false}
              autoComplete="username"
              textContentType="username"
            />

            <View style={{ gap: theme.space[2] }}>
              <AmTextField
                label="Kata sandi"
                value={password}
                onChangeText={setPassword}
                error={errors.password}
                secureTextEntry
                autoCapitalize="none"
                autoCorrect={false}
                autoComplete="new-password"
                textContentType="newPassword"
              />
              <PasswordStrength password={password} />
            </View>

            <ConsentCheckbox checked={consent} onChange={setConsent} label={CONSENT_LABEL} />

            {formError ? (
              <Text
                accessibilityLiveRegion="polite"
                accessibilityRole="alert"
                style={[theme.type.caption, { color: theme.color.semanticText.danger }]}
              >
                {formError}
              </Text>
            ) : null}

            <AmButton
              label="Daftar"
              onPress={submit}
              size="lg"
              variant="accent"
              loading={register.isPending}
              disabled={!canSubmit}
            />
          </View>
        </AmCard>

        <View style={{ gap: theme.space[2], alignItems: "center" }}>
          <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
            Sudah punya akun?
          </Text>
          <Link href="/login" style={[theme.type.label, { color: theme.color.accentText }]}>
            Masuk
          </Link>
        </View>
      </ScrollView>
    </KeyboardAvoidingView>
  );
}
```

Note the consent field is stripped before the request: `registerSchema` validates it, and the server
has no `consent` parameter. Destructuring with a leading-underscore name is how it is discarded
without an unused-variable lint.

- [ ] **Step 5: Verify the gate**

```bash
bun run format
make mb-check
```

- [ ] **Step 6: Visual verification (§27 — mandatory)**

Open `/register`. Screenshot **each state, light and dark**:

1. **Empty** — hint text under the username field, no errors, button **disabled** (consent unticked).
2. **Typing an invalid username** — walk the boundary table from Task 1 Step 3 through the field.
   Watch the network log: **no request fires while the shape is invalid**.
3. **Debounce + cancel** — type `oksasatya` steadily. Confirm in the network log that one request
   fires after you stop, not nine, and that a request in flight is **cancelled** when you type again.
4. **Available / taken** — register a name, then try the same name in a fresh session: the hint says
   taken, the message moves under the field, and the button is disabled.
5. **Availability endpoint down** — kill the API and type a valid name: the "belum bisa memeriksa"
   line appears and the button stays enabled.
6. **Password strength** — 3, 8, 12, and 16 characters; the bar and the label both move, and no copy
   anywhere asks for a symbol or a capital.
7. **Consent gate** — everything valid, consent unticked → button disabled. Tick it → enabled. Tap
   the row text rather than the box and confirm it toggles.
8. **Submitting** and **server error** — the button's spinner, then the API stopped mid-submit.
9. Largest system font size: nothing clips, the consent label wraps rather than truncating.

---

## Task 4: The already-registered path — carry the email across to login

**Closes:** AM-50 AC3, AM-59.

**Brief opener:** Invoke `frontend-design` first. The font/palette gate does **not** apply here —
this surface has a committed design system (`docs/design.md`, `packages/tokens`, and the AM-15 `Am*`
primitives), so `impeccable` refines within it and does not re-open the typeface or the palette.

**Files:**
- Modify: `apps/mobile/src/app/(auth)/register.tsx` (the `onError` branch and the card's tail)
- Modify: `apps/mobile/src/app/(auth)/login.tsx` (the `email` initial state)

**Interfaces:**
- Consumes: `registerConflictOf` (Task 1) · `useLocalSearchParams`, `router` from `expo-router`
- Produces: `/login` accepts an optional `email` search param. Nothing else consumes it.

**TDD: no** — a runner exists (corrected ENV 8) but has no React renderer, and this task is
navigation and copy with no extractable pure logic. Verified by running the flow.

**Acceptance criteria:**
- **AM-50 AC3 (partial, deliberately)** — registering with a taken email offers **signing in** as a
  real action. Password recovery is named honestly in one sentence and has no control, because
  AM-54 does not exist. *This gap is reported on the ticket, not hidden.*
- **AM-59** — the typed email arrives in the login field already filled. Nobody retypes it, and
  nobody reaches a dead end.
- A taken **username** at submit still lands under the username field (the race-loser case that the
  live check cannot catch), and does not show the email affordance.

- [ ] **Step 1: Extend `register.tsx`'s error handling**

Merge `router` into the **existing** `expo-router` import line rather than adding a second import of
the same module, and add the conflict helper:

```tsx
import { Link, router } from "expo-router";
import { registerConflictOf } from "@/features/auth/conflict";
```

Add one piece of state beside the others:

```tsx
const [emailTaken, setEmailTaken] = useState(false);
```

Replace the mutation's `onError` with:

```tsx
      onError: (error) => {
        const conflict = registerConflictOf(error);
        if (conflict === "email") {
          setEmailTaken(true);
          return;
        }
        if (conflict === "username") {
          setErrors({ username: error.fields?.username ?? "Username ini sudah dipakai." });
          return;
        }
        if (error.fields) {
          setErrors(error.fields);
          return;
        }
        setFormError(error.message);
      },
```

Clear it whenever the email changes, so the panel does not outlive the address it was about — change
the email field's handler to:

```tsx
              onChangeText={(value) => {
                setEmail(value);
                setEmailTaken(false);
              }}
```

Then render the panel immediately after the `AmButton` inside the card:

```tsx
            {emailTaken ? (
              <View
                accessibilityLiveRegion="polite"
                style={{ gap: theme.space[3], marginTop: theme.space[2] }}
              >
                <Text style={[theme.type.body, { color: theme.color.textPrimary }]}>
                  Email ini sudah punya akun.
                </Text>
                <AmButton
                  label="Masuk dengan email ini"
                  variant="secondary"
                  onPress={() =>
                    router.push({ pathname: "/login", params: { email: email.trim() } })
                  }
                />
                <Text style={[theme.type.caption, { color: theme.color.textTertiary }]}>
                  Pengaturan ulang kata sandi belum tersedia di aplikasi.
                </Text>
              </View>
            ) : null}
```

> **Do not turn that last sentence into a button.** There is no reset endpoint, no reset screen, and
> no email sender — AM-54 owns all three. A control whose only behaviour is to announce that it does
> not work is a dead end wearing a button, which is the specific thing AM-59's definition of done
> forbids. Do not invent a support email address either; none exists to invent.

- [ ] **Step 2: Read the param in `login.tsx`**

Add the import and replace the email initial state:

```tsx
import { Link, useLocalSearchParams } from "expo-router";

// …inside the component, above the useState calls:
  const params = useLocalSearchParams<{ email?: string }>();
  const [email, setEmail] = useState(params.email ?? "");
```

`useState`'s initialiser runs once, which is the behaviour wanted: the param seeds the field and
then the person owns it. Do not add an effect that syncs the param back into state — that would
overwrite what they typed on every re-render.

The email travels as a navigation parameter and never leaves the device. It must not be written to
a log, an analytics event, or a crash report.

- [ ] **Step 3: Verify the gate**

```bash
bun run format
make mb-check
```

- [ ] **Step 4: Visual verification (§27 — mandatory)**

1. Register an account. Sign out. Try to register **the same email** again with a different
   username: the panel appears under the button, in both themes.
2. Tap "Masuk dengan email ini" → the login screen opens with the email **already filled** and the
   cursor free for the password.
3. Change the email on the register screen after the panel has appeared: the panel disappears.
4. The username race: two devices (or a manually inserted row) claiming the same username → the
   message lands under the **username** field and no email panel appears.
5. Confirm there is no control anywhere on the path whose destination does not exist.

---

## Task 5: Welcome screen

**Closes:** E1-1 (feature breakdown; **no Jira story exists for this screen** — keep it minimal).

**Brief opener:** Invoke `frontend-design` first. The font/palette gate does **not** apply here —
this surface has a committed design system (`docs/design.md`, `packages/tokens`, and the AM-15 `Am*`
primitives), so `impeccable` refines within it and does not re-open the typeface or the palette.

**Files:**
- **Replace** (the file EXISTS): `apps/mobile/src/app/(auth)/index.tsx` — Plan A shipped a
  placeholder there reading "Belum masuk / Layar masuk dan daftar menyusul", whose own comment says
  Plan B replaces it. Overwrite it wholesale; do not append to it and do not create a second route.

**Interfaces:**
- Consumes: `AmButton`, `useTheme`, `Link`/`router` from `expo-router`, the routes `/login` and
  `/register` (Tasks 2 and 3 — this is why it is serialised behind them: typed routes will not
  compile a link to a route file that does not exist).
- Produces: the route `/` within the `(auth)` group. Nothing consumes it.

**TDD: no** — layout and copy, no logic, and no React renderer in the suite. Verified by opening it.

**Acceptance criteria:**
- Two actions: "Masuk" and "Daftar", both ≥44pt, both reaching a real screen.
- A value proposition of at most three lines that claims nothing the product does not do — no
  community counts, no "ribuan pengguna", no testimonial. The platform launches empty and says so.
- No guest-preview link (decided above — there is no guest surface to preview).
- Renders correctly on the smallest supported phone without scrolling to reach either button.

- [ ] **Step 1: Write `app/(auth)/index.tsx`**

```tsx
import { router } from "expo-router";
import { ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmButton } from "@/components/input";
import { useTheme } from "@/theme";

/**
 * The first screen of the product, and the one most tempting to fill with
 * numbers nobody has yet. The repository rule is that nothing is seeded with
 * fake data and the platform launches empty and says so — so this promises
 * what the app does, and counts nothing.
 */
export default function Welcome() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();

  return (
    <View
      style={{
        flex: 1,
        padding: theme.pagePadding,
        paddingTop: insets.top + theme.space[12],
        paddingBottom: insets.bottom + theme.space[8],
        justifyContent: "space-between",
      }}
    >
      <View style={{ gap: theme.space[4] }}>
        <Text accessibilityRole="header" style={[theme.type.display, { color: theme.color.textPrimary }]}>
          AnakMobil
        </Text>
        <Text style={[theme.type["body-lg"], { color: theme.color.textSecondary }]}>
          Garasi digital buat mobil kamu. Catat servis dan modifikasi, lalu tanya apa pun soal
          mobilmu ke sesama pemilik.
        </Text>
      </View>

      <View style={{ gap: theme.space[3] }}>
        <AmButton
          label="Daftar"
          variant="accent"
          size="lg"
          onPress={() => router.push("/register")}
        />
        <AmButton
          label="Masuk"
          variant="secondary"
          size="lg"
          onPress={() => router.push("/login")}
        />
      </View>
    </View>
  );
}
```

- [ ] **Step 2: Verify the gate**

```bash
bun run format
make mb-check
```

- [ ] **Step 3: Visual verification (§27 — mandatory)**

Screenshot in **light and dark**, on the smallest simulator available and on a large one, at default
and largest system font size. Both buttons must be reachable without scrolling in every combination;
the `AmGround` gradient must show through (if it does not, `_layout.tsx`'s transparent navigation
theme has been disturbed — ENV 6).

---

## Task 6: Sign-out trigger

**Closes:** AM-51 AC4 (the trigger half).

**Brief opener:** Invoke `frontend-design` first. The font/palette gate does **not** apply here —
this surface has a committed design system (`docs/design.md`, `packages/tokens`, and the AM-15 `Am*`
primitives), so `impeccable` refines within it and does not re-open the typeface or the palette.

**Files:**
- Create: `apps/mobile/src/features/auth/SignOutConfirm.tsx`
- **Modify** `apps/mobile/src/app/(app)/index.tsx` — the file EXISTS and already ships a sign-out,
  which changes this task from "add a mount" to "replace an unsafe control". Plan A left
  `<AmButton label="Keluar" variant="secondary" onPress={() => void signOut()} />` on that screen:
  it signs out on a single tap with **no confirmation**, which is precisely what AM-51 AC4 forbids,
  and `void signOut()` swallows a rejection. Swap that one element for `<SignOutConfirm />` and
  drop the now-unused `signOut` import. Leave the rest of that screen alone — its healthcheck body
  and its raw StyleSheet numbers are AM-14's and are not this plan's to churn.

**Interfaces:**
- Consumes: `signOut` from Plan A's session module · `AmButton`, `AmBottomSheet`, `useTheme`
- Produces: `function SignOutConfirm(props: SignOutConfirmProps)` with
  `interface SignOutConfirmProps { readonly label?: string }` — Plan C mounts this in the profile
  tab's settings.

**TDD: no** — a runner exists now (corrected ENV 8), but it cannot render a component, and the
behaviour under test is a native sheet plus a Plan A transaction. Verified by running the app.

> **The sign-out transaction belongs to Plan A and this plan only triggers it.** Plan A's `signOut()`
> increments the auth epoch, cancels in-flight requests, clears the in-memory query cache, awaits the
> deletion of the persisted cache, resets the client stores, wipes secure storage, and performs
> exactly one redirect. **Nothing in this task clears a cache, deletes a token, or navigates.** If
> something appears not to be cleaned up, the defect is in Plan A's transaction and is fixed there —
> adding a second cleanup here is how two owners of one transaction start disagreeing, and it is how
> the next account ends up seeing the previous account's garage.

**Acceptance criteria:**
- **AM-51 AC4** — a confirmation is required (the AC says "When saya mengonfirmasi"), and confirming
  calls `signOut()` exactly once.
- Reopening the app afterwards shows the welcome screen, not the previous user's data. *That
  property is Plan A's to deliver; this task verifies it and reports a failure as a Plan A defect.*
- Double-tapping confirm does not call `signOut()` twice.
- The component is self-contained, so Plan C mounts it in settings with one line and no wiring.

- [ ] **Step 1: Write `SignOutConfirm.tsx`**

```tsx
import { useState } from "react";
import { Text, View } from "react-native";

import { AmBottomSheet } from "@/components/display";
import { AmButton } from "@/components/input";
import { signOut } from "@/shared";
import { useTheme } from "@/theme";

export interface SignOutConfirmProps {
  readonly label?: string;
}

/**
 * The button and its confirmation. Nothing more.
 *
 * signOut() is Plan A's transaction — epoch, cancellation, cache, secure
 * store, one redirect. This component's entire job is to ask once and call it
 * once.
 */
export function SignOutConfirm({ label = "Keluar" }: SignOutConfirmProps) {
  const theme = useTheme();
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  const confirm = () => {
    // Guards the double tap: the sheet stays open for the duration of the
    // transaction, and a second press while `busy` does nothing.
    // NOT a guard against the double tap — `busy` here is this render's stale
    // value, so a second tap landing before the commit reads `false`. The tap is
    // actually stopped by AmButton's `disabled={loading}` once the commit lands,
    // and anything that slips through is absorbed by signOut()'s single-flight
    // `inFlight`: one run(), one POST /auth/logout. (Corrected after Task 6's
    // review — the original snippet had `if (busy) return;` here, which is
    // unreachable on every path, plus a comment asserting it guarded the race.)
    setBusy(true);
    void signOut()
      // Plan A leaves the app signed out either way — setSignedOut() runs in its
      // `finally`. Nothing to say to the person and nothing to retry; without this
      // the rejection is unhandled, which is the exact shape (`void signOut()`)
      // this task replaced. Corrected after Task 6's review.
      .catch(() => {})
      .finally(() => {
      setBusy(false);
      setOpen(false);
    });
  };

  return (
    <>
      <AmButton label={label} variant="destructive" onPress={() => setOpen(true)} />
      <AmBottomSheet visible={open} onClose={() => setOpen(false)} title="Keluar dari akun?">
        <View style={{ gap: theme.space[4], paddingBottom: theme.space[4] }}>
          <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
            Data garasi kamu tetap aman di akun ini. Kamu perlu masuk lagi untuk membukanya.
          </Text>
          <AmButton label="Ya, keluar" variant="destructive" loading={busy} onPress={confirm} />
          <AmButton
            label="Batal"
            variant="secondary"
            disabled={busy}
            onPress={() => setOpen(false)}
          />
        </View>
      </AmBottomSheet>
    </>
  );
}
```

Read `AmBottomSheet`'s actual props before writing this — it is used by `AmSelect` with
`visible`/`onClose`/`title`, which is what this assumes. If it differs, follow the component.

- [ ] **Step 2: Mount it somewhere it can be exercised**

Plan C owns the profile tab and its settings screen. Until it exists, mount this on Plan A's
post-auth placeholder so AM-51 AC4 is verifiable now:

```tsx
// apps/mobile/src/app/(app)/index.tsx — TEMPORARY. Plan C replaces this file
// with the real home screen and moves SignOutConfirm into the profile tab's
// settings.
import { View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { SignOutConfirm } from "@/features/auth/SignOutConfirm";
import { useTheme } from "@/theme";

export default function Home() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  return (
    <View
      style={{
        flex: 1,
        padding: theme.pagePadding,
        paddingTop: insets.top + theme.space[8],
        justifyContent: "flex-end",
        paddingBottom: insets.bottom + theme.space[8],
      }}
    >
      <SignOutConfirm />
    </View>
  );
}
```

If Plan A already created `app/(app)/index.tsx` with content, **add the component to it rather than
replacing the file**, and keep the same TEMPORARY comment on the added lines.

- [ ] **Step 3: Verify the gate**

```bash
bun run format
make mb-check
```

- [ ] **Step 4: Visual verification (§27 — mandatory)**

1. Sign in. Reach the post-auth screen. Screenshot the button in **light and dark**.
2. Tap it → the sheet opens. Screenshot. Tap "Batal" → nothing happens to the session.
3. Tap "Ya, keluar" → the app returns to the welcome screen, through **one** redirect.
4. **Double-tap "Ya, keluar" fast.** `signOut()` must be called once. Watch the network log for a
   single `POST /auth/logout`.
5. Force-close the app and reopen it: the welcome screen, with **no** previous-account data visible
   anywhere. If any appears, that is a **Plan A** defect — file it as such rather than patching it
   here.

---

## Tidak boleh ada

Carried from the spec, narrowed to what this plan could plausibly violate.

- **No second copy of the username rules on the client** beyond the mirror regex. The client never
  decides a name is acceptable on its own; the server's canonicaliser is the authority.
- **No distinguishing an unknown email from a wrong password** — anywhere, ever. Not in UI copy, not
  in a hint, not in a log line, not in an analytics event, not in a "did you mean to register?"
  nudge.
- **No token in MMKV, in the query cache, in a log, or in a URL.** No screen in this plan touches a
  token at all: `signIn(tokens)` receives it and Plan A owns it from there.
- **No second redirect on session expiry**, and no `router.replace()` in a mutation's `onSuccess`.
  The `(auth)` layout owns the redirect.
- **No social login, no email verification, no password reset, no biometrics, no two-factor.** Those
  are AM-52, AM-53, AM-54, AM-77 and stay theirs — including any button that pretends to start one.
- **No invented counts, no seeded data, no fabricated testimonials** on the welcome screen.
- **No new npm/bun dependency beyond `zod`**, which Task 1 installs. Nothing else touches
  `bun.lock`. Never `npm` — `bun add --cwd apps/mobile <pkg>`.
- **No new design value.** Everything through `useTheme()`.
- **No control whose destination does not exist.**

---

## Commits

Commits happen **at the end**, after the owner reviews, never between tasks (repository CLAUDE.md).
One logical change each, each one building:

1. `feat(mobile): add input-behaviour props to AmTextField` — Task 1 Step 2 alone, so the
   design-system change is reviewable on its own.
2. `feat(mobile): add the auth request layer and form schemas` — Task 1 Steps 3–5.
3. `feat(mobile): add the login screen with a uniform error and a rate-limit countdown` — Task 2.
4. `feat(mobile): add the register screen with live validation and a debounced username check` —
   Task 3.
5. `feat(mobile): carry a taken email across to the login screen` — Task 4.
6. `feat(mobile): add the welcome screen` — Task 5.
7. `feat(mobile): add the sign-out confirmation` — Task 6.

Then a pull request into `dev` — `gh pr create --base dev`, the base passed explicitly.

---

## Execution status

- [x] Task 1 — Auth request layer, schemas, and the AmTextField input props. `schemas.ts`,
  `conflict.ts`, `api.ts` created; `AmTextField` gained the five input-behaviour props (its
  `components/input/index.ts` already re-exports `AmTextFieldProps` by name, so they flow through
  with no edit there). `zod@4.4.3` added. **CG-1, CG-2, CG-3 all confirmed held** against Plan A's
  shipped code — the CG-2 409 fallback was correctly NOT implemented. 23 new tests; suite
  `30 pass 0 fail`. Controller re-ran the gate: `bun run format` EXIT=0, `make mb-check` EXIT=0.
  Reviewed on `opus`.
- [x] Task 2 — Login screen (uniform error, rate-limit countdown). `useCountdown.ts` (wall-clock,
  not a decrementing counter), `login.tsx`, 9 tests written failing-first and three of them
  mutation-verified by the writer. Its writer **refused a wrong instruction from the controller and
  was right** — see the `mutation.data` ledger row. Reviewed on `opus`.
- [x] Task 3 — Register screen (live validation, debounced availability, consent gate). Split into
  `passwordScore.ts` + `availability.ts` (pure, testable) and `useUsernameAvailability.ts` +
  `PasswordStrength.tsx` + `ConsentCheckbox.tsx` + `register.tsx`. 22 tests, each confirmed failing
  before implementation. Controller re-ran the gate: `format` EXIT=0, `mb-check` EXIT=0,
  `61 pass 0 fail`. Reviewed on `opus`.
- [x] Task 4 — The already-registered path. `emailTaken` + the conflict panel in `register.tsx`,
  the seeded email on `login.tsx`. The reset-password half is one honest sentence with no control,
  because AM-54 does not exist. Reviewed on `opus`.
- [~] Task 5 — Welcome screen. **Built, then DELETED on the owner's call, 2026-08-21.** `(auth)/index.tsx`
  is now a one-line `<Redirect href="/login" />`. The reason is worth keeping: the screen asked for a
  tap without answering anything — a person opening the app either has an account or does not, and
  both need the same next screen. It was also the least honest surface in the app, describing
  features that are not built. Its two controls now live where they belong: sign-in is the screen,
  and register is a link on it. Originally **written inline by the controller** (all four inline tests held:
  nothing else was ready, one file, `TDD: no`, and it touches nothing on the floor list). Replaced
  Plan A's placeholder. Controller gate: `format` EXIT=0, `mb-check` EXIT=0, `61 pass 0 fail`.
  Reviewed on `sonnet` — an inline-written task is never folded into a batch.
- [x] Task 6 — Sign-out trigger. `SignOutConfirm.tsx` created; `(app)/index.tsx`'s unconfirmed
  one-tap `AmButton` REPLACED (see the pre-exec `structural` ledger row). `AmBottomSheet`'s real
  props (`visible`/`onClose`/`title`/`children`) matched the plan's assumption exactly, as did
  `AmButton`'s `"destructive"` variant. One deviation from the plan's literal snippet: it imported
  `signOut` from `"@/shared/session"`, which is **not a valid module path** — no such barrel exists.
  Corrected to `"@/shared"`. Reviewed on `opus`.

---

## Review findings ledger

*Empty at plan time. Severity vocabulary: `structural` (raise and fix now — a column, a constraint,
or a public contract) · `correctness` · `test-integrity` · `hygiene`.*

**Consolidated 2026-08-21, and the pass earned its place.** Four Task 6 findings were recorded as
fixed because the PLAN's snippet had been corrected — but the snippet is not the code, and
`SignOutConfirm.tsx` still carried all four live: the unreachable double-tap guard, the swallowed
rejection, the unguarded `onClose`, and the missing `TEMPORARY` marker. Reading the rows back
against the files is what caught it. Two more rows were stale in the other direction (fixed, never
written back), and one proposed deleting a module that Task 4 had since made load-bearing.

| Task | Severity | File:line | Failure scenario | Smallest fix | Closed how |
|---|---|---|---|---|---|
| pre-exec | `hygiene` | plan ENVIRONMENT item 8 | The card said "apps/mobile has NO test runner and this work does not add one". Plan A added one (`bun test test/`, in `mb-check` and CI). Every task inherited a `TDD: no` justified by an absence that no longer holds, so Task 1's pure functions — the username regex, the password bands, `fieldErrorsOf`, `registerConflictOf` — would have shipped with no test that can fail. | Rewrite item 8 with what actually exists, including the limit that matters: no React renderer, so components still cannot be tested. | Fixed in the plan before task 1 was dispatched; Task 1's verdict flipped to `TDD: yes` for `schemas.ts` and `conflict.ts`. |
| pre-exec | `correctness` | plan Task 5 "Files: Create `app/(auth)/index.tsx`" | The file already exists — Plan A shipped a placeholder there. A writer told to *create* it either fails on a collision or, worse, adds a second route file next to it. | Say **replace**, and name what is being replaced. | Fixed in the plan. |
| pre-exec | `structural` (product) | `apps/mobile/src/app/(app)/index.tsx` | Plan A left `<AmButton label="Keluar" onPress={() => void signOut()} />` on the post-auth screen: a **single tap signs out with no confirmation**, which is exactly what AM-51 AC4 ("When saya mengonfirmasi") forbids, and `void signOut()` swallows a rejection. Task 6 was briefed as "add a mount", which would have left the unsafe control sitting next to the safe one. | Task 6 replaces that element with `<SignOutConfirm />` rather than adding beside it. | Fixed in the plan before task 6 was dispatched. |
| pre-exec | `correctness` | plan CG-2 "Fallback if Plan A kept 409" | The plan offered two readings and told Task 1 to pin one. Plan A in fact routes a 409-that-names-a-field to `kind: "validation"` already, so the fallback (adding `code?: string` to `ApiError`) would have been a needless widening of a frozen contract. | Record the verified answer with file:line and delete the choice. | Fixed in the plan; the fallback is marked not-needed. |
| pre-exec | `correctness` | `apps/mobile/src/shared/api/errors.ts` | A 401 from `/auth/login` is a **wrong password**, and `toApiError` maps every 401 to `kind: "unauthorized"`. A login screen that treats that kind as "session expired" would sign out or redirect on a typo. | Stated in the plan and in both screen briefs. | Recorded; Task 2 must honour it. |
| pre-exec | `test-integrity` | plan, 5 `TDD:` verdicts | **Systemic, not one instance.** Every TDD verdict in the plan was justified by "no runner in `apps/mobile` (ENV 8)" — the same phantom absence as the row above. Task 2's countdown is arithmetic over two timestamps and Task 3's `strengthOf` and availability state machine are pure functions the plan *itself* identified as "the pure part"; all three would have shipped with nothing that could fail. A countdown wrong at a boundary and a `strengthOf` counting bytes instead of characters are both invisible on screen. | Re-derive each verdict from what is actually true: a runner exists, it has **no React renderer**, so screens stay test-after and the pure logic becomes test-first. | Fixed in the plan. Tasks 2 and 3 dispatched with `TDD: yes` for their pure parts and explicit failing-test-first instructions; Tasks 4 and 5 stay `no` but no longer cite a false reason. |
| Task 1 | `correctness` | plan Task 1 Steps 4-5, and Plan D ×5 | The plan's own code snippets import from `@/shared/api` and `@/shared/session`. **Neither module path exists** — Plan A ships exactly one barrel, `apps/mobile/src/shared/index.ts`, whose header says Plans B/C/D import from `@/shared` and nowhere else. Tasks 1 and 6 hit this independently, minutes apart. Plan D carried the same bad path five times and would have hit it again. | Rewrite every such import to `@/shared`, in Plan B **and** Plan D. | Fixed in both plan files, with a note in Plan D saying why. Plan C was checked and was already clean. |
| Task 1 | `hygiene` | plan Task 1 Step 6 | Step 6 required "`git diff --stat bun.lock` must be empty" while Step 4 of the same task mandates `bun add --cwd apps/mobile zod`, which necessarily changes it. A writer reading Step 6 literally fails its own task. The same contradiction sat in Global Constraints and in `Tidak boleh ada`. | State the real invariant: `bun install --frozen-lockfile` stays EXIT=0, i.e. nothing drifted *beyond* the sanctioned zod add. | Fixed in all three places. |
| Task 6 | `test-integrity` | `SignOutConfirm.tsx:26-28` | The `if (busy) return;` double-tap guard **is unreachable on every path**. Committed `busy === true` → `AmButton` is already `disabled` and `onPress` never fires. Committed `false` → the check is false. The pre-commit race the comment names → the second tap runs the previous render's closure where `busy` is still `false`, so the check is false there too. The AC holds, but for two reasons this line is not: `AmButton`'s `disabled={loading}`, and `signOut.ts:112`'s module-level single-flight `inFlight ??= run()`. A later edit that swaps the spinner treatment, trusting this comment, would let two `confirm` calls through. | Delete the line; rewrite the comment to name the two real guards. **The line came from the plan's own Step 1 snippet** — fix the snippet too. | **Closed** in the fix pass. |
| Task 6 | `hygiene` | `SignOutConfirm.tsx:30` | `void signOut().finally(…)` **still swallows the rejection this task existed to stop swallowing** — `.finally()` returns a promise that adopts the rejection and `void` discards it, with no `.catch` anywhere. `signOut()` really can reject: `signOut.ts:75-101` is `try`/`finally` with no `catch`, and its own comment names an MMKV throw from `purgeAllPersistedCache()` and a rejecting `cancelQueries()`. The person *is* signed out (`setSignedOut()` runs in the `finally` first), so no stuck spinner and no false success — the cost is an unhandled rejection, and a pre-exec ledger row that reads as closed while the shape it named survived. | `.catch(() => {})` before `.finally(...)`. Same snippet origin as the row above. | **Closed** in the fix pass. |
| Task 6 | `hygiene` | `apps/mobile/src/app/(app)/index.tsx:77` | The plan's Step 2 requires a `TEMPORARY` marker on this mount and Open question #2 records that Plan C moves it to profile settings. There is none (`grep TEMPORARY` → no hits). **Cross-plan cost:** Plan C's writer mounts `SignOutConfirm` in settings, sees no signal that line 77 is provisional, leaves it — and the app ships **two destructive sign-out controls, one of them on a healthcheck screen**. | Add the two-line comment the plan already wrote. | **Closed** in the fix pass. |
| Task 6 | `hygiene` | `SignOutConfirm.tsx:39` | `onClose` is unconditional while "Batal" is `disabled={busy}`, and the component's own comment claims "the sheet stays open for the duration of the transaction". `AmBottomSheet` has four other dismissal paths — scrim tap, "Tutup" header button, a >96pt drag, Android hardware back — all routing to the unconditional `onClose`. Sequence: confirm → drag down → sheet closes with `busy` still true → the outer trigger is not gated on `busy`, so reopening shows a sheet whose only two buttons are both greyed out until the ~5s abort bound elapses. Nothing breaks; the component contradicts itself. | `onClose={() => { if (!busy) setOpen(false); }}` | **Closed** in the fix pass. |
| Tasks 2-3 | `correctness` (product) | `apps/mobile/src/shared/api/errors.ts`, `toApiError` 409 branch | **Measured against the running API, not assumed.** A taken email answers `409 {"error":{"code":"conflict","message":"Data ini sudah berubah. Muat ulang, lalu coba lagi.","details":{"email":"Email ini sudah terdaftar."}}}`. `toApiError` copies the TOP-LEVEL message onto `ApiError.message`, so a screen that renders `error.message` as a banner — the natural thing to do — tells somebody whose email is already registered *"This data has changed. Reload and try again."* That breaks AM-50 AC3 and AM-57. Separately, a **missing required field** returns axum's plain-text 422 with **no envelope**, which parses to `kind: "validation"` with `fields: undefined` and the generic server message — so a screen that only renders `fields` shows **nothing at all** and the button appears dead. | One rule covering both: on `kind: "validation"` **with** `fields`, render the field messages and suppress `message`; **without** `fields`, render `message` as a form-level banner. | Sent to both writers mid-flight; to be confirmed in their reports. |
| Task 1 | `correctness` | `features/auth/api.ts`, `useRegister`/`useLogin` | **`isError` does not mean the request failed.** TanStack Query awaits `onSuccess` and routes its rejection into the mutation's error state, and `signIn` writes the token pair to SecureStore *before* calling `fetchMe()`. So: `201 Created` → tokens on disk → `fetchMe()` rejects offline → mutation shows `kind: "offline"` → the screen offers "coba lagi" → a **second** `POST /auth/register` → `409 "Email ini sudah terdaftar."` The person is told their own thirty-second-old email is taken while valid credentials sit on the device. On login the same path mints a second server session alive to the refresh token's full TTL. No automatic retry is involved — `queryClient` sets no `mutations` block, so v5's default of 0 applies; the retry is a human finger, which is worse because the UI invites it. `signIn.ts:41-45` warns about exactly this, addressed at this file, and nothing carried it forward. | Doc comment at source naming the hazard; the screens retry with `signIn(mutation.data)` once `data` exists, never by re-submitting. | **Closed.** Doc comment added to `api.ts`; sent to both screen writers mid-flight. |
| Task 1 | `correctness` | `features/auth/schemas.ts:28-32` | **The mirror was not a mirror — it rejected names the server accepts, with a message describing a rule the person did not break.** `username.rs:78` does `raw.trim().to_ascii_lowercase()` and only *then* validates; the client validated the raw string. Measured: `Oksa` → server accepts as `oksa`, client rejects with "Awali dan akhiri dengan huruf atau angka" (the problem is a capital letter, which the message never mentions); `"  Budi  "` → same. This is the "client never decides a username is acceptable" constraint in its inverted form. The plan's boundary table ratified it on the grounds that "the field lowercases as you type, so this is only reachable by paste" — but paste from a password manager is the normal path, and no task anywhere planned a `trim`. | `.trim().toLowerCase()` before `.min(3)`, reproducing `canonicalise`'s exact order. | **Closed.** Fixed in `schemas.ts`; a new test pins `Oksa`→`oksa`, `"  Budi  "`→`budi`, and that canonicalising does not soften `ok`, `Ok..Sa`, `.budi`. |
| Task 1 | `test-integrity` | `test/auth-schemas.test.ts:73-83` | A test named "fieldErrorsOf keeps only the first message per field" **could not fail**. Verified at runtime against zod 4.4.3: `z.string().min(3).max(5)` against `"a"` raises exactly **one** issue (`.max(5)` passes), so the `!(key in out)` dedup guard was never reached. Deleting the guard left the test green. Its own comment claimed the opposite. | Move the assertion to the sibling fixture that genuinely raises two issues on one path (`username: "no"` fails `.min(3)` **and** the regex) and assert the exact first message; delete the redundant test. | **Closed.** `expect(errors.username).toBe("Minimal 3 karakter.")` now pins first-wins; suite `22 pass 0 fail` on that file. |
| Task 1 | `correctness` (low) | `features/auth/api.ts:30-36` | `body: input` shipped whatever object it was handed. TypeScript's excess-property check fires only on fresh literals, so `mutate(parsed.data)` — `registerSchema`'s output, which **includes `consent: true`** — compiled and put a fourth field on the wire. Not exploitable (the server's `RegistrationRequest` is an explicit three-field DTO and serde ignores unknown keys, and there is no role/verified/balance field to smuggle), but the client's payload was not pinned to the frozen contract, so the next field added to a form would ship silently. | Destructure, so the wire shape is provable by reading the function. | **Closed.** Both request functions destructure. |
| Task 1 | `hygiene` | `features/auth/conflict.ts:16-17` | Keyed on truthiness, not presence. `stringFields` admits an empty string, so `details: {username: ""}` → falsy → `registerConflictOf` returns `null` → **no conflict shown at all** for a taken username. Not reachable against today's backend (its messages are non-empty consts) and inconsistent with `fieldErrorsOf`, which correctly uses `key in out`. | `!== undefined`. | **Closed.** |
| Task 1 | `hygiene` | `features/auth/api.ts:48` | `encodeURIComponent` closes the injection-shaped hole but **does not neutralise dot segments**: `encodeURIComponent("..") === ".."`, so `usernameAvailability("..")` builds a path that `fetch`'s URL parser normalises to `${BASE_URL}/availability` → 404 on the same origin, no session attached, nothing forgeable. A guard *in* the function would make the client decide a name is unacceptable, which the spec forbids. | One sentence in the doc comment; callers gate on `USERNAME_PATTERN`, which is where it belongs. | **Closed.** |
| Task 3 | `correctness` | `test/auth-register.test.ts`, plan Task 3 file layout | **The plan's single-file layout does not survive this test runner.** Importing `PasswordStrength.tsx` (react-native) or the original `useUsernameAvailability.ts` (value-imports `./api` → `@/shared` → `react-native-mmkv`/`expo-router`) crashes bun's transpiler on react-native's Flow-typed internals — the same landmine `session.test.ts`'s own header comment already documents. Left as designed, the pure logic would have been untestable and the `TDD: yes` verdict unmeetable. | Split the pure logic into `availability.ts` and `passwordScore.ts` with zero heavy imports; the component and hook files import from and re-export them. | **Closed** by the writer. Worth carrying into Plans C and D: any pure function that needs a test must live in a file that imports no react-native. |
| Task 3 | `correctness` | `features/auth/passwordScore.ts` | **Case-collision on a case-insensitive filesystem.** The pure file was first named `passwordStrength.ts`, which on macOS's default APFS collides with `PasswordStrength.tsx` — the import silently resolved to the wrong file, still pulled in react-native, and still crashed the runner. Two files whose names differ only by case are one file to the loader. | Rename to `passwordScore.ts`. | **Closed** by the writer, self-caught. |
| Task 3 | `correctness` | `features/auth/useUsernameAvailability.ts` | ESLint's `react-hooks/set-state-in-effect` rejected the first design, which called a reducer synchronously at the top of the effect body. The redesign **derives** the state at render (`deriveAvailability(enabled, username, resolution)`) and only ever `setState`s inside the async `.then`/`.catch`. Better model as well as a passing lint: a stale or aborted resolution, tagged with a superseded username, is simply ignored by the derivation rather than needing an explicit "aborted" event. | Derive rather than reduce. | **Closed** by the writer. |
| Tasks 2-3 | `correctness` | `features/auth/api.ts`, both screens | **The controller's own first fix was dead code, and Task 2's writer caught it by reading the package instead of implementing what it was told.** The instruction was "once a mutation has `data`, retry with `signIn(mutation.data)`". But query-core@5.101.4's reducer does `case "error": return { ...state, data: void 0, ... }` (verified by the controller at `node_modules/.bun/@tanstack+query-core@5.101.4/.../mutation.js:239-248`), so `mutation.data` is **unconditionally `undefined`** whenever `isError` is true. Task 2 refused to implement it and documented why; **Task 3 implemented it**, leaving `register.tsx` with a retry branch that could never run — worse than absent, because it read as handled. There is also no other signal separating "the POST never landed" from "the POST landed and only `/me` failed": both are `kind: "offline"`. | The pair rides the rethrown error instead. `finish()` in `api.ts` wraps `signIn` and rethrows `{ ...error, tokens }`; `isSignInFailure(error)` narrows it; both screens hold it in `pendingTokens` and the next tap resumes rather than re-submits. | **Closed.** `api.ts` gained `finish`/`SignInFailure`/`isSignInFailure`; `register.tsx` switched from `register.data` to the error-carried pair; `login.tsx` gained the resume path and a "Lanjutkan" button label. Four tests added in `test/auth-signin-failure.test.ts`. Suite `65 pass 0 fail`. |
| Task 5 | `correctness` | `app/(auth)/index.tsx`, plan Task 5 snippet | **At a large system font size the "Masuk" button slides off the bottom and becomes unreachable.** The container was a plain `View` with `justifyContent: "space-between"`, which does not shrink non-flex children. On a 320×568pt device the title wraps to two lines and the body copy roughly doubles in height; the overflow has nowhere to go, and there is no `ScrollView`. `allowFontScaling` is correctly never disabled, so this is reachable by any person who has enlarged their system text. The AC — "renders without scrolling to reach either button" — was being read as "must not scroll" when it means "must not require scrolling at normal scale". `login.tsx` and `register.tsx` already use `ScrollView` for exactly this; the welcome screen was the odd one out. **The `View` came from the plan's own snippet**, not from the controller. | `ScrollView` with `contentContainerStyle={{ flexGrow: 1, ... }}` — identical at normal scale, scrollable once it stops fitting. | **Closed** in the file **and** in the plan's snippet. |
| Task 5 | `hygiene` | `app/(auth)/index.tsx`, plan Task 5 snippet | The value-prop copy was ~139 characters, which at `body-lg` in a 288pt column is about four lines — the AC says at most three, before any font scaling. | Shortened to two sentences that still claim nothing the product does not do. | **Closed** in the file and in the plan's snippet. |
| Task 5 | `hygiene` | `app/(auth)/index.tsx:57,63` | A fast double-tap on "Daftar" can push two Register screens, so one back-press does not return to Welcome. Shared with the sibling screens' `Link` navigations rather than introduced here; the cost is an extra back-press, not data loss. | A `disabled`-while-navigating guard, if it ever proves annoying. | Open — accepted, not worth the state. |
| Task 3 | `test-integrity` | `test/auth-register.test.ts:101-120`, `useUsernameAvailability.ts:60` | **The security property the suite is named for had no coverage.** Three byte-identical tests, named for a 429, a 5xx, and offline, all called `deriveAvailability` with an outcome **already** equal to `"unknown"` — exercising a pass-through two earlier tests already cover. The mapping they were named for lived inside the hook, which no test can import (it reaches `./api` → `@/shared` → `react-native-mmkv`). Concretely: change the hook's failure outcome to `"taken"` and **all 22 tests stayed green** — a CGNAT-shared 429 would then tell somebody a free name is taken and block their registration. | Lift `outcomeOfResult()` and `OUTCOME_ON_FAILURE` into `availability.ts` (zero imports) and pin both. | **Closed and mutation-verified**: with `OUTCOME_ON_FAILURE` flipped to `"taken"` the suite now reports `1 fail`; restored, `20 pass 0 fail`. |
| Task 3 | `test-integrity` | `test/auth-register.test.ts:63-67, 76-78`, `passwordScore.ts:12-14` | **Two tests could not fail, and the claim under them describes a failure mode JavaScript does not have.** The source holds `"é"` as U+00E9 **precomposed — one code point** (verified in the bytes: `c3 a9`), so `[...s].length` and `s.length` both give 8; deleting the spread from `strengthOf` left them green, and they duplicated the plain `"a".repeat(8)` case. A decomposed `"e\u0301"` would not discriminate either (2 code points **and** 2 UTF-16 units). The comments spoke of a "byte count", but `.length` counts **UTF-16 code units** and JS has no byte-count operator — framing that came from the controller's own brief and had propagated into `passwordScore.ts`. Only the emoji case genuinely diverges. | Delete both accented tests; keep the emoji one; correct the framing in both files. | **Closed.** |
| Task 3 | `correctness` | `features/auth/api.ts`, `finish()` | **The controller's own fix dropped the error message.** `Error.prototype.message` is an own property but **non-enumerable**, so `{ ...new Error("Keystore unavailable") }` is `{}`. `signIn` has two steps that reject with real `Error`s rather than taxonomy literals — `clearActiveVehicle()` through MMKV and `writeSession()` through expo-secure-store; only `fetchMe()` rejects with an object. On a Keystore failure the screen received `message === undefined`, the banner was falsy, and **nothing rendered** — the person tapped a button that visibly did nothing. It self-corrected on the second tap, which is worse, not better. | An `asApiError()` normaliser before the spread. | **Closed**, with three tests including one that pins the bare-spread defect itself. |
| Task 3 | `correctness` | `app/(auth)/register.tsx:134, 185-186` | **A stale server field error is never cleared and permanently suppresses the availability hint.** `errors` is written only by `submit()` and cleared only by another `submit()`. So: availability endpoint down → `unknown`, button enabled → submit `budi`, which is genuinely taken → 409 sets `errors.username` → the person types `budi.satya` → the message *"Username ini sudah dipakai."* still shows under a name that is not `budi`, the hint is suppressed so `"Username ini tersedia."` can never render even once the endpoint recovers, and the button stays enabled. The screen says the name is taken and lets them submit it. Same shape for `email` and `password`, minus the hint suppression. | Clear a field's entry from `onChangeText`. | **Closed** in the fix pass, once Task 4 released the file. |
| Task 3 | `hygiene` | `app/(auth)/register.tsx:84-107` | After a `SignInFailure` the form stays editable but every field is ignored: `canSubmit` is forced true and `submit()` always takes the resume branch. Somebody who reads the offline banner, decides their email was wrong, edits it and taps "Daftar" is signed in under the **original** email with no sign the edit was discarded. The label still reads "Daftar", which now means "resume". | Label it "Lanjutkan" while `pendingTokens !== null`, as `login.tsx` already does. | **Closed** in the fix pass, once Task 4 released the file. |
| Task 3 | `hygiene` | `useUsernameAvailability.ts:34-35` | The comment said the AbortController is "built and **armed** by hand". Nothing arms it — there is no `setTimeout(() => controller.abort(), …)`, unlike every other hand-built controller in the repo (`signOut.ts:53`, `bootstrap.ts:89`, `refresh.ts:96`). On a black-hole network the hint reads "Memeriksa ketersediaan…" until the next keystroke. Nothing is blocked (`disablesSubmit("checking")` is false), so the real cost is the comment: a reader trusting "armed" will not add the bound. | Say what is actually true. | **Closed** — comment corrected; no bound added, deliberately, since nothing is blocked. |
| Task 3 | `hygiene` | `features/auth/conflict.ts` | `registerConflictOf` has **no production consumer**. Its own doc comment says it exists "so no screen encodes the choice itself", but `register.tsx` reads `error.fields` directly — which is the better code, because `setErrors(error.fields)` covers both fields at once and the `"email" \| "username" \| null` return cannot. Only the test imports it. | Delete the module and its three tests rather than manufacture a consumer. | **Superseded** — Task 4 landed and IS the consumer (`register.tsx:9`, `:124`). The deletion was correctly never performed; see the row below. |
| Task 2 | `correctness` | `shared/session/signIn.ts`, both screens | **The resume path could make the server revoke every session on every device.** `signIn` starts with `writeSession`, so calling it again to resume rewrites the pair the screen is holding. Chain: `POST /auth/login` → 200 with **P1**, `writeSession(P1)` → `fetchMe()` 401s (clock skew on `iat`/`nbf`, or any first-request rejection) → `apiRequest`'s refresh branch now sees a stored session, rotates, and writes **P2**, spending P1's refresh token → the retried `/me` fails too → the screen holds P1 → the person taps Lanjutkan → `signIn(P1)` **overwrites the live P2 with the spent P1** → the next 401 presents a spent refresh token, which the server reads as reuse and answers by revoking every session on every device. Low probability, and exactly the failure class the session layer is written at length to prevent. Both screens had it; the design was the controller's. | `resumeSignIn(tokens)` in Plan A's session module: skip `writeSession` when the disk already holds a pair, since a resume by definition follows a successful write. | **Closed.** Added to `signIn.ts`, exported from the barrel, both screens switched. |
| Task 2 | `correctness` | `app/(auth)/login.tsx` | `pendingTokens` was `useState`, written once and never cleared, so a resume that can never succeed — a Keystore invalidated by a biometric re-enrolment — left the button reading "Lanjutkan" forever with **no path back to an ordinary login**, and typing a different password was silently ignored because `submit()` returned before reading the fields. `register.tsx` derived it from the mutation from the outset; this screen was the outlier. | Derive it from `login.error` the way the sibling does. | **Closed.** |
| Task 2 | `correctness` | `app/(auth)/login.tsx` | The resume's catch was the **one call site that skipped `asApiError`** — an unguarded `(error as ApiError).message`. Same defect as the `finish()` one, at the third site: an MMKV or Keystore rejection has a non-enumerable `message`, the banner is falsy, and nothing renders. "The fix landed at two of three call sites." | Export `asApiError` and use it; `register.tsx`'s hand-rolled `messageOf` duplicate deleted with it. | **Closed.** |
| Task 2 | `test-integrity` | `test/auth-signin-failure.test.ts` | **The test restated the contract instead of importing it**, so it would not catch a regression: renaming `AuthTokens`' fields, typoing `isSignInFailure`'s key, or dropping `asApiError`'s guard all left every assertion green. The header blamed `api.ts`'s react-native reach, which is real — but does not apply to those symbols, which are pure. | Move them to `features/auth/signInFailure.ts` (only a type-only import), re-export from `api.ts`, import the real ones. | **Closed and mutation-verified**: the `"token" in error` typo and a disabled `asApiError` guard now each kill 3 tests; both previously passed. |
| Task 2 | `hygiene` | both screens' footer `Link` | ~18pt tap target against this plan's own ≥44pt floor. The `Am*` primitives apply `minHeight` after the caller's style so it cannot be defeated — a bare `Link` is not one of them. | The review suggested `hitSlop`; **that is not on expo-router's `LinkProps`** and fails `tsc`. Vertical padding on the underlying `Text` grows the touchable region instead: 18 + 2×16 = 50. | **Closed**, by a different mechanism than the one suggested. |
| Task 2 | `hygiene` | `test/auth-countdown.test.ts` | A comment claimed a test caught a `>` vs `>=` slip in the clamp. It does not — that mutant survives, and is behaviourally equivalent, because `Math.min(0, total)` is 0 for every `total >= 1`, which `normalizeSeconds` guarantees. The other half of the sentence (`Math.ceil(x) + 1`) is correct. | Delete the wrong half. | **Closed.** |
| Task 4 | `correctness` | `app/(auth)/register.tsx` | **The conflict panel asserted something false during an in-flight edit.** `emailTaken` was a boolean bound to nothing: the email field stays editable while the POST is in flight (only the button is gated), so correcting a typo mid-request and then receiving the 409 for the OLD address showed "Email ini sudah punya akun." about an address with no account — and "Masuk dengan email ini" seeded login with the NEW one, which then answers "Email atau kata sandi salah." Told to sign in to an account that does not exist, then told the password is wrong. | Hold the **address** the server refused, not a flag, and render on `emailTaken === values.email`. | **Closed.** |
| Task 4 | `correctness` | `app/(auth)/login.tsx` | **A crafted deep link crashed the login screen's primary action.** `useLocalSearchParams<{ email?: string }>()` is a type *assertion*; expo-router passes arrays straight through (`build/hooks/useLocalSearchParams.js`). `app.config.ts` sets `scheme: "anakmobil"`, so `anakmobil://login?email=a&email=b` delivered `["a","b"]` into a `readonly value: string` prop, and the first tap on Masuk hit `email.trim is not a function`. | Removed the route parameter entirely — see the row below, which closes both. | **Closed.** |
| Task 4 | `correctness` (policy) | `app/(auth)/register.tsx` → `login.tsx` | The email travelled in a **route parameter**. On native that is clean today — `grep` for `console.`/`Sentry`/`analytics`/`track(` over `apps/mobile/src` returns **zero hits**, and there is no address bar. But `app.config.ts` declares `web: { output: "static" }` and `package.json` ships a `web` script, and on that target it renders as `/login?email=oksa%40example.com` — in the address bar, in history, in the back/forward cache, and in the `Referer` of any later cross-origin request. The repository's standing rule is that personal data never goes in a URL parameter or query string, and that build path is one command away. | An out-of-band handoff: `features/auth/pendingEmail.ts`, read-once, module scope (one producer, one consumer, no render depends on it). `router.push("/login")` with no params. | **Closed**, with three tests. Also closes the deep-link crash above: with no param, there is nothing to craft. |
| Task 4 | `hygiene` | the `conflict.ts` ledger row above | That row said `registerConflictOf` had no production consumer and should be deleted "held until Task 4 lands". **Task 4 landed and is that consumer** (`register.tsx:9`, `:124`). A fix pass working the ledger in order would have deleted it and broken the gate — or worse, "repaired" it by inlining `error.fields?.email` and silently losing the presence-not-truthiness correction. | Close the row rather than act on it. | **Closed as superseded** — the deletion was correctly never performed. |
| env | `hygiene` | `Makefile`, `mb-run-dev` | `pod install` crashed inside CocoaPods' own **error reporter** (`Encoding::CompatibilityError` in `unicode_normalize`), which masks whatever the real error was. Cause is a non-UTF-8 locale; the target does not set one. | `LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8` — with it, `pod install` is EXIT=0 and installs all 114 pods. | **Closed.** `LANG`/`LC_ALL=en_US.UTF-8` on all three `mb-run-*` targets, with a comment saying why. |
| redesign | `correctness` (product) | `app.config.ts`, splash | **The app opened bright blue on a graphite product.** The native splash was `#208AEF` — a value that appears nowhere in `packages/tokens`, whose brand is graphite `#0F141A`–`#3C4550` plus orange `#ED491C`. So every cold start flashed an unbranded blue and then snapped to near-black. It also carried no wordmark, so the product's own name was never on screen at launch. | Splash background to `#0F141A` (`brand.950`), regenerated through `expo prebuild` rather than by hand-editing the generated `SplashScreenBackground.colorset`, which the next prebuild would overwrite. | **Closed.** Verified: the colorset now reads rgb(15, 20, 26). |
| redesign | `correctness` (product) | `app/_layout.tsx` | The native splash held until **fonts AND the session** resolved, so a cold start with a stored session sat on a bare mark for a whole `/me` round trip — and a native splash is one static PNG that cannot be given a wordmark without a new asset. Nothing in-app could improve it because nothing in-app was reached. | Hide the native splash once **fonts** are ready (they must land first, or the wordmark would draw in the system face and reflow when Inter arrives) and carry the remaining wait in an in-app `LaunchView` that continues it: same ground, same mark, same 76pt width, same centre, plus the wordmark. No spinner — there is nothing to act on and a spinner under a logo reads as "something is wrong". | **Closed.** |
| redesign | `correctness` (a11y) | `components/input/AmButton.tsx` | **A disabled accent button's label was barely legible**, and it is the first thing anyone sees on the registration screen, whose CTA is disabled until the form is complete. The blanket `opacity: 0.45` worked for the graphite variants (white on graphite survives halving) but `#ED491C` at 45% over the graphite ground is a muddy brown, and its label is `onAccent` — graphite-950, near black — at the same opacity. `docs/design.md` specifies no disabled treatment, so the opacity was an implementer's choice rather than committed spec. | Re-colour when disabled (`surfaceSubtle` fill, `textTertiary` label, `border`) instead of fading. `loading` deliberately keeps the variant's own colours — a button mid-request should look like itself with a spinner, not switched off. | **Closed.** Design-system change: it affects every disabled button in the app, which is the point. |
| redesign | `hygiene` | both auth screens | The `AmCard` wrapper around each form drew a `#29313A` border on a `#0F141A` ground — barely visible — and indented the fields away from the page gutter for no structure in return. The fields carry their own borders. | Remove it. | **Closed.** |
| redesign | `hygiene` | both auth screens | The screens were top-anchored with no bottom anchor, so the register/login link floated mid-screen above a half-screen void — visible in the owner's own screenshot. | `flexGrow: 1` on the scroll content plus `marginTop: "auto"` on the footer: pinned to the thumb zone on a tall screen, flowing right after the form on a short one or at a large font size. | **Closed.** |
| redesign | `hygiene` | `app/(auth)/login.tsx` | The sign-in button used the graphite `primary` default, so the screen's only action rendered grey — and disabled-on-load it read as a dead control. The design system reserves accent for "the strongest brand CTA, used selectively", and a screen whose entire job is one action is exactly that. | `variant="accent"`. | **Closed.** |
| redesign | n/a | — | **Not a defect:** the floating gear in the owner's screenshots is the **Expo dev-client menu button**, not application UI. It does not exist in a preview or production build. Nothing was changed for it. | — | Recorded so it is not "fixed". |
| redesign | `hygiene` (a11y) | `components/display/AmBrandLockup.tsx` | At the largest accessibility text size the uncapped 32px wordmark wrapped to two lines and took **two thirds of the screen**, pushing the form somebody actually opened the app for below the fold. Verified on device, not reasoned. | `maxFontSizeMultiplier={1.4}` on the wordmark ONLY — a cap, not `allowFontScaling={false}`, which stays banned. It still grows to ~45px; it just stops the logo becoming the interface. Every other string on these screens scales uncapped. | **Closed.** Re-verified at `accessibility-extra-extra-extra-large`: wordmark on one line, both fields and the CTA on screen without scrolling. |
| redesign | n/a | — | **Not a defect, and it cost an hour to establish:** the full-screen blue in the owner's first screenshot is the **Expo dev-client's own launcher**, not this app's splash. The dev client ships a blue icon and a blue "Searching for development servers…" screen, and it is what runs in a `development` build. Confirmed by catching it mid-launch. The app's own splash was separately off-brand and has been fixed; a `preview` or `production` build shows neither the blue launcher nor the gear. | — | Recorded so the next person does not chase it. |
| redesign | `correctness` (brand) | `app.config.ts`, `AmBrandLockup`, `apps/mobile/assets/images/` | **The mobile app was not using the AnakMobil logo anywhere.** `splash-icon.png` and `icon.png` are a blue chevron — a leftover placeholder, not the brand mark — and they were standing in for it on the splash, the app icon, and (briefly) the auth screens. The real mark ships in `@anakmobil/assets`: a graphite garage with an "AM" and an orange road, plus an "AnakMobil**.id**" wordmark, with `favicon-dark.png` / `favicon-light.png` as a purpose-built theme pair. The owner spotted it; nothing in the repo would have. | `AmBrandLockup` imports the theme-appropriate mark **through the package** (`@anakmobil/assets/img/…`), as that package's README asks, and sets the wordmark in the product's own display face so it inherits theme colour and text-size settings. `.id` is orange because the brand's own lockup sets it that way. The native splash gets a build-time copy — an Expo config plugin cannot import from a workspace package. `@anakmobil/assets` added to the mobile app's dependencies. | **Closed.** Verified in both themes on device. |
| redesign | `correctness` (brand) | `app.config.ts` `ios.icon`, `assets/expo.icon`, `assets/images/*` | **The app shipped Expo's own logo as its icon.** `ios.icon` pointed at `./assets/expo.icon` — a default icon bundle containing literally `expo-symbol.svg` on Expo's blue automatic gradient. The root `icon.png` was the same placeholder chevron. So the home-screen icon was never AnakMobil's. | No AI generation: the real mark was **squared off from the brand's own `favicon-dark.png`** rather than redrawn, so it is pixel-for-pixel theirs. That asset floats a rounded tile on pure-black padding; the tile was cropped out, its vertical gradient sampled from a left-margin column and extended into the rounded corners, then resized to 1024x1024 with no alpha (iOS masks its own squircle — a pre-rounded source double-rounds). The Android adaptive foreground was extracted by luminance mask and inset to the 62% safe zone, with the gradient as the background and a white silhouette as the monochrome layer. `ios.icon` removed so it falls through to the root icon. | **Closed.** Needed `expo prebuild --clean` and a delete of `ios/AnakMobilDev/expo.icon` — `expo run:ios` alone regenerates neither. Verified on the simulator home screen. |
| pre-exec | `correctness` (product) | AM-51 AC1 | "diantar ke beranda dengan **kendaraan aktif yang terakhir saya pilih**" cannot be satisfied by this plan and is not merely unbuilt: Plan A's `signIn` calls `clearActiveVehicle()` **by design**, so the device-global active vehicle is deliberately dropped on every sign-in — otherwise the next account opens on a car it does not own. Restoring the last vehicle must therefore be **per-account**, and that belongs to Plan C's shell. | No code change here. Report the split honestly on AM-51 at the end rather than letting the AC read as delivered. | Open — to be reported on the ticket. |

**Findings known before execution, to be entered by the task that touches them:**

- Task 1 — `hygiene` — the design system's `AmTextField` shipped with no
  `autoCapitalize`/`autoCorrect`/`autoComplete`/`textContentType`/`maxLength`, which makes every
  email and username field on iOS capitalise and autocorrect its first word. Five props added.
- Task 3 — `hygiene` — the design system has no checkbox primitive and React Native has no checkbox
  element. A local `ConsentCheckbox` was built from theme tokens; promote to `AmCheckbox` on a
  second consumer, not before.
- Task 4 — `correctness` (product) — AM-50 AC3 cannot be fully closed while AM-54 does not exist.
  The "masuk" half ships; the "pulihkan kata sandi" half is one sentence of honest copy. **Report on
  the AM-50 ticket.**

---

## Open questions the spec does not settle

Neither blocks execution.

1. **What the register endpoint returns for a taken username.** The spec says the handler "reports
   the field that actually collided" without naming a status code. Plan B's `registerConflictOf`
   handles both readings, and Task 1 Step 1 pins whichever one Plan A shipped. If Plan A chose
   something a third way, that function is the only thing that changes.
2. **Where sign-out lives before Plan C.** AM-51 AC4 says "dari setelan", and the settings screen is
   Plan C's. This plan mounts the component temporarily on the post-auth placeholder so the AC is
   verifiable, and marks the mount `TEMPORARY`. If the owner would rather AM-51 stay open until Plan
   C mounts it properly, that is a one-line change to this task and a note on the ticket.
