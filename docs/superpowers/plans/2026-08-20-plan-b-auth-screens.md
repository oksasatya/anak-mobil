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
- **Nothing new is installed.** `bun.lock` must be unchanged at the end of this plan.
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
8. apps/mobile has NO test runner and this work does not add one. tsconfig is
   strict; `@typescript-eslint/no-explicit-any: error`.
9. The AM-15 design system is complete and MUST be used — see
   apps/mobile/src/app/catalog.tsx for a worked example of every primitive.
10. Never set allowFontScaling={false}. Large system text must reflow.
11. Root .env belongs to the BACKEND; apps/mobile reads only EXPO_PUBLIC_*.
12. CI workflows are path-filtered per app.
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
import { signIn } from "@/shared/session";

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

**TDD: no** — no runner in `apps/mobile` (ENV 8), and the deliverable is a screen. Verified by
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

**TDD: no** — no runner (ENV 8). The pure part (`strengthOf`, the availability state machine) is
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

**TDD: no** — no runner (ENV 8); this is navigation and copy, verified by running the flow.

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
- Create: `apps/mobile/src/app/(auth)/index.tsx`

**Interfaces:**
- Consumes: `AmButton`, `useTheme`, `Link`/`router` from `expo-router`, the routes `/login` and
  `/register` (Tasks 2 and 3 — this is why it is serialised behind them: typed routes will not
  compile a link to a route file that does not exist).
- Produces: the route `/` within the `(auth)` group. Nothing consumes it.

**TDD: no** — layout and copy, no logic. Verified by opening it.

**Acceptance criteria:**
- Two actions: "Masuk" and "Daftar", both ≥44pt, both reaching a real screen.
- A value proposition of at most three lines that claims nothing the product does not do — no
  community counts, no "ribuan pengguna", no testimonial. The platform launches empty and says so.
- No guest-preview link (decided above — there is no guest surface to preview).
- Renders correctly on the smallest supported phone without scrolling to reach either button.

- [ ] **Step 1: Write `app/(auth)/index.tsx`**

```tsx
import { router } from "expo-router";
import { Text, View } from "react-native";
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
          Garasi digital buat mobil kamu. Catat servis dan modifikasi, cari tahu masalah yang umum
          terjadi, dan tanya soal mobilmu ke sesama pemilik.
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
- Modify (or create if Plan A did not): `apps/mobile/src/app/(app)/index.tsx` — a temporary mount so
  the flow is exercisable before Plan C exists.

**Interfaces:**
- Consumes: `signOut` from Plan A's session module · `AmButton`, `AmBottomSheet`, `useTheme`
- Produces: `function SignOutConfirm(props: SignOutConfirmProps)` with
  `interface SignOutConfirmProps { readonly label?: string }` — Plan C mounts this in the profile
  tab's settings.

**TDD: no** — no runner (ENV 8); the behaviour under test is a native sheet and a Plan A transaction.

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
import { signOut } from "@/shared/session";
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
    if (busy) return;
    setBusy(true);
    void signOut().finally(() => {
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
- **No new npm/bun dependency.** `bun.lock` unchanged.
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

- [ ] Task 1 — Auth request layer, schemas, and the AmTextField input props
- [ ] Task 2 — Login screen (uniform error, rate-limit countdown)
- [ ] Task 3 — Register screen (live validation, debounced availability, consent gate)
- [ ] Task 4 — The already-registered path
- [ ] Task 5 — Welcome screen
- [ ] Task 6 — Sign-out trigger

---

## Review findings ledger

*Empty at plan time. Severity vocabulary: `structural` (raise and fix now — a column, a constraint,
or a public contract) · `correctness` · `test-integrity` · `hygiene`.*

| Task | Severity | File:line | Failure scenario | Smallest fix | Closed how |
|---|---|---|---|---|---|

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
