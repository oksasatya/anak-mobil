# Plan D — Onboarding, vehicle wizard, and the aha screen

> **For agentic workers:** REQUIRED SUB-SKILL: use `executing-plans-hybrid` to
> implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for
> tracking.

**Goal:** A person who has just registered is carried through a short profile
step into a six-step catalog wizard, saves their first car, and lands on an
aha screen that is honest about having no community data yet.

**Architecture:** Three screens in Plan A's `(onboarding)` route group, backed
by two feature folders — `features/vehicle/` for the catalog reads and the
vehicle write, `features/onboarding/` for the persisted draft and the
seen-aha record. The wizard is **one route with six steps held in a persisted
store**, not six routes: that is what makes "resume at step four with every
choice intact" a single render instead of a reconstructed navigation stack.

**Tech Stack:** React Native 0.86 · Expo SDK 57 · expo-router 57 (typed
routes) · TanStack Query · zustand + MMKV · the AM-15 design system
(`apps/mobile/src/theme`, `apps/mobile/src/components`).

**Spec:** `docs/superpowers/specs/2026-08-20-am-17-auth-session-onboarding-design.md`

**Closes:** [AM-55](https://oksasatyaa.atlassian.net/browse/AM-55) ·
[AM-113](https://oksasatyaa.atlassian.net/browse/AM-113) ·
[AM-56](https://oksasatyaa.atlassian.net/browse/AM-56)

**Depends on:** Plan A (session, API client, route group and gate, TanStack
Query, zustand, MMKV), Plan B (auth screens), Plan C (app shell tabs). This is
the last of the four and assumes all three have landed.

---

## Global Constraints

Every task's requirements implicitly include this section.

- **Install nothing.** Every dependency this plan needs is already in
  `apps/mobile/package.json` or arrives with Plan A. A task that believes it
  needs a new package has found a design error — stop and report it.
- **No new hex, font size, spacing, or radius literal.** Everything through
  `useTheme()`. A viewport fraction (`height * 0.5`) is none of those four and
  is permitted; say so in the diff so a reviewer does not flag it.
- **Product strings are Bahasa Indonesia.** Identifiers, comments, this plan,
  and commit messages are English.
- **≥44pt touch targets without caller padding.** The primitives enforce this
  themselves (`theme.touchTargetMin`); a caller never adds padding to reach it.
- **Pickers go through `AmBottomSheet`, never a native picker or dialog.**
  `AmSelect` is the existing wrapper and this plan reuses it.
- **Never `allowFontScaling={false}`.**
- **Nothing is seeded with fake data.** No invented counts on the aha screen,
  no sample vehicles, no stock car photo. A vehicle without a photo gets a
  neutral placeholder (`docs/design.md` §48).
- **No skip button in the first-car wizard.** The variant step and the photo
  step are individually skippable because AM-113 AC3 and its technical note
  say so; the *wizard* is not.
- The AM-15 design system is committed (`docs/design.md`, `packages/tokens`,
  the `Am*` primitives). **The font/palette gate does not apply** — there is
  nothing to choose. `impeccable` refines within the existing system.

---

## Findings that shape this plan

Each of these was read out of the code, not inferred from the tickets. They
change what a ticket can honestly claim, so they are recorded here rather than
left for an implementer to trip over.

### 1. There is no upload endpoint anywhere, so neither photo ships

The full route table (`apps/api/crates/runtime/src/adapter/http/mod.rs:37-90`)
has no multipart route, no presign route, and no media route. `users` has no
photo column — its migration says so outright:

```
-- display name, photo, preferences — belongs to the story that renders one.
apps/api/crates/runtime/migrations/20260815164713_users.up.sql:4
```

`VehicleRequest` (`adapter/http/vehicles.rs:126-150`) has no photo field
either. The only photo storage in the schema is `build_photos`, and *its*
migration defers the mechanism explicitly ("presigned URLs, size limits, EXIF
stripping"). `expo-image-picker` is not installed and this plan installs
nothing.

**Therefore:**

- **AM-55 AC1's optional profile photo does not ship.** The step collects the
  display name only. `AmAvatar` renders initials from that name, which is the
  design system's existing answer to "no photo" and costs no new code.
  **AC1 is partially met** and the plan says so rather than implying otherwise.
- **AM-113's photo step ships in skip-only mode.** The step exists, appears in
  the progress indicator, and is passable — which is what AC1's six steps and
  AC2's back-navigation need. AM-113's own technical note authorises this:
  *"Foto boleh dilewati pada langkah terakhir; kendaraan tanpa foto memakai
  placeholder netral."* The step renders that placeholder and says plainly
  that uploading is not available yet. **AC1 is met**, with upload deferred.

A real vehicle silhouette asset is an owner-supplied file, never a generated
one. Until one exists the placeholder is a themed box with a caption — which
satisfies §48's "neutral silhouette / simplified vehicle placeholder" and
categorically avoids §48's prohibition on a stock car implying the wrong model.

### 2. The API supplies the year range; the client derives the list

`GenerationResponse` (`adapter/http/catalog.rs:41-51`) carries `year_start:
i16`, `year_end: Option<i16>`, and a prebuilt display string `years`
("2015–2021", or "2021–" for a generation still in production).

So AM-113 AC2's *"memilih generasi membuat pilihan tahun terbatas pada rentang
generasi itu"* is satisfied by deriving `year_start ..= (year_end ?? current
year)` on the client. **No API change is needed and none should be made.** The
open-ended case is the one to get right: `year_end: null` means "still being
built", and the list must end at the current year rather than at nothing.

### 3. Skipping the variant forces the client to send `described_as`

`VehicleRequest::check()` (`adapter/http/vehicles.rs:172-180`) refuses a car
that has neither a `variant_id` nor a non-empty `described_as`:

```rust
if self.variant_id.is_none() && self.described_as.as_deref().unwrap_or("").trim().is_empty()
```

AM-113 AC3 makes the variant skippable. A skipped variant therefore **must**
be accompanied by a composed description or the save is a 422. This plan
always sends `described_as`, composed as `"{brand} {model} {year}"` — the
exact shape the handler's own test fixture uses
(`described_as: Some("Toyota Avanza 2019")`, `vehicles.rs:380`). The
generation and variant names are deliberately not folded in: the seed's
generation names already repeat the model ("Avanza Gen 3"), so
`"Toyota Avanza Avanza Gen 3"` is what the obvious version produces.

### 4. `AmSelect` cannot show a catalog-length list, and the fix is four lines

`AmSelect` (`components/input/AmSelect.tsx:79-118`) maps every option into a
plain `View` inside `AmBottomSheet`. The sheet has no height cap and no
scroll, so a list longer than the screen renders off the top of it. The seed
carries roughly forty brands; an open generation produces up to about forty
years.

The lazy correct fix is to **make `AmSelect` scroll**, not to write a second
picker. `AmSelect<T extends string>` already accepts UUID strings as values,
and the one thing the wizard wants that it lacks — a subtitle line — is
covered by composing it into the label with a middle dot
(`"Avanza Gen 3 · 2021–"`, `"1.5 G · 2NR-VE"`). Every future caller gets the
scroll for free, and the reviewer reads a four-line diff instead of a
ninety-line new file.

The scroll must come from `react-native-gesture-handler`'s `ScrollView`, not
React Native's: the sheet wraps its content in a `GestureDetector` running a
`Gesture.Pan()`, and RNGH's scrollable is the one that composes with a parent
pan instead of fighting it. See the fallback in the environment card.

### 5. Saving a vehicle without invalidating `me` puts the gate in a loop

The onboarding gate routes on `Me.hasVehicles` (spec, §Bootstrap). After
`POST /vehicles` succeeds, that cached value is still `false`. Navigating
before the `me` query is invalidated **and awaited** sends the person to a
route the gate immediately bounces back into the wizard.

Just as important, the branch in AM-113 AC5 — aha screen for the first car,
elsewhere otherwise — must read `hasVehicles` **before** the invalidation
flips it. Capture it first, then invalidate.

### 6. Two destinations named by the tickets do not exist yet

- AM-113 AC5 sends a non-first car to *"halaman kendaraannya"*. That is
  **AM-116**, which has no screen. Honest substitute: the **Garage tab** with
  the new car active and a success toast.
- AM-113 AC3 says the engine and transmission *"bisa saya isi belakangan dari
  halaman kendaraan"* — the same absent screen. The server already supports it
  (`PUT /vehicles/{id}`), so **AC3's first half is met and its second half is
  AM-116's to deliver.** Nothing in this plan should build an edit screen to
  cover the gap.
- AM-56 AC2 offers *"mencatat modifikasi pertama"* as an example
  (`misalnya`) of a concrete action. The modification form is E2/E3 and has no
  mobile screen after Plan D. The word is illustrative, not binding, so AC2
  ships with a concrete action that **works** — "Lihat garasi saya" — and the
  illustrative one is recorded as deferred.

### 7. AM-56 AC3 is fully deliverable, because the ticket scopes out the menu

AC3 wants an invitation to ask AnakMobil AI. Its own out-of-scope line reads
*"Menu AI itu sendiri, yang dikerjakan di epic E8"*, and there is no AI route
on the server. So the invitation is copy and an affordance, not a link into a
screen that does not exist — and that is the ticket's own reading, not a
shortcut. **AC3 is met.**

### 8. `docs/design.md` §54 lists a five-step flow; the ticket lists six

§54's recommended flow is Brand → Model → Generation → Year → Photo. AM-113
AC1 inserts Variant before Photo. **The ticket wins.** Recorded so a reader
comparing the two does not file it as a defect.

---

## What each ticket honestly closes

| Criterion | Verdict | Where |
|---|---|---|
| AM-55 AC1 — display name + optional photo | **Partial** — name ships, photo deferred (finding 1) | Task 4 |
| AM-55 AC2 — first car cannot be skipped | Met — enforced by Plan A's gate; this plan adds no second guard | Task 5 |
| AM-55 AC3 — progress survives closing the app | Met | Tasks 2, 4, 5 |
| AM-55 AC4 — finishing lands on the aha screen | Met | Tasks 5, 6 |
| AM-113 AC1 — six steps, free back-navigation, visible progress | Met — photo step is skip-only (finding 1) | Tasks 3, 5 |
| AM-113 AC2 — each step narrows the next; year bounded by generation | Met (finding 2) | Tasks 1, 5 |
| AM-113 AC3 — engine/transmission skippable | **Partial** — skippable ✓; fill-in-later is AM-116 (finding 6) | Task 5 |
| AM-113 AC4 — draft survives a force-close | Met | Tasks 2, 5 |
| AM-113 AC5 — saved vehicle becomes active | Met; non-first destination substituted (finding 6) | Task 5 |
| AM-56 AC1 — community counts | **Deferred, unmet.** Nothing computes them and the project forbids invented numbers. Seam left per Task 6. | — |
| AM-56 AC2 — "be the first" plus one concrete action | Met; the illustrative action substituted (finding 6) | Task 6 |
| AM-56 AC3 — AI invitation always present | Met (finding 7) | Task 6 |
| AM-56 AC4 — exits to home, never shown again for that vehicle | Met | Tasks 2, 6 |

**AM-56 closes with AC1 explicitly unmet.** That is the spec's decision
(§"The aha screen renders in AC2 mode only"), not a discovery made here.

---

## File structure

```
apps/mobile/src/features/vehicle/
  types.ts                    wire shapes, mirroring the Rust response structs
  catalog.ts                  the four catalog queries + yearOptions()
  createVehicle.ts            POST /vehicles + describedAsFrom()
  VehiclePhotoPlaceholder.tsx §48's neutral placeholder

apps/mobile/src/features/onboarding/
  draft.ts                    the persisted wizard draft + its cascade rules
  ahaSeen.ts                  which vehicles have already seen the aha screen
  WizardProgress.tsx          AC1's visible progress

apps/mobile/src/app/(onboarding)/
  profile.tsx                 AM-55 AC1
  vehicle.tsx                 the six-step wizard
  aha.tsx                     AM-56 in AC2 mode

apps/mobile/src/components/input/AmSelect.tsx   MODIFIED — the scroll fix
apps/mobile/src/shared/session/signOut.ts       MODIFIED — clear both stores
```

`app/(onboarding)/_layout.tsx` and the gate belong to **Plan A**. If Plan A
shipped placeholder screens at these three paths, this plan replaces them; if
it shipped none, these are new files. Read before writing.

---

## ENVIRONMENT — paste verbatim into every task brief

```
1. Every make target runs from the REPOSITORY ROOT.
2. Mobile gate: `make mb-check` (fmt-check -> expo typed routes -> tsc --noEmit
   -> expo lint). Whole repo: `make check`. Catalog data: `make db-seed`.
3. Bun, never npm. `bun add --filter` DOES NOT EXIST — use
   `bun add --cwd apps/mobile <pkg>` or `bun x expo install <pkg>`.
   `bun install --frozen-lockfile` must stay EXIT=0 with bun.lock unchanged.
4. Prettier runs from the ROOT only (`bun run format`). Markdown excluded.
5. ** expo-router SDK 56+ VENDORS ITS OWN NAVIGATION. ** Do NOT install
   @react-navigation/native. ThemeProvider/DarkTheme/DefaultTheme come FROM
   "expo-router".
6. _layout.tsx already overrides the navigation container background to
   transparent so AmGround shows through. Do not undo it.
7. NEVER put a changing `key` on a View wrapping {children} at the app root —
   it unmounts the subtree and discards state, which would destroy a wizard
   draft held in component state.
8. apps/mobile has NO test runner and this work does not add one. tsconfig
   strict; `@typescript-eslint/no-explicit-any: error`.
9. The AM-15 design system is complete and MUST be used — apps/mobile/src/app/
   catalog.tsx is the worked example.
10. Never set allowFontScaling={false}.
11. Root .env belongs to the BACKEND; apps/mobile reads only EXPO_PUBLIC_*.
12. CI workflows are path-filtered per app.
```

**Plan-D additions to the environment card — paste these too:**

```
13. THE CATALOG RESPONSES ARE snake_case ON THE WIRE. GenerationResponse has
    year_start / year_end / years; VariantResponse has engine_code. The TS
    types mirror the wire field-for-field ON PURPOSE so drift is visible
    against apps/api/crates/runtime/src/adapter/http/catalog.rs. Do NOT
    "fix" them to camelCase — Me is camelCase because Plan A maps it; the
    catalog types are not mapped because nothing would be gained by a second
    naming to keep in sync.
14. AmBottomSheet runs a Gesture.Pan() over its whole body. A scrollable child
    must be ScrollView/FlatList from "react-native-gesture-handler", NOT from
    "react-native". If drag-to-dismiss still fights the scroll on a device,
    the fallback is ONE line on the sheet's pan —
    `.activeOffsetY([-12, 12])` in components/display/AmBottomSheet.tsx — and
    that change requires re-verifying the existing Transmisi select on
    apps/mobile/src/app/catalog.tsx before it is accepted.
15. Route groups are omitted from expo-router URLs, and typed routes generate
    the accepted literals into .expo/types. Write "/(onboarding)/aha" first;
    if tsc rejects it, use "/aha". Run `make mb-check` to regenerate the
    types — do not hand-edit them.
16. PLAN A OWNS THESE AND THEY MUST BE READ, NEVER ASSUMED: the TanStack Query
    key for GET /me (written below as ["me"]) and for GET /vehicles (["vehicles"]);
    whether apiRequest<T> returns the envelope or the unwrapped `data` (this
    plan assumes UNWRAPPED); whether an MMKV StateStorage adapter for zustand
    already exists in shared/api/queryClient.ts (reuse it if so); and the
    display-name validation rule the server applies to PATCH /me. Open
    apps/mobile/src/shared/ before writing a line that depends on any of them,
    and correct this plan file in place if it is wrong.
17. To see an EMPTY catalog step, run `make db-drop` (migrates, does not seed).
    To see a populated one, follow it with `make db-seed`. An empty step is a
    real state, not an error.
```

---

## Quality gate — paste verbatim into every task brief

This repository runs **no Sonar**. This block is the gate in its place.

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

## Tidak boleh ada

Carried from the spec's own block, narrowed to what this plan could violate.

- No invented counts on the aha screen, no seeded vehicles, no placeholder
  community numbers, no zeros rendered as if they were data (AM-56 AC2 says
  *"angka nol tidak ditampilkan"*).
- No stock car photo standing in for a vehicle photo (`docs/design.md` §48).
- **No skip button in the first-car wizard.** The variant step and the photo
  step are individually skippable; the wizard is not.
- No second, weaker onboarding guard inside a component. Plan A's route gate
  is the only enforcement of AM-55 AC2.
- No STNK scanning, no import from another source, no "model not in catalog"
  escape hatch — that last one is **AM-114**, and `POST /catalog/suggestions`
  must not be called from anywhere in this plan.
- No plate, VIN, purchase price, or purchase date collected during onboarding.
  The wizard never sends a `private` block.
- No `cost_visibility` sent from the wizard. Omitting it makes the server
  default to `private`; sending it is how a default silently widens later.
- No new dependency, and no new test runner.
- No vehicle-edit screen built to cover AM-116's absence.
- No modification of `AmBottomSheet` unless the fallback in environment note
  14 is genuinely needed, and never without re-verifying `catalog.tsx`.

---

## Task 1: The vehicle API layer — catalog reads, the year derivation, the create

**Files:**
- Create: `apps/mobile/src/features/vehicle/types.ts`
- Create: `apps/mobile/src/features/vehicle/catalog.ts`
- Create: `apps/mobile/src/features/vehicle/createVehicle.ts`

**Interfaces:**
- Consumes: `apiRequest<T>`, `ApiError` from Plan A's `@/shared/api`.
- Produces:
  ```ts
  export interface CatalogEntry { readonly id: string; readonly name: string }
  export interface Generation extends CatalogEntry {
    readonly year_start: number;
    readonly year_end: number | null;
    readonly years: string;
  }
  export interface Variant extends CatalogEntry { readonly engine_code: string | null }

  export function useBrands(): UseQueryResult<CatalogEntry[], ApiError>;
  export function useModels(brandId: string | null): UseQueryResult<CatalogEntry[], ApiError>;
  export function useGenerations(modelId: string | null): UseQueryResult<Generation[], ApiError>;
  export function useVariants(generationId: string | null): UseQueryResult<Variant[], ApiError>;

  export function toOptions(entries: readonly CatalogEntry[]): AmSelectOption<string>[];
  export function generationOptions(list: readonly Generation[]): AmSelectOption<string>[];
  export function variantOptions(list: readonly Variant[]): AmSelectOption<string>[];
  export function yearOptions(g: Generation, currentYear?: number): AmSelectOption<string>[];

  export function describedAsFrom(brand: string, model: string, year: number | null): string;
  export interface CreateVehicleInput {
    readonly variantId: string | null;
    readonly describedAs: string;
    readonly year: number | null;
  }
  export function useCreateVehicle(): UseMutationResult<{ id: string }, ApiError, CreateVehicleInput>;
  ```

**TDD: no** — `apps/mobile` has no test runner and this work does not add one
(environment note 8, and the spec's own Testing section). This is *impossible*
here, not inadvisable: `yearOptions` and `describedAsFrom` are pure functions
with boundary cases and would be the first things a runner covered. They are
written as free functions taking their inputs explicitly — including
`currentYear`, mirroring the backend's own discipline of passing `today` into
`derive_reminders` rather than reading the clock — so that a runner can cover
them the day one arrives. Verified meanwhile by Task 5 on a simulator.

**Big O:** `yearOptions` is O(k) in the generation's span, k ≤ ~40. The option
mappers are O(n) over one catalog level, n ≤ ~40. Nothing here loops over a
query, and no step fetches inside a loop.

**Acceptance criteria:** AM-113 AC2 (the queries' `enabled` chain *is* the
narrowing; `yearOptions` *is* the year bound).

- [ ] **Step 1: Write the wire types**

Create `apps/mobile/src/features/vehicle/types.ts`:

```ts
/**
 * These mirror the Rust response structs field-for-field, snake_case
 * included, so drift against the server is visible rather than absorbed by a
 * mapping layer. The source of truth is
 * apps/api/crates/runtime/src/adapter/http/catalog.rs.
 *
 * Only the fields this app actually reads are declared. A variant carries
 * pcd, torque, and bolt circle on the wire; the wizard shows none of them,
 * and typing a field nobody reads is how a type starts lying about what the
 * code depends on.
 */
export interface CatalogEntry {
  readonly id: string;
  readonly name: string;
}

export interface Generation extends CatalogEntry {
  readonly year_start: number;
  /** Null means the generation is still in production. */
  readonly year_end: number | null;
  /** Prebuilt by the server: "2015–2021", or "2021–" while in production. */
  readonly years: string;
}

export interface Variant extends CatalogEntry {
  readonly engine_code: string | null;
}
```

- [ ] **Step 2: Write the catalog queries and the option mappers**

Create `apps/mobile/src/features/vehicle/catalog.ts`:

```ts
import type { AmSelectOption } from "@/components/input";
import { apiRequest } from "@/shared/api";
import { useQuery } from "@tanstack/react-query";

import type { CatalogEntry, Generation, Variant } from "./types";

/**
 * Each level is enabled only once its parent has been chosen. That `enabled`
 * chain IS AM-113 AC2's narrowing — there is no client-side filtering to get
 * wrong, because the server only ever returns the children of the parent in
 * the path.
 */
export function useBrands() {
  return useQuery({
    queryKey: ["catalog", "brands"],
    queryFn: () => apiRequest<CatalogEntry[]>("/catalog/brands"),
  });
}

export function useModels(brandId: string | null) {
  return useQuery({
    queryKey: ["catalog", "models", brandId],
    queryFn: () => apiRequest<CatalogEntry[]>(`/catalog/brands/${brandId}/models`),
    enabled: brandId !== null,
  });
}

export function useGenerations(modelId: string | null) {
  return useQuery({
    queryKey: ["catalog", "generations", modelId],
    queryFn: () => apiRequest<Generation[]>(`/catalog/models/${modelId}/generations`),
    enabled: modelId !== null,
  });
}

export function useVariants(generationId: string | null) {
  return useQuery({
    queryKey: ["catalog", "variants", generationId],
    queryFn: () => apiRequest<Variant[]>(`/catalog/generations/${generationId}/variants`),
    enabled: generationId !== null,
  });
}

export function toOptions(entries: readonly CatalogEntry[]): AmSelectOption<string>[] {
  return entries.map((entry) => ({ value: entry.id, label: entry.name }));
}

/**
 * The years string goes into the label rather than a subtitle line: it is how
 * a person recognises their generation, and AmSelect has one line per option.
 */
export function generationOptions(list: readonly Generation[]): AmSelectOption<string>[] {
  return list.map((g) => ({ value: g.id, label: `${g.name} · ${g.years}` }));
}

export function variantOptions(list: readonly Variant[]): AmSelectOption<string>[] {
  return list.map((v) => ({
    value: v.id,
    label: v.engine_code ? `${v.name} · ${v.engine_code}` : v.name,
  }));
}

/**
 * AM-113 AC2: the year list is the generation's own range, newest first.
 *
 * `year_end: null` means still in production, so the range ends at the
 * current year. `currentYear` is a parameter rather than a call to the clock
 * for the same reason `derive_reminders` takes `today` — it is what makes the
 * function checkable without waiting a year.
 *
 * The clamp matters: a generation whose start is in the future would
 * otherwise produce an empty list, and an empty year step has no honest
 * empty state because a generation always has at least its opening year.
 */
export function yearOptions(
  g: Generation,
  currentYear: number = new Date().getFullYear(),
): AmSelectOption<string>[] {
  const last = Math.max(g.year_start, g.year_end ?? currentYear);
  const years: AmSelectOption<string>[] = [];
  for (let year = last; year >= g.year_start; year -= 1) {
    years.push({ value: String(year), label: String(year) });
  }
  return years;
}
```

- [ ] **Step 3: Write the create mutation**

Create `apps/mobile/src/features/vehicle/createVehicle.ts`:

```ts
import { apiRequest, type ApiError } from "@/shared/api";
import { useMutation } from "@tanstack/react-query";

export interface CreateVehicleInput {
  readonly variantId: string | null;
  readonly describedAs: string;
  readonly year: number | null;
}

/**
 * `POST /vehicles` refuses a car with neither a variant_id nor a non-empty
 * described_as (adapter/http/vehicles.rs:172), so a skipped variant must
 * arrive with a description or the save is a 422.
 *
 * Brand and model only. The seed's generation names already repeat the model
 * ("Avanza Gen 3"), so folding them in produces "Toyota Avanza Avanza Gen 3".
 * The shape below is the one the handler's own fixture uses.
 */
export function describedAsFrom(brand: string, model: string, year: number | null): string {
  return [brand, model, year === null ? null : String(year)]
    .filter((part): part is string => part !== null && part.trim() !== "")
    .join(" ");
}

/**
 * cost_visibility is deliberately absent: the server defaults it to private
 * (`default_cost_visibility`), and an absent field can never widen who sees
 * what a car cost. `private` is absent too — onboarding collects no plate,
 * VIN, or price.
 */
export function useCreateVehicle() {
  return useMutation<{ id: string }, ApiError, CreateVehicleInput>({
    mutationFn: (input) =>
      apiRequest<{ id: string }>("/vehicles", {
        method: "POST",
        body: {
          variant_id: input.variantId,
          described_as: input.describedAs,
          year: input.year,
        },
      }),
  });
}
```

- [ ] **Step 4: Confirm the assumptions this file makes about Plan A**

Open `apps/mobile/src/shared/api/` and check three things. Correct the code
and **this plan file** if any is wrong:

1. `apiRequest<T>` returns the unwrapped `data`, not the `{meta, data, error}`
   envelope. If it returns the envelope, every `apiRequest<X[]>` above becomes
   `apiRequest<{ data: X[] }>(...)` and reads `.data`.
2. `apiRequest`'s `init.body` is serialised for you (the frozen signature takes
   `body?: unknown`, so it should be). If it expects a pre-stringified body,
   wrap it.
3. `ApiError` is exported from `@/shared/api`. If it lives at
   `@/shared/api/errors`, fix the import.

- [ ] **Step 5: Verify**

```bash
bun run format
make mb-check
```

Expected: `EXIT=0` and `mobile gate green`. Nothing renders yet; this task's
deliverable is a compiling API layer that Task 5 consumes.

- [ ] **Step 6: Report**

Report the three answers from Step 4 verbatim to the controller — the query
shapes in Tasks 5 and 6 depend on them.

---

## Task 2: The persisted onboarding draft and the seen-aha record

**Files:**
- Create: `apps/mobile/src/features/onboarding/draft.ts`
- Create: `apps/mobile/src/features/onboarding/ahaSeen.ts`
- Modify: `apps/mobile/src/shared/session/signOut.ts` (add two `clear()` calls)

**Interfaces:**
- Consumes: zustand and its MMKV storage adapter — **reuse Plan A's if one
  exists** (environment note 16); Plan A's `signOut` transaction.
- Produces:
  ```ts
  export const WIZARD_STEPS: readonly ["brand","model","generation","year","variant","photo"];
  export type WizardStep = (typeof WIZARD_STEPS)[number];
  export interface Choice { readonly id: string; readonly name: string }
  export interface GenerationChoice extends Choice {
    readonly yearStart: number; readonly yearEnd: number | null; readonly years: string;
  }
  export interface DraftState {
    readonly version: number;
    readonly userId: string | null;
    readonly displayName: string;
    readonly step: WizardStep;
    readonly brand: Choice | null;
    readonly model: Choice | null;
    readonly generation: GenerationChoice | null;
    readonly year: number | null;
    readonly variant: Choice | null;
    readonly variantSkipped: boolean;
    setDisplayName(name: string): void;
    setBrand(choice: Choice): void;
    setModel(choice: Choice): void;
    setGeneration(choice: GenerationChoice): void;
    setYear(year: number): void;
    setVariant(choice: Choice): void;
    skipVariant(): void;
    goTo(step: WizardStep): void;
    adoptUser(userId: string): void;
    clear(): void;
  }
  export const useDraft: UseBoundStore<StoreApi<DraftState>>;
  export function canAdvance(state: DraftState, step: WizardStep): boolean;

  export interface AhaSeenState {
    readonly seen: readonly string[];
    markSeen(vehicleId: string): void;
    clear(): void;
  }
  export const useAhaSeen: UseBoundStore<StoreApi<AhaSeenState>>;
  ```

**TDD: no** — same reason as Task 1, and it is worth being precise about
which half of that is environmental. The cascade in `setBrand`/`setModel`/
`setGeneration` is the single strongest test candidate in this plan: it has a
real invariant ("changing a parent clears its children") and a real trap
("re-selecting the same parent must clear nothing"). With no runner and none
added, it is verified by the explicit simulator recipe in Task 5's Step 8. If
the controller decides one pure-logic check is worth the exception, this file
is where it goes — but adding a test that `make mb-check` does not execute is
worse than none, because it rots unnoticed, so adding it means adding it to
the gate, which is out of scope here.

**Big O:** `AhaSeenState.seen.includes()` is O(n) over a person's vehicles,
n in single digits. A `Set` would buy nothing and would not survive JSON
persistence without a custom serialiser.

**Acceptance criteria:** AM-55 AC3, AM-113 AC1 (nothing is lost moving
backwards), AM-113 AC4, AM-56 AC4 (the "never again" half).

- [ ] **Step 1: Read how Plan A persists state, before writing any of it**

```bash
cat apps/mobile/src/shared/api/queryClient.ts
cat apps/mobile/src/shared/session/store.ts
cat apps/mobile/src/shared/session/signOut.ts
```

If a zustand-compatible MMKV `StateStorage` is already exported, **import it**
rather than writing the adapter in Step 2. Two adapters over one MMKV instance
is a second source of truth about where state lives.

- [ ] **Step 2: Write the draft store**

Create `apps/mobile/src/features/onboarding/draft.ts`. If Step 1 found an
existing storage adapter, delete the local `storage` const and import theirs.

```ts
import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

export const WIZARD_STEPS = [
  "brand",
  "model",
  "generation",
  "year",
  "variant",
  "photo",
] as const;

export type WizardStep = (typeof WIZARD_STEPS)[number];

export interface Choice {
  readonly id: string;
  readonly name: string;
}

export interface GenerationChoice extends Choice {
  readonly yearStart: number;
  readonly yearEnd: number | null;
  readonly years: string;
}

/**
 * Bumped whenever the persisted shape changes. A draft written by an older
 * shape is discarded rather than migrated — a half-restored wizard is worse
 * than starting the six steps again, and a migration for a draft measured in
 * minutes of a person's time is not worth writing.
 */
export const DRAFT_VERSION = 1;

interface DraftData {
  version: number;
  /** Stamped on first write; a draft belonging to another account is discarded. */
  userId: string | null;
  displayName: string;
  step: WizardStep;
  brand: Choice | null;
  model: Choice | null;
  generation: GenerationChoice | null;
  year: number | null;
  variant: Choice | null;
  /** Distinct from `variant === null`: "not chosen yet" cannot advance, "skipped" can. */
  variantSkipped: boolean;
}

const EMPTY: DraftData = {
  version: DRAFT_VERSION,
  userId: null,
  displayName: "",
  step: "brand",
  brand: null,
  model: null,
  generation: null,
  year: null,
  variant: null,
  variantSkipped: false,
};

export interface DraftState extends DraftData {
  setDisplayName(name: string): void;
  setBrand(choice: Choice): void;
  setModel(choice: Choice): void;
  setGeneration(choice: GenerationChoice): void;
  setYear(year: number): void;
  setVariant(choice: Choice): void;
  skipVariant(): void;
  goTo(step: WizardStep): void;
  adoptUser(userId: string): void;
  clear(): void;
}

export const useDraft = create<DraftState>()(
  persist(
    (set, get) => ({
      ...EMPTY,

      setDisplayName: (displayName) => set({ displayName }),

      // The cascade, and the guard that makes AM-113 AC1 work.
      //
      // Choosing a different brand invalidates everything below it — a Civic
      // generation under Toyota is not a thing. But going BACK to the brand
      // step and re-confirming the SAME brand must clear nothing, or "kembali
      // ke langkah mana pun tanpa kehilangan isian" is false. Hence the
      // early return on an unchanged id, repeated at each level.
      setBrand: (brand) => {
        if (get().brand?.id === brand.id) return;
        set({
          brand,
          model: null,
          generation: null,
          year: null,
          variant: null,
          variantSkipped: false,
        });
      },

      setModel: (model) => {
        if (get().model?.id === model.id) return;
        set({ model, generation: null, year: null, variant: null, variantSkipped: false });
      },

      setGeneration: (generation) => {
        if (get().generation?.id === generation.id) return;
        // Year is cleared because the new generation's range may not contain
        // it. Variant is cleared because variants hang off the generation.
        set({ generation, year: null, variant: null, variantSkipped: false });
      },

      // Year does NOT cascade: /catalog/generations/{id}/variants is keyed by
      // the generation alone, so the variant list does not depend on the year.
      setYear: (year) => set({ year }),

      setVariant: (variant) => set({ variant, variantSkipped: false }),
      skipVariant: () => set({ variant: null, variantSkipped: true }),

      goTo: (step) => set({ step }),

      adoptUser: (userId) => {
        const current = get();
        if (current.userId !== null && current.userId !== userId) {
          set({ ...EMPTY, userId });
          return;
        }
        if (current.userId === null) set({ userId });
      },

      clear: () => set({ ...EMPTY }),
    }),
    {
      name: "anakmobil.onboarding.draft",
      storage: createJSONStorage(() => storage),
      version: DRAFT_VERSION,
      // A shape written by an older version is dropped, not migrated.
      migrate: () => ({ ...EMPTY }),
    },
  ),
);

/**
 * Whether the wizard may leave `step`.
 *
 * The photo step always may — AM-113's technical note authorises skipping it,
 * and there is no upload endpoint to make it anything else.
 */
export function canAdvance(state: DraftState, step: WizardStep): boolean {
  switch (step) {
    case "brand":
      return state.brand !== null;
    case "model":
      return state.model !== null;
    case "generation":
      return state.generation !== null;
    case "year":
      return state.year !== null;
    case "variant":
      return state.variant !== null || state.variantSkipped;
    case "photo":
      return true;
  }
}
```

The MMKV adapter, **only if Step 1 found none** — put it directly above
`useDraft` and export nothing:

```ts
import { MMKV } from "react-native-mmkv";
import type { StateStorage } from "zustand/middleware";

const mmkv = new MMKV({ id: "anakmobil.onboarding" });

const storage: StateStorage = {
  getItem: (name) => mmkv.getString(name) ?? null,
  setItem: (name, value) => mmkv.set(name, value),
  removeItem: (name) => mmkv.delete(name),
};
```

- [ ] **Step 3: Write the seen-aha store**

Create `apps/mobile/src/features/onboarding/ahaSeen.ts`:

```ts
import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

export interface AhaSeenState {
  readonly seen: readonly string[];
  markSeen(vehicleId: string): void;
  clear(): void;
}

/**
 * AM-56 AC4: "layar aha tidak muncul lagi untuk mobil yang sama".
 *
 * A list rather than a Set: a person has a handful of cars, `includes` over
 * single digits costs nothing, and a Set does not survive JSON persistence
 * without a serialiser nobody needs.
 */
export const useAhaSeen = create<AhaSeenState>()(
  persist(
    (set, get) => ({
      seen: [],
      markSeen: (vehicleId) => {
        if (get().seen.includes(vehicleId)) return;
        set({ seen: [...get().seen, vehicleId] });
      },
      clear: () => set({ seen: [] }),
    }),
    { name: "anakmobil.onboarding.ahaSeen", storage: createJSONStorage(() => storage) },
  ),
);
```

Import the same `storage` used by `draft.ts`; if it is local to that file,
export it from there rather than constructing a second MMKV instance.

- [ ] **Step 4: Clear both stores on sign-out**

Open `apps/mobile/src/shared/session/signOut.ts` and add the two `clear()`
calls to the "client stores are reset" phase of the epoch transaction, beside
whatever Plan A already resets there. Do not invent a new phase.

```ts
useDraft.getState().clear();
useAhaSeen.getState().clear();
```

This is hygiene, not the defence. The defence is `adoptUser`, which discards a
draft belonging to a different account on read — so a missed wipe cannot show
one person's half-finished car to the next.

- [ ] **Step 5: Verify**

```bash
bun run format
make mb-check
```

Expected: `EXIT=0`. Confirm in the diff that `signOut.ts` still increments the
epoch first and still performs exactly one redirect — this task adds two lines
to an existing transaction and must not reorder it.

---

## Task 3: Make `AmSelect` scroll, and add the neutral photo placeholder

**Invoke `frontend-design` first.** The font/palette gate does **not** apply:
this surface has a committed design system (`docs/design.md`,
`packages/tokens`, the AM-15 primitives), so `impeccable` refines within it
rather than opening a visual direction.

**Files:**
- Modify: `apps/mobile/src/components/input/AmSelect.tsx` (the options list only)
- Create: `apps/mobile/src/features/vehicle/VehiclePhotoPlaceholder.tsx`

**Interfaces:**
- Consumes: `AmBottomSheet`, `useTheme`.
- Produces: `AmSelect`'s public props are **unchanged** — this is an internal
  fix, and any change to `AmSelectProps` is a design error. Plus:
  ```ts
  export interface VehiclePhotoPlaceholderProps { readonly caption?: string }
  export function VehiclePhotoPlaceholder(props: VehiclePhotoPlaceholderProps): JSX.Element;
  ```

**TDD: no** — visual and layout work with no branching logic worth a contract.
Verified by opening it (§16's test-after side): the existing Transmisi select
on `catalog.tsx` must look and behave exactly as it does today, and a
forty-option list must scroll inside the sheet.

**Acceptance criteria:** AM-113 AC1 (a step whose options do not fit is a
dead end), AM-113's technical note (neutral placeholder), `docs/design.md`
§48.

- [ ] **Step 1: Reproduce the overflow before fixing it**

Run the app and open the component catalog:

```bash
make db-up && make db-seed
# in another shell
cd apps/mobile && bun x expo start --dev-client
```

Temporarily extend `TRANSMISSIONS` in `apps/mobile/src/app/catalog.tsx` to
forty entries, open the Transmisi select, and screenshot the sheet running off
the top of the screen. **Revert that edit before committing** — it is a
reproduction, not a change.

- [ ] **Step 2: Cap the height and scroll, using RNGH's scrollable**

In `apps/mobile/src/components/input/AmSelect.tsx`, add the imports:

```ts
import { useWindowDimensions } from "react-native";
import { ScrollView } from "react-native-gesture-handler";
```

Then wrap the existing options `View` — leaving the `View`, its
`accessibilityRole="radiogroup"`, and every `Pressable` inside it exactly as
they are:

```tsx
<AmBottomSheet visible={open} onClose={() => setOpen(false)} title={label}>
  {/*
    The sheet grows to its content and has no scroll of its own, so a
    catalog-length list renders off the top of the screen. Half the viewport
    is the cap; `useWindowDimensions` is a viewport fraction rather than a
    design value, so it is not a raw literal the theme should own.

    ScrollView comes from react-native-gesture-handler, not react-native:
    AmBottomSheet runs a Gesture.Pan() over its whole body, and RNGH's
    scrollable is the one that composes with a parent pan instead of losing
    to it.
  */}
  <ScrollView style={{ maxHeight: height * 0.5 }} showsVerticalScrollIndicator>
    <View accessibilityRole="radiogroup">
      {/* unchanged */}
    </View>
  </ScrollView>
</AmBottomSheet>
```

with `const { height } = useWindowDimensions();` beside the existing
`const theme = useTheme();`.

- [ ] **Step 3: Write the placeholder**

Create `apps/mobile/src/features/vehicle/VehiclePhotoPlaceholder.tsx`:

```tsx
import { StyleSheet, Text, View } from "react-native";

import { AmMaterial } from "@/components/material";
import { useTheme } from "@/theme";

export interface VehiclePhotoPlaceholderProps {
  readonly caption?: string;
}

/**
 * docs/design.md §48: a car with no photo gets a neutral placeholder, never a
 * stock car that would imply the wrong model — and AM-113's technical note
 * says the same thing in the ticket's own words.
 *
 * There is no silhouette asset in packages/assets and no icon library
 * installed, so the neutral form is a themed frame with a caption. That is
 * genuinely neutral, adds no dependency, and cannot mislead. A real
 * silhouette is an owner-supplied file; when one exists it drops in here and
 * nothing else changes.
 */
export function VehiclePhotoPlaceholder({
  caption = "Belum ada foto",
}: VehiclePhotoPlaceholderProps) {
  const theme = useTheme();
  return (
    <AmMaterial
      role="working"
      radius="lg"
      style={[
        styles.frame,
        { borderColor: theme.color.border, borderRadius: theme.radius.lg },
      ]}
    >
      <View style={{ gap: theme.space[2] }}>
        <Text style={[theme.type.label, styles.centered, { color: theme.color.textSecondary }]}>
          {caption}
        </Text>
      </View>
    </AmMaterial>
  );
}

const styles = StyleSheet.create({
  // 16:9, the shape a vehicle photo will occupy when one can be uploaded.
  frame: { aspectRatio: 16 / 9, alignItems: "center", justifyContent: "center", borderWidth: 1 },
  centered: { textAlign: "center" },
});
```

- [ ] **Step 4: Verify, including the primitive you just changed**

```bash
bun run format
make mb-check
```

Then on a simulator, **in both themes**:

- `catalog.tsx` → Transmisi select with its real four options: unchanged
  appearance, still opens, still dismisses by drag and by "Tutup".
- The same select temporarily extended to forty options: the sheet stays on
  screen, the list scrolls, and dragging the sheet header still dismisses it.
  If the drag and the scroll fight each other, apply the fallback in
  environment note 14 and re-verify both cases.

Screenshot both, both themes, and confirm the reproduction edit is reverted.

---

## Task 4: The profile step

**Invoke `frontend-design` first.** The font/palette gate does **not** apply —
committed design system; `impeccable` refines within it.

**Files:**
- Create (or replace Plan A's placeholder): `apps/mobile/src/app/(onboarding)/profile.tsx`

**Interfaces:**
- Consumes: `useSession`, `apiRequest`, `Me`, `ApiError` from Plan A;
  `useDraft`, `adoptUser`, `setDisplayName` from Task 2; `AmTextField`,
  `AmButton`, `AmAvatar`, `AmCard` from the design system.
- Produces: the route `/(onboarding)/profile`. Nothing imports from it.

**TDD: no** — a single-field form whose only rule is "not empty", verified by
running it. The authoritative validation is the server's 422, which is mapped
onto the field rather than duplicated.

**Acceptance criteria:**

- **AM-55 AC1 — partially met.** Display name is collected and the step leads
  straight into the wizard. The optional photo does not ship (finding 1); the
  screen states this in one line rather than showing a control that cannot
  work, and `AmAvatar` renders initials so the slot is not empty.
- **AM-55 AC3** — a name typed but not submitted survives a force-close,
  because it is written to the draft on every keystroke.

- [ ] **Step 1: Read the two things Plan A owns**

```bash
cat apps/mobile/src/app/\(onboarding\)/_layout.tsx
grep -rn "display_name\|displayName" apps/mobile/src/shared/
```

Confirm the query key for `me`, the exact `PATCH /me` call shape if Plan A
already wrote one, and the display-name rule the server enforces. Mirror that
rule; do not invent a different one.

- [ ] **Step 2: Write the screen**

Create `apps/mobile/src/app/(onboarding)/profile.tsx`:

```tsx
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useRouter } from "expo-router";
import { useEffect } from "react";
import { ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmAvatar, AmCard } from "@/components/display";
import { AmButton, AmTextField } from "@/components/input";
import { useDraft } from "@/features/onboarding/draft";
import { apiRequest, type ApiError } from "@/shared/api";
import { useSession, type Me } from "@/shared/session";
import { useTheme } from "@/theme";

const MIN_NAME = 2;
const MAX_NAME = 50;

export default function ProfileStep() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const router = useRouter();
  const queryClient = useQueryClient();
  const { user } = useSession();
  const draft = useDraft();

  // A draft belonging to another account is discarded here rather than shown.
  useEffect(() => {
    if (user) draft.adoptUser(user.id);
  }, [user, draft]);

  // Seed from the server once, so somebody returning after a partial
  // onboarding sees the name they already saved.
  useEffect(() => {
    if (draft.displayName === "" && user?.displayName) draft.setDisplayName(user.displayName);
  }, [user?.displayName, draft]);

  const name = draft.displayName.trim();
  const tooShort = name.length > 0 && name.length < MIN_NAME;
  const valid = name.length >= MIN_NAME && name.length <= MAX_NAME;

  const save = useMutation<Me, ApiError, string>({
    mutationFn: (displayName) =>
      apiRequest<Me>("/me", { method: "PATCH", body: { display_name: displayName } }),
    onSuccess: async (me) => {
      queryClient.setQueryData(["me"], me);
      router.replace("/(onboarding)/vehicle");
    },
  });

  return (
    <ScrollView
      contentContainerStyle={{
        padding: theme.pagePadding,
        paddingTop: insets.top + theme.space[8],
        paddingBottom: insets.bottom + theme.space[10],
        gap: theme.space[6],
      }}
      keyboardShouldPersistTaps="handled"
    >
      <View style={{ gap: theme.space[2] }}>
        <Text accessibilityRole="header" style={[theme.type.h1, { color: theme.color.textPrimary }]}>
          Kenalan dulu
        </Text>
        <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
          Satu langkah singkat, lalu langsung ke mobil kamu.
        </Text>
      </View>

      <AmCard role="working">
        <View style={{ gap: theme.space[4] }}>
          <View style={{ alignItems: "center", gap: theme.space[2] }}>
            <AmAvatar name={name === "" ? "?" : name} size={72} />
            {/*
              AM-55 AC1 asks for an optional profile photo. There is no upload
              endpoint, no photo column on `users`, and no storage story — so
              the honest thing is to say so rather than render a control that
              cannot work. Initials stand in until an upload story exists.
            */}
            <Text style={[theme.type.caption, { color: theme.color.textTertiary }]}>
              Foto profil belum bisa diunggah. Untuk sekarang, inisial nama kamu yang tampil.
            </Text>
          </View>

          <AmTextField
            label="Nama tampilan"
            value={draft.displayName}
            onChangeText={draft.setDisplayName}
            placeholder="Budi Santoso"
            hint="Nama ini yang dilihat pengguna lain."
            error={
              tooShort
                ? "Minimal 2 huruf."
                : (save.error?.fields?.display_name ?? undefined)
            }
          />
        </View>
      </AmCard>

      {save.error && save.error.kind !== "validation" ? (
        <Text style={[theme.type.body, { color: theme.color.semanticText.danger }]}>
          {save.error.message}
        </Text>
      ) : null}

      <AmButton
        label="Lanjut ke mobil saya"
        variant="accent"
        size="lg"
        disabled={!valid}
        loading={save.isPending}
        onPress={() => save.mutate(name)}
      />
    </ScrollView>
  );
}
```

- [ ] **Step 3: Verify**

```bash
bun run format
make mb-check
```

- [ ] **Step 4: Visual verification — mandatory, both themes**

On a simulator, open and screenshot every state, **light and dark**:

| State | How to reach it |
|---|---|
| Empty | fresh account, field untouched — the button is disabled |
| Typing / invalid | one character typed — "Minimal 2 huruf." under the field |
| Valid | a real name — the button enables |
| Submitting | tap Lanjut — the button shows its spinner |
| Validation error | stop the API and return a 422, or submit a name the server rejects — the message lands **under the field**, not in the banner |
| Offline | disable networking, tap Lanjut — "Tidak ada koneksi" in the banner |
| Draft resume | type a name, force-close the app, reopen — the name is still there |

Confirm the touch target of the button is ≥44pt without added padding, and
that nothing anywhere sets `allowFontScaling={false}`.

---

## Task 5: The six-step wizard

**Invoke `frontend-design` first.** The font/palette gate does **not** apply —
committed design system; `impeccable` refines within it.

**Files:**
- Create (or replace Plan A's placeholder): `apps/mobile/src/app/(onboarding)/vehicle.tsx`
- Create: `apps/mobile/src/features/onboarding/WizardProgress.tsx`

**Interfaces:**
- Consumes: everything Task 1 produces; `useDraft`, `canAdvance`,
  `WIZARD_STEPS` from Task 2; `AmSelect` (post-Task-3) and
  `VehiclePhotoPlaceholder` from Task 3; `setActiveVehicleId`, `useSession`
  from Plan A; the Garage tab route from Plan C.
- Produces: the route `/(onboarding)/vehicle`, and
  `WizardProgress({ step }: { readonly step: WizardStep })`.

**TDD: no** — screen composition and navigation, verified by running it. The
one piece of real logic it drives (the cascade) lives in Task 2, and Step 8
below is its explicit verification recipe.

**Big O:** one query per step, none inside a loop or a render. Six queries
total for a complete pass; TanStack Query dedupes and caches, so moving
backwards refetches nothing.

**Acceptance criteria:** AM-113 AC1, AC2, AC3 (first half), AC4, AC5;
AM-55 AC2, AC3, AC4.

- [ ] **Step 1: Write the progress indicator**

Create `apps/mobile/src/features/onboarding/WizardProgress.tsx`:

```tsx
import { StyleSheet, Text, View } from "react-native";

import { WIZARD_STEPS, type WizardStep } from "@/features/onboarding/draft";
import { numeric, useTheme } from "@/theme";

const LABELS: Record<WizardStep, string> = {
  brand: "Merek",
  model: "Model",
  generation: "Generasi",
  year: "Tahun",
  variant: "Varian",
  photo: "Foto",
};

export interface WizardProgressProps {
  readonly step: WizardStep;
}

/** AM-113 AC1: "kemajuan saya terlihat di setiap langkah". */
export function WizardProgress({ step }: WizardProgressProps) {
  const theme = useTheme();
  const position = WIZARD_STEPS.indexOf(step) + 1;
  const total = WIZARD_STEPS.length;

  return (
    <View style={{ gap: theme.space[2] }}>
      <View style={styles.row}>
        <Text style={[theme.type.label, { color: theme.color.textSecondary }]}>
          {LABELS[step]}
        </Text>
        <Text style={[theme.type.caption, numeric, { color: theme.color.textTertiary }]}>
          Langkah {position} dari {total}
        </Text>
      </View>
      {/*
        React Native has no <progress>, so accessibilityRole is the real
        element here rather than a stand-in for one.
      */}
      <View
        accessibilityRole="progressbar"
        accessibilityValue={{ min: 1, max: total, now: position }}
        style={{
          height: theme.space[1],
          borderRadius: theme.radius.pill,
          backgroundColor: theme.color.border,
          overflow: "hidden",
        }}
      >
        <View
          style={{
            width: `${(position / total) * 100}%`,
            height: "100%",
            backgroundColor: theme.color.accent,
          }}
        />
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", justifyContent: "space-between" },
});
```

- [ ] **Step 2: Write the wizard shell — draft-driven step, back navigation, no skip**

Create `apps/mobile/src/app/(onboarding)/vehicle.tsx`. This step writes the
frame; Steps 3–5 fill in the step bodies and the save.

```tsx
import { useQueryClient } from "@tanstack/react-query";
import { useRouter } from "expo-router";
import { useEffect } from "react";
import { ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmButton, AmSelect } from "@/components/input";
import { AmEmptyState, AmErrorState, AmSkeleton, useToast } from "@/components/state";
import { canAdvance, useDraft, WIZARD_STEPS, type WizardStep } from "@/features/onboarding/draft";
import { WizardProgress } from "@/features/onboarding/WizardProgress";
import {
  generationOptions,
  toOptions,
  useBrands,
  useGenerations,
  useModels,
  useVariants,
  variantOptions,
  yearOptions,
} from "@/features/vehicle/catalog";
import { describedAsFrom, useCreateVehicle } from "@/features/vehicle/createVehicle";
import { VehiclePhotoPlaceholder } from "@/features/vehicle/VehiclePhotoPlaceholder";
import { setActiveVehicleId, useSession } from "@/shared/session";
import { useTheme } from "@/theme";

export default function VehicleWizard() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const router = useRouter();
  const queryClient = useQueryClient();
  const toast = useToast();
  const { user } = useSession();
  const draft = useDraft();
  const create = useCreateVehicle();

  useEffect(() => {
    if (user) draft.adoptUser(user.id);
  }, [user, draft]);

  const brands = useBrands();
  const models = useModels(draft.brand?.id ?? null);
  const generations = useGenerations(draft.model?.id ?? null);
  const variants = useVariants(draft.generation?.id ?? null);

  const index = WIZARD_STEPS.indexOf(draft.step);
  const isLast = index === WIZARD_STEPS.length - 1;

  // AM-113 AC1: backwards is always allowed and clears nothing. The cascade
  // in the store fires on a CHANGED value, not on revisiting a step.
  const back = () => {
    const previous = WIZARD_STEPS[index - 1];
    if (previous) draft.goTo(previous);
  };

  const forward = () => {
    const next = WIZARD_STEPS[index + 1];
    if (next) draft.goTo(next);
  };

  return (
    <ScrollView
      contentContainerStyle={{
        padding: theme.pagePadding,
        paddingTop: insets.top + theme.space[6],
        paddingBottom: insets.bottom + theme.space[10],
        gap: theme.space[6],
      }}
      keyboardShouldPersistTaps="handled"
    >
      <View style={{ gap: theme.space[2] }}>
        <Text accessibilityRole="header" style={[theme.type.h1, { color: theme.color.textPrimary }]}>
          Mobil kamu apa?
        </Text>
        <WizardProgress step={draft.step} />
      </View>

      {/* Step body — Steps 3 and 4 below. */}

      <View style={{ gap: theme.space[3] }}>
        {/*
          There is NO skip control here, and there must never be one. AM-55
          AC2: "tidak ada tombol lewati, karena aplikasi tanpa mobil tidak
          punya isi". The enforcement is Plan A's route gate; this screen
          simply offers no way out, and adds no second guard of its own.
        */}
        <AmButton
          label={isLast ? "Simpan mobil saya" : "Lanjut"}
          variant="accent"
          size="lg"
          disabled={!canAdvance(draft, draft.step)}
          loading={create.isPending}
          onPress={isLast ? save : forward}
        />
        {index > 0 ? <AmButton label="Kembali" variant="ghost" onPress={back} /> : null}
      </View>
    </ScrollView>
  );
}
```

- [ ] **Step 3: Write the shared step body for the five picker steps**

Add above the `return`, inside the component. Each step is the same shape —
loading, error, empty, or a select — so it is written once and the differences
are data.

```tsx
  // Rendered in place of the picker while a level is loading, has failed, or
  // has come back empty. An empty catalog level is a real state on a fresh
  // database (`make db-drop` without `make db-seed`), not a fault.
  const stepBody = () => {
    switch (draft.step) {
      case "brand":
        if (brands.isPending) return <Skeletons />;
        if (brands.isError)
          return (
            <AmErrorState
              title="Katalog gagal dimuat"
              body={brands.error.message}
              onRetry={() => void brands.refetch()}
            />
          );
        if (brands.data.length === 0)
          return (
            <AmEmptyState
              title="Katalog masih kosong"
              body="Belum ada merek yang bisa dipilih. Coba muat ulang sebentar lagi."
              actionLabel="Muat ulang"
              onAction={() => void brands.refetch()}
            />
          );
        return (
          <AmSelect
            label="Merek"
            value={draft.brand?.id ?? null}
            options={toOptions(brands.data)}
            placeholder="Pilih merek"
            onChange={(id) => {
              const picked = brands.data.find((entry) => entry.id === id);
              if (picked) draft.setBrand({ id: picked.id, name: picked.name });
            }}
          />
        );

      case "model":
        if (models.isPending) return <Skeletons />;
        if (models.isError)
          return (
            <AmErrorState
              title="Model gagal dimuat"
              body={models.error.message}
              onRetry={() => void models.refetch()}
            />
          );
        if (models.data.length === 0)
          return (
            <AmEmptyState
              title="Belum ada model untuk merek ini"
              body="Katalog belum mencatat model apa pun di bawah merek ini."
              actionLabel="Pilih merek lain"
              onAction={back}
            />
          );
        return (
          <AmSelect
            label="Model"
            value={draft.model?.id ?? null}
            options={toOptions(models.data)}
            placeholder="Pilih model"
            onChange={(id) => {
              const picked = models.data.find((entry) => entry.id === id);
              if (picked) draft.setModel({ id: picked.id, name: picked.name });
            }}
          />
        );

      case "generation":
        if (generations.isPending) return <Skeletons />;
        if (generations.isError)
          return (
            <AmErrorState
              title="Generasi gagal dimuat"
              body={generations.error.message}
              onRetry={() => void generations.refetch()}
            />
          );
        if (generations.data.length === 0)
          return (
            <AmEmptyState
              title="Belum ada generasi untuk model ini"
              body="Katalog belum mencatat generasi apa pun di bawah model ini."
              actionLabel="Pilih model lain"
              onAction={back}
            />
          );
        return (
          <AmSelect
            label="Generasi"
            value={draft.generation?.id ?? null}
            options={generationOptions(generations.data)}
            placeholder="Pilih generasi"
            onChange={(id) => {
              const picked = generations.data.find((entry) => entry.id === id);
              if (picked)
                draft.setGeneration({
                  id: picked.id,
                  name: picked.name,
                  yearStart: picked.year_start,
                  yearEnd: picked.year_end,
                  years: picked.years,
                });
            }}
          />
        );

      case "year": {
        // AM-113 AC2: the range comes from the chosen generation, and the
        // server supplies it — year_start with year_end, where null means the
        // generation is still in production.
        const generation = draft.generation;
        if (!generation) return null;
        return (
          <AmSelect
            label={`Tahun (${generation.years})`}
            value={draft.year === null ? null : String(draft.year)}
            options={yearOptions({
              id: generation.id,
              name: generation.name,
              year_start: generation.yearStart,
              year_end: generation.yearEnd,
              years: generation.years,
            })}
            placeholder="Pilih tahun"
            onChange={(value) => draft.setYear(Number(value))}
          />
        );
      }

      case "variant":
        if (variants.isPending) return <Skeletons />;
        if (variants.isError)
          return (
            <AmErrorState
              title="Varian gagal dimuat"
              body={variants.error.message}
              onRetry={() => void variants.refetch()}
            />
          );
        if (variants.data.length === 0)
          return (
            <AmEmptyState
              title="Belum ada varian untuk generasi ini"
              body="Kamu tetap bisa lanjut. Varian menentukan kode mesin dan transmisi, dan itu bisa dilengkapi nanti."
              actionLabel="Lanjut tanpa varian"
              onAction={() => {
                draft.skipVariant();
                forward();
              }}
            />
          );
        return (
          <View style={{ gap: theme.space[3] }}>
            <AmSelect
              label="Varian"
              value={draft.variant?.id ?? null}
              options={variantOptions(variants.data)}
              placeholder="Pilih varian"
              onChange={(id) => {
                const picked = variants.data.find((entry) => entry.id === id);
                if (picked) draft.setVariant({ id: picked.id, name: picked.name });
              }}
            />
            {/*
              AM-113 AC3. This skips the VARIANT, which is what carries the
              engine code and transmission — it is not a skip of the wizard.
              Filling it in later is the vehicle page, which is AM-116; the
              server already accepts it through PUT /vehicles/{id}.
            */}
            <AmButton
              label="Saya tidak tahu varian mobil saya"
              variant="ghost"
              onPress={() => {
                draft.skipVariant();
                forward();
              }}
            />
          </View>
        );

      case "photo":
        return (
          <View style={{ gap: theme.space[3] }}>
            <VehiclePhotoPlaceholder />
            <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
              Unggah foto belum tersedia. Mobil kamu akan tampil dengan gambar netral ini
              sampai fiturnya siap.
            </Text>
          </View>
        );
    }
  };
```

And the small loading component, at module scope below the screen:

```tsx
function Skeletons() {
  const theme = useTheme();
  return (
    <View style={{ gap: theme.space[2] }}>
      <AmSkeleton height={20} width="40%" />
      <AmSkeleton height={52} />
    </View>
  );
}
```

Render it in the frame with `{stepBody()}` where Step 2 left the comment.

- [ ] **Step 4: Write the save, in the order that does not loop the gate**

Add above the `return`, inside the component:

```tsx
  const save = () => {
    const brand = draft.brand;
    const model = draft.model;
    if (!brand || !model) return;

    // Captured BEFORE the invalidation below flips it. AM-113 AC5 branches on
    // whether this is the account's first car, and `me` is about to say it is
    // not.
    const wasFirstCar = user?.hasVehicles === false;

    create.mutate(
      {
        variantId: draft.variant?.id ?? null,
        describedAs: describedAsFrom(brand.name, model.name, draft.year),
        year: draft.year,
      },
      {
        onSuccess: async ({ id }) => {
          setActiveVehicleId(id);

          // Awaited, not fired and forgotten. The onboarding gate routes on
          // Me.hasVehicles; navigating before this resolves sends the person
          // to a route the gate immediately bounces back into this wizard.
          await queryClient.invalidateQueries({ queryKey: ["me"] });
          await queryClient.invalidateQueries({ queryKey: ["vehicles"] });

          const vehicleName = describedAsFrom(brand.name, model.name, draft.year);
          draft.clear();

          if (wasFirstCar) {
            router.replace({
              pathname: "/(onboarding)/aha",
              params: { vehicleId: id, vehicleName },
            });
            return;
          }

          // AM-113 AC5 sends a non-first car to "halaman kendaraannya" —
          // AM-116, which has no screen. The garage is the honest substitute
          // and the car is already active there.
          toast({ message: `${vehicleName} masuk garasi.`, tone: "success" });
          router.replace("/(app)/garage");
        },
      },
    );
  };
```

Add the failure banner beside the buttons:

```tsx
      {create.error ? (
        <Text style={[theme.type.body, { color: theme.color.semanticText.danger }]}>
          {create.error.message}
        </Text>
      ) : null}
```

- [ ] **Step 5: Confirm the two routes this task assumes**

The garage href (`/(app)/garage`) and the aha pathname are literals this task
guesses. Run `make mb-check` — expo's typed routes will reject a path that
does not exist. Read `apps/mobile/src/app/(app)/` for the real tab route name
and fix the literal; **correct this plan file in place** so Task 6 and the
review do not repeat the guess.

- [ ] **Step 6: Verify the gate**

```bash
bun run format
make mb-check
```

- [ ] **Step 7: Visual verification — mandatory, both themes**

With `make db-up && make db-seed`, on a simulator, screenshot **light and
dark**:

| State | How to reach it |
|---|---|
| Loading | open the brand step with the API slow or throttled |
| Populated | the seeded catalog — brand list scrolls inside the sheet |
| Empty (brand) | `make db-drop` without `make db-seed` — "Katalog masih kosong" + Muat ulang |
| Empty (child) | a seeded brand whose model has no generations — "Pilih model lain" |
| Empty (variant) | a generation with no variants — "Lanjut tanpa varian" advances |
| Error | stop the API mid-step — `AmErrorState` with a retry that works |
| Year step | a finished generation (2015–2021) and an in-production one (2021–); the in-production list must end at the current year |
| Variant skip | tap "Saya tidak tahu varian mobil saya" — advances to photo |
| Photo step | the neutral placeholder and its one-line explanation |
| Saving | tap Simpan — spinner, then the aha screen |
| Mid-draft resume | see Step 8 |

Also confirm, on every step: the progress bar advances, "Kembali" appears from
step 2 onward, and **there is no skip control for the wizard itself**.

- [ ] **Step 8: Verify the cascade and the resume — the two behaviours with no test**

This is the explicit replacement for the tests Task 2 could not write. Run all
four and record the result in `## Execution status`.

1. **Back preserves.** Pick Toyota → Avanza → Gen 3 → 2022. Go back to the
   brand step. Re-select **Toyota**. Go forward. Model, generation, and year
   are all still selected.
2. **Change clears.** From the same state, go back to the brand step and
   select **Honda**. Go forward: the model step is empty, and the generation,
   year, and variant are cleared. The brand step does not offer stale models.
3. **Force-close resumes (AM-113 AC4).** Reach step four (year) with brand,
   model, and generation chosen. Force-quit the app from the app switcher —
   not a reload. Reopen. The wizard opens **on the year step** with all three
   earlier choices intact and the progress bar reading "Langkah 4 dari 6".
4. **A second account sees nothing.** Sign out mid-wizard, sign in as a
   different account, and reach the wizard. It starts at the brand step with
   an empty draft.

---

## Task 6: The aha screen, in AC2 mode

**Invoke `frontend-design` first.** The font/palette gate does **not** apply —
committed design system; `impeccable` refines within it.

**Files:**
- Create (or replace Plan A's placeholder): `apps/mobile/src/app/(onboarding)/aha.tsx`

**Interfaces:**
- Consumes: `useAhaSeen` from Task 2; `AmCard`, `AmButton`,
  `VehiclePhotoPlaceholder`; the Home and Garage tab routes from Plan C.
- Produces: the route `/(onboarding)/aha`, taking `vehicleId` and
  `vehicleName` as params.

**TDD: no** — a static screen with one persisted flag, verified by running it.

**Acceptance criteria:**

- **AM-56 AC1 — deferred and unmet.** Nothing on the server computes build,
  known-issue, part, or community counts, and the project forbids invented
  numbers. **The seam is composition, not scaffolding:** the "be the first"
  block is its own component in this file, so a `CommunityCounts` component
  can be placed above it when an endpoint exists, without the screen being
  redesigned. No placeholder component, no feature flag, and no prop threaded
  through for a caller that does not exist — those would be scaffolding for a
  need that has no date.
- **AM-56 AC2 — met.** No zeros are rendered. The invitation carries one
  concrete action: "Lihat garasi saya", which works. The ticket's illustrative
  action ("mencatat modifikasi pertama") is deferred with the build form.
- **AM-56 AC3 — met.** The invitation is present at the bottom in every state.
  The AI menu itself is out of scope by the ticket's own line, so this is copy
  and not a link into an absent screen.
- **AM-56 AC4 — met.** "Lanjut" goes to the home tab with the car active, and
  the screen never appears again for that vehicle.
- **AM-55 AC4 — met.** This is where a completed wizard lands.

- [ ] **Step 1: Write the screen**

Create `apps/mobile/src/app/(onboarding)/aha.tsx`:

```tsx
import { useLocalSearchParams, useRouter } from "expo-router";
import { useEffect } from "react";
import { ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmCard } from "@/components/display";
import { AmButton } from "@/components/input";
import { useAhaSeen } from "@/features/onboarding/ahaSeen";
import { VehiclePhotoPlaceholder } from "@/features/vehicle/VehiclePhotoPlaceholder";
import { useTheme } from "@/theme";

/**
 * AM-56, AC2 mode.
 *
 * AC1 wants build, known-issue, part, and community counts for the car. No
 * endpoint computes them, and this project does not seed fake data — so the
 * counts are absent rather than zeroed, which is also what AC2 asks for
 * ("angka nol tidak ditampilkan").
 *
 * The seam for AC1 is this file's composition: when an endpoint exists, a
 * CommunityCounts block is placed above <FirstHere /> and the rest of the
 * screen is untouched. Nothing is scaffolded for it in advance.
 */
export default function AhaScreen() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const router = useRouter();
  const markSeen = useAhaSeen((state) => state.markSeen);
  const seen = useAhaSeen((state) => state.seen);

  const { vehicleId, vehicleName } = useLocalSearchParams<{
    vehicleId: string;
    vehicleName: string;
  }>();

  // AC4's other half: a stale link back here after the person has moved on
  // goes straight home rather than replaying the moment.
  useEffect(() => {
    if (vehicleId && seen.includes(vehicleId)) router.replace("/(app)");
  }, [vehicleId, seen, router]);

  const leave = (href: "/(app)" | "/(app)/garage") => {
    if (vehicleId) markSeen(vehicleId);
    router.replace(href);
  };

  return (
    <ScrollView
      contentContainerStyle={{
        padding: theme.pagePadding,
        paddingTop: insets.top + theme.space[8],
        paddingBottom: insets.bottom + theme.space[10],
        gap: theme.space[6],
      }}
    >
      <View style={{ gap: theme.space[2] }}>
        <Text accessibilityRole="header" style={[theme.type.display, { color: theme.color.textPrimary }]}>
          {vehicleName ?? "Mobil kamu"} sudah masuk garasi
        </Text>
        <Text style={[theme.type["body-lg"], { color: theme.color.textSecondary }]}>
          Ini titik awal garasi digital kamu.
        </Text>
      </View>

      <VehiclePhotoPlaceholder />

      <FirstHere onAction={() => leave("/(app)/garage")} />

      <AiInvitation />

      <AmButton label="Lanjut" variant="accent" size="lg" onPress={() => leave("/(app)")} />
    </ScrollView>
  );
}

/**
 * AC2. Deliberately not AmEmptyState: this is not an empty list, it is the
 * opening move of the platform, and it reads as an invitation rather than an
 * absence.
 */
function FirstHere({ onAction }: { readonly onAction: () => void }) {
  const theme = useTheme();
  return (
    <AmCard role="working">
      <View style={{ gap: theme.space[3] }}>
        <Text style={[theme.type.h3, { color: theme.color.textPrimary }]}>
          Jadilah yang pertama
        </Text>
        <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
          Belum ada yang berbagi soal mobil ini. Apa pun yang kamu catat — modifikasi, servis,
          masalah yang kamu temui — jadi rujukan pertama buat pemilik berikutnya.
        </Text>
        <AmButton label="Lihat garasi saya" onPress={onAction} />
      </View>
    </AmCard>
  );
}

/**
 * AC3: the invitation is present in every state. The AI menu itself is E8 and
 * is out of scope by the ticket's own line, so this says what will be
 * possible rather than linking into a screen that does not exist. It is not a
 * disabled button — a control that cannot be pressed is worse than a sentence
 * that is honest.
 */
function AiInvitation() {
  const theme = useTheme();
  return (
    <AmCard role="surface">
      <View style={{ gap: theme.space[2] }}>
        <Text style={[theme.type.label, { color: theme.color.textSecondary }]}>
          AnakMobil AI
        </Text>
        <Text style={[theme.type.body, { color: theme.color.textPrimary }]}>
          Nanti kamu bisa tanya apa saja soal mobil ini — servis, part yang cocok, keluhan
          yang sering muncul — dan jawabannya bersandar pada catatan pemilik lain.
        </Text>
      </View>
    </AmCard>
  );
}
```

- [ ] **Step 2: Confirm the home and garage route literals**

`/(app)` and `/(app)/garage` are guesses at Plan C's tab routes. `make
mb-check` fails on a path expo's typed routes do not know. Read
`apps/mobile/src/app/(app)/` for the real names, fix them, and **correct this
plan file in place**.

- [ ] **Step 3: Verify**

```bash
bun run format
make mb-check
```

- [ ] **Step 4: Visual verification — mandatory, both themes**

| State | How to reach it |
|---|---|
| Populated (the only data state) | finish the wizard as a first car |
| Long name | a car whose composed name wraps two lines — the heading must not clip |
| Already seen | press Lanjut, then navigate back to the route manually — it redirects home instead of showing |
| Missing params | open the route with no params — it renders "Mobil kamu" rather than crashing |

Confirm by eye that **no zero appears anywhere on this screen**, that the AI
invitation is visible without scrolling on a small device (or is clearly
reachable), and that both buttons clear 44pt.

- [ ] **Step 5: Walk the whole flow once, end to end**

Register a new account and go straight through: register → profile → six steps
→ aha → home. Confirm the car is the active vehicle on the home tab, and that
signing out and back in does **not** show the aha screen again. AM-55's
technical note targets under 90 seconds for this walk — time it and record the
number in `## Execution status`. If it is well over, say so; the likely cause
is the extra tap per step that `AmSelect`'s trigger-then-sheet pattern costs,
and that is an owner decision rather than a defect (see Open questions).

---

## Execution mode

**Run shape.** Six tasks, and this plan has the most genuinely parallel work
of the four in the series.

**1. What runs in parallel, and what is serialised on what.**

```
wave 1 (no edges between them — different directories, no shared file):
  T1  features/vehicle/{types,catalog,createVehicle}.ts
  T2  features/onboarding/{draft,ahaSeen}.ts + shared/session/signOut.ts
  T3  components/input/AmSelect.tsx + features/vehicle/VehiclePhotoPlaceholder.tsx

then, the moment T2 lands:
  T4  app/(onboarding)/profile.tsx      consumes T2 only
  T6  app/(onboarding)/aha.tsx          consumes T2 only

then, once T1 AND T2 AND T3 have all landed:
  T5  app/(onboarding)/vehicle.tsx      consumes all three
```

T3 touches `features/vehicle/` and so does T1, but on different files with no
import between them — `VehiclePhotoPlaceholder.tsx` imports nothing from
`catalog.ts`. That is a shared directory, not a shared file, and it does not
serialise.

Peak concurrency is three writers in wave 1, then two (T4, T6) while T5 waits
on T3. The critical path is **T3 → T5 → done**, so T3 is the one to dispatch
first if the queue can only start one.

**2. What the writers cannot discover for themselves.** The environment card
above, pasted verbatim into every brief — in particular note 13 (the
snake_case types are deliberate), note 14 (the RNGH scrollable, and the
one-line fallback with its re-verification cost), note 15 (route-group
literals and typed routes), note 16 (**the four things Plan A owns that must
be read, not assumed**), and note 17 (`make db-drop` is how you see an empty
catalog). Plus the four findings that are not in the spec: no upload endpoint
anywhere, the API supplies the year range, a skipped variant forces
`described_as`, and the `me` invalidation must be awaited before navigating.

**3. Where the risk concentrates.**

- **T5's save ordering** is the expensive one. Navigating before `me` is
  invalidated puts the person in a gate loop, and reading `hasVehicles` after
  the invalidation sends every first car to the garage instead of the aha
  screen. Both are silent — they look like a working app that behaves wrongly.
  The reviewer floor for T5 is `opus`.
- **T3 modifies a shipped design-system primitive** used by an existing
  screen. It is four lines, but a regression there is a regression in
  `catalog.tsx` and in every future caller. Its review re-checks the existing
  Transmisi select, not only the new behaviour.
- **T2's cascade** is the piece with a real invariant and no test. Its
  verification is Step 8 of T5, which means a T2 defect surfaces two tasks
  later — worth the reviewer reading the early-return guards specifically.
- **T2 edits `signOut.ts`**, a Plan A file inside a transaction whose ordering
  is load-bearing (the epoch increments first, exactly one redirect happens).
  Two added lines must not reorder it.

Nothing in this plan touches money, auth, access control, sessions, a state
machine, a migration, a column, or a public contract — the backend is not
modified at all. `signOut.ts` is the closest thing to a session touch and is
additive.

**4. Anything the plan is missing.** Nothing known. Every task carries `Files:`,
`Interfaces:`, a TDD verdict with its reason, and acceptance criteria traced
to a ticket. The `Tidak boleh ada` block above carries the spec's anti-goals.
Four route literals (`/(onboarding)/vehicle`, `/(onboarding)/aha`, `/(app)`,
`/(app)/garage`) and two query keys (`["me"]`, `["vehicles"]`) are **stated
assumptions with a named step that verifies each** — they are not gaps, but
they are the first thing to correct in this file if execution finds them wrong.

**Tiers.** T1 and T2 are `sonnet` writers — the code is here, the judgement is
in following it. T3 is `sonnet`. T4 and T6 are `sonnet`. T5 is `sonnet` with
the save block reviewed at `opus`; if the writer struggles with the ordering,
promote it rather than iterating. Reviewers: `opus` for T2, T3, and T5;
`sonnet` for T1, T4, and T6, resolved upward from the `sonnet` writers.

**No commits between tasks.** Work accumulates for the owner's review; the
repository's `CLAUDE.md` puts commits at the end, on a feature branch, into a
pull request against `dev`.

---

## Open questions for the owner

Neither blocks execution.

1. **`AmSelect` costs one extra tap per step** (tap the trigger, then tap the
   option) against AM-55's under-90-seconds target — six extra taps across the
   wizard. The repo rule that every picker goes through `AmBottomSheet`
   (AM-27's definition of done) is what this plan follows. Rendering each
   step's options directly as screen content would be faster and is arguably
   not "a picker" at all, since a wizard step is a screen rather than a field.
   Time the walk in Task 6 Step 5 before deciding; changing it later is a
   contained edit to one file.
2. **AM-56 AC1's counters** need an endpoint that returns build, known-issue,
   part, and community counts for a vehicle. `GET /vehicles/{id}/summary` is a
   *service* summary and is not it. Whether that endpoint lands before launch
   is the decision the spec already flagged as still owed.

---

## Execution status

| Task | Status | Notes |
|---|---|---|
| 1 — vehicle API layer | not started | |
| 2 — draft + seen-aha stores | not started | |
| 3 — AmSelect scroll + placeholder | not started | |
| 4 — profile step | not started | |
| 5 — the six-step wizard | not started | |
| 6 — the aha screen | not started | |

Record here, per task: corrections the plan got wrong, deliberate cuts,
defects found and how they were closed, whether the task was written inline or
dispatched, and — for Tasks 4, 5, and 6 — that the visual states were opened
in both themes. Task 5 additionally records the four cascade/resume results
from its Step 8; Task 6 records the end-to-end walk time.

---

## Review findings ledger

| # | Task | Severity | File:line | Failure scenario | Smallest fix | Closed by |
|---|---|---|---|---|---|---|

Severity vocabulary: `structural` (a column, constraint, or public contract —
raise and fix immediately) · `correctness` · `test-integrity` · `hygiene`.
Everything but `structural` is worked in one fix pass after the final task.
</content>
</invoke>
