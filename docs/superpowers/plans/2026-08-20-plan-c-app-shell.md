# Plan C — App shell (AM-16) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `executing-plans-hybrid` to implement this
> plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Five tabs that keep their own scroll position and navigation stack, a real Home screen
built from the account's own vehicles, honest empty states for the tabs whose features do not
exist, a Profile tab that can end the session, and the global add action wired end-to-end behind a
registry that is empty until a form exists to add to.

**Architecture:** One expo-router `Tabs` navigator at `app/(app)/_layout.tsx`. Every tab is a
**directory** with its own `_layout.tsx` rendering `<Stack>`, which is what makes AC1 the
navigator's own behaviour rather than something a screen arranges. Server state comes from a
single `GET /vehicles` query through TanStack Query (Plan A); client state — the active vehicle —
comes from Plan A's zustand store, with the vehicle list as the authority and the store healed to
match. Everything visual goes through the AM-15 design system; no screen draws a surface itself.

**Tech Stack:** React Native 0.86 · Expo SDK 57 · expo-router 57.0.14 (typed routes + React
Compiler on) · TanStack Query + zustand (from Plan A) · the AM-15 design system in
`apps/mobile/src/{theme,components}` · one new dependency, `@expo/vector-icons` (Task 2).

**Spec:** [`docs/superpowers/specs/2026-08-20-am-17-auth-session-onboarding-design.md`](../specs/2026-08-20-am-17-auth-session-onboarding-design.md)

**Ticket:** [AM-16](https://oksasatyaa.atlassian.net/browse/AM-16) — *Shell navigasi dan aksi tambah global*.
Its out-of-scope line — *"Isi setiap tab dan isi setiap formulir tambah"* — is what bounds this
plan. Home is the one carve-out: AC3 needs an active vehicle, and an active vehicle needs a screen
that shows one.

---

## Global Constraints

Every task's requirements implicitly include this section.

- **This plan is third of four.** Plan A (session foundation) and Plan B (auth screens) land
  first. Code against the frozen contract in *Interfaces from Plan A* below; do not redefine any
  of it, and do not install TanStack Query, zustand, or an HTTP client — Plan A brings them.
- **Nothing is seeded with fake data.** No invented counts, no sample vehicles, no placeholder
  community numbers. The launch state is empty and says so.
- **No new hex, font size, spacing, or radius literal anywhere.** Every design value comes from
  `useTheme()`. A raw number in a style is a blocker, not a nit. Layout-only keys
  (`flexDirection`, `alignItems`, `position`) may live in `StyleSheet.create`; design values go
  inline, which is the pattern every existing primitive follows.
- **Product strings are Bahasa Indonesia.** Code, comments, file names, and this plan are English.
- **Touch targets ≥ 44 pt without caller padding.** `AmButton`, `AmChip`, and `AmSelect`'s option
  rows already enforce it; any bespoke `Pressable` sets `minHeight: theme.touchTargetMin` itself.
- **No native pickers, dialogs, or `Alert`.** `AmBottomSheet` is the pattern — `AmSelect` already
  wraps it and is the right answer for anything that picks one value from a list.
- **No tab content beyond an honest empty state** for Garage, Explore, and Community. **No
  add-forms** — they belong to other epics.
- **Do not commit and do not push.** Work accumulates in the working tree for the owner's review;
  commits happen once at the end (repository `CLAUDE.md`, "Working here"). This is why no task
  below ends with a commit step, unlike the default plan template.
- **`make mb-check` is the gate**, run from the repository root, and its exit code is the
  evidence — never a summary of what you expect it to print.

### Interfaces from Plan A — frozen, do not redefine

```ts
export interface Me { id: string; email: string; username: string | null; displayName: string | null; hasVehicles: boolean }
export type ApiErrorKind = "offline" | "validation" | "rateLimited" | "unauthorized" | "server";
export interface ApiError { kind: ApiErrorKind; message: string; fields?: Record<string, string>; retryAfterSeconds?: number }
export function apiRequest<T>(path: string, init?: { method?: string; body?: unknown; signal?: AbortSignal }): Promise<T>;
export type SessionStatus = "loading" | "signedOut" | "signedIn";
export function useSession(): { status: SessionStatus; user: Me | null };
export function signOut(): Promise<void>;
export function useActiveVehicleId(): string | null;
export function setActiveVehicleId(id: string | null): void;
```

**Two assumptions this plan makes about Plan A, to be checked in Task 1 step 1 and corrected in
this file if wrong** (§29 step 1 — a correction to the plan is written down, not carried in
someone's head):

1. **`apiRequest<T>` resolves the envelope's `data` payload**, not the whole
   `{ meta, data, error }` envelope. If it resolves the envelope, every `apiRequest<T>` call site
   in this plan takes `T` = the envelope type and reads `.data`.
2. **Module paths**, taken from the spec's architecture block: `@/shared/api/client`
   (`apiRequest`), `@/shared/api/errors` (`ApiError`), `@/shared/session/store` (`useSession`,
   `useActiveVehicleId`, `setActiveVehicleId`), `@/shared/session/signOut` (`signOut`). Plan A may
   have put `useActiveVehicleId` elsewhere. One `grep -rn "useActiveVehicleId" apps/mobile/src`
   settles it; fix the imports and record the real paths here.

### AM-16 acceptance criteria, and where each is met

| AC | What it asks | Where |
|---|---|---|
| **AC1** | Leaving a tab and returning preserves scroll position **and** the navigation stack | Task 2 (structure + scroll evidence), re-checked in Task 3 |
| **AC2** | A global add action offering modifikasi, servis, problem, foto | Task 5 — as mechanism with zero available entries; see the decision below |
| **AC3** | The add action opens with the active vehicle pre-filled and changeable | Task 5 (add sheet) and Task 2 (the same `AmSelect` switcher on Home) |

**AC2 ships as a mechanism, not as a visible button, and the ticket comment must say so.** All four
entries route to forms that belong to other epics and do not exist. The spec's rule is that such an
entry is *absent*, never present-and-broken — so the registry is empty, and a "+" that opens a
sheet with nothing in it is precisely the dead end `AmEmptyState`'s design note exists to prevent.
The button therefore renders `null` while the registry is empty. Task 5 verifies the whole path by
temporarily pointing one entry at the existing `/catalog` route, screenshotting it, and reverting.

---

## File structure

```
apps/mobile/src/
  app/(app)/_layout.tsx              Tabs navigator, tab bar, order, anchor   T2 (edited T3, T4)
  app/(app)/home/_layout.tsx         the Home stack                           T2
  app/(app)/home/index.tsx           the real Home screen                     T2
  app/(app)/garage/{_layout,index}   honest empty tab                         T3
  app/(app)/explore/{_layout,index}  honest empty tab                         T3
  app/(app)/community/{_layout,index} honest empty tab                        T3
  app/(app)/profile/{_layout,index}  identity + sign-out                      T4
  components/shell/TabScreen.tsx     safe area + tab-bar inset + the FAB slot  T2 (edited T5)
  components/shell/TabStack.tsx      the per-tab <Stack>, written once         T2
  components/shell/AddButton.tsx     the global add action                     T5
  components/shell/index.ts          barrel                                    T2 (edited T5)
  features/garage/types.ts           wire shapes for GET /vehicles             T1
  features/garage/format.ts          rupiah, kilometres, short date            T1
  features/garage/queries.ts         useVehicles()                             T1
  features/garage/useActiveVehicle.ts active-vehicle resolution + healing      T1
  features/garage/VehicleCard.tsx    the active car and its service summary    T2
  features/shell/errorCopy.ts        ApiError -> what a person reads           T1
  features/shell/addActions.ts       the add registry                          T5
```

`packages/api-types` is not scaffolded (repository `CLAUDE.md`), so `features/garage/types.ts` is
hand-written against the Rust response types and says so in its own comment.

### What the tab bar looks like after each task

Tab order is set by the order of `<Tabs.Screen>` children in `(app)/_layout.tsx`
(`expo-router/build/useScreens.js:63` — `getSortedChildren` uses the declared order when there is
one, and falls back to file-system sort when there is not). Each task inserts its screen in the
right slot, so the bar grows without ever being reordered:

| After | Tabs in the bar |
|---|---|
| Task 2 | Beranda |
| Task 3 | Beranda · Garasi · Jelajah · Komunitas |
| Task 4 | Beranda · Garasi · Jelajah · Komunitas · Profil |

---

## Task 1: Vehicle data layer and Indonesian formatters

The only task that writes no UI, and the reason it is first: every screen below imports from it,
and a screen written against an invented shape is the failure mode this task exists to remove.

**Files:**
- Create: `apps/mobile/src/features/garage/types.ts`
- Create: `apps/mobile/src/features/garage/format.ts`
- Create: `apps/mobile/src/features/garage/queries.ts`
- Create: `apps/mobile/src/features/garage/useActiveVehicle.ts`
- Create: `apps/mobile/src/features/shell/errorCopy.ts`
- Test: none — `apps/mobile` has no test runner and this work does not add one.

**Interfaces:**
- Consumes: `apiRequest` (`@/shared/api/client`), `ApiError` (`@/shared/api/errors`),
  `useActiveVehicleId` / `setActiveVehicleId` (`@/shared/session/store`), `useQuery`
  (`@tanstack/react-query`) — all from Plan A.
- Produces:
  ```ts
  export interface VehicleSummary { readonly service_count: number; readonly total_cost: string | null; readonly last_service_date?: string; readonly overdue_count: number; readonly due_soon_count: number }
  export interface Vehicle { readonly id: string; readonly variant_id: string | null; readonly name: string; readonly nickname: string | null; readonly year: number | null; readonly colour: string | null; readonly mileage_km: number | null; readonly position: number; readonly summary?: VehicleSummary }
  export const vehiclesQueryKey: readonly ["vehicles"];
  export function useVehicles(): UseQueryResult<Vehicle[], Error>;
  export function useActiveVehicle(vehicles: readonly Vehicle[] | undefined): Vehicle | null;
  export function formatRupiah(decimal: string): string;
  export function formatKilometres(km: number): string;
  export function formatShortDate(iso: string): string;
  export function errorBody(error: unknown): string;
  ```

**TDD: no** — `apps/mobile` has no test runner (spec, §Testing). The formatters are pure and are
written so they can be covered the day a runner arrives; until then they are verified by reading
their output on the Home screen in Task 2, against a real vehicle with real service history.

**Acceptance criteria:** `make mb-check` is green with these five files present; `useVehicles`
issues exactly one request to `GET /vehicles`; `useActiveVehicle` returns the stored vehicle when
it is still in the list, the first vehicle when it is not, and `null` for an empty list, and never
writes to the store during render.

- [ ] **Step 1: Verify the two Plan A assumptions before writing a line**

```bash
grep -rn "export function apiRequest" -A 12 apps/mobile/src/shared/api/
grep -rn "useActiveVehicleId\|setActiveVehicleId\|export function useSession\|export function signOut" apps/mobile/src/shared/
```

Read what `apiRequest` actually resolves and where the active-vehicle hooks live. If either
assumption in *Global Constraints* is wrong, fix the imports in every task below **and edit this
plan file to record the real shape** — a correction that stays in the transcript is lost to the
next reader.

- [ ] **Step 2: Write the wire shapes**

`apps/mobile/src/features/garage/types.ts`:

```ts
/**
 * `GET /vehicles`, on the wire.
 *
 * Hand-written against `VehicleResponse` and `ListSummaryResponse` in
 * apps/api/crates/runtime/src/adapter/http/{vehicles,summary}.rs. Neither
 * struct carries `#[serde(rename_all)]`, so every key is the Rust field name
 * unchanged — snake_case, deliberately, rather than a camelCase copy that
 * would have to be mapped somewhere.
 *
 * `packages/api-types` does not exist yet; when it does, this file is what it
 * replaces.
 *
 * There is no plate, VIN, or price here, and there is no field for one. That
 * is the server's privacy boundary (vehicles.rs module docs), and this type
 * mirrors it rather than re-deciding it.
 */
export interface VehicleSummary {
  readonly service_count: number;
  /** A decimal string — money, never a JSON number. Null when nothing is recorded. */
  readonly total_cost: string | null;
  /** ISO `YYYY-MM-DD`. Absent (not null) when there is no service history. */
  readonly last_service_date?: string;
  readonly overdue_count: number;
  readonly due_soon_count: number;
}

export interface Vehicle {
  readonly id: string;
  readonly variant_id: string | null;
  /** The nickname, else the catalog name, else what the owner typed. Never empty. */
  readonly name: string;
  readonly nickname: string | null;
  readonly year: number | null;
  readonly colour: string | null;
  readonly mileage_km: number | null;
  readonly position: number;
  /** Present on the list endpoint; the server omits it nowhere today. */
  readonly summary?: VehicleSummary;
}
```

- [ ] **Step 3: Write the formatters**

`apps/mobile/src/features/garage/format.ts`:

```ts
const MONTHS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "Mei",
  "Jun",
  "Jul",
  "Agu",
  "Sep",
  "Okt",
  "Nov",
  "Des",
] as const;

/** 4200000 -> "4.200.000". Indonesian groups thousands with dots. */
function groupThousands(digits: string): string {
  return digits.replace(/\B(?=(\d{3})+(?!\d))/g, ".");
}

/**
 * "4200000.00" -> "Rp 4.200.000".
 *
 * The server sends money as a decimal string on purpose — a JSON number is a
 * double in most clients — so this formats the string rather than parsing it.
 * Nothing on this screen does arithmetic with a price, so nothing needs to
 * turn one into a number and risk rounding it.
 *
 * Sen are dropped: `summary.rs` pins the scale at two and rupiah has no
 * subunit in practice, so "4.200.000,00" would be noise on a card.
 */
export function formatRupiah(decimal: string): string {
  const whole = decimal.split(".")[0] ?? "0";
  const negative = whole.startsWith("-");
  const digits = negative ? whole.slice(1) : whole;
  return `${negative ? "-" : ""}Rp ${groupThousands(digits)}`;
}

/** 146120 -> "146.120 km". */
export function formatKilometres(km: number): string {
  return `${groupThousands(Math.trunc(km).toString())} km`;
}

/**
 * "2026-08-12" -> "12 Agu 2026".
 *
 * A twelve-entry table rather than `Intl`/`toLocaleDateString`: Hermes ships
 * without full ICU on some Android builds, and a date that silently renders in
 * English on one platform is worse than a table nobody has to think about.
 * An unparseable value is returned as-is rather than guessed at.
 */
export function formatShortDate(iso: string): string {
  const [year, month, day] = iso.split("-");
  const name = MONTHS[Number(month) - 1];
  if (!year || !day || !name) return iso;
  return `${Number(day)} ${name} ${year}`;
}
```

- [ ] **Step 4: Write the query hook**

`apps/mobile/src/features/garage/queries.ts`:

```ts
import { useQuery } from "@tanstack/react-query";

import { apiRequest } from "@/shared/api/client";

import type { Vehicle } from "./types";

/** Exported so a later mutation can invalidate exactly this, not everything. */
export const vehiclesQueryKey = ["vehicles"] as const;

/**
 * The account's cars, with each car's service rollup already attached.
 *
 * One request, not two. `GET /vehicles` returns every car's
 * `service_count`, `total_cost`, `last_service_date`, `overdue_count`, and
 * `due_soon_count` (vehicles.rs::list -> service_summary::for_list, two
 * queries for the whole garage rather than two per car). `GET
 * /vehicles/{id}/summary` is a different, richer answer — `cost_last_year`,
 * `odometer_km`, `by_category`, and the reminder list — and nothing on the
 * shell renders any of those. It is the endpoint the Home screen calls the
 * day it grows an "Upcoming Maintenance" block, and not before.
 */
export function useVehicles() {
  return useQuery({
    queryKey: vehiclesQueryKey,
    queryFn: ({ signal }) => apiRequest<Vehicle[]>("/vehicles", { signal }),
  });
}
```

- [ ] **Step 5: Write the active-vehicle resolution**

`apps/mobile/src/features/garage/useActiveVehicle.ts`:

```ts
import { useEffect } from "react";

import { setActiveVehicleId, useActiveVehicleId } from "@/shared/session/store";

import type { Vehicle } from "./types";

/**
 * The car every screen means by "mobil aktif".
 *
 * The stored id is a preference, not an authority. A car can be removed on
 * another device, and a screen that trusted the id alone would then render
 * nothing at all while the person is looking at a garage that has cars in it.
 * So the list decides, and the store is healed to match.
 *
 * The healing happens in an effect rather than during render, because a store
 * write during render is how a component re-renders forever.
 */
export function useActiveVehicle(vehicles: readonly Vehicle[] | undefined): Vehicle | null {
  const storedId = useActiveVehicleId();
  const active = vehicles?.find((vehicle) => vehicle.id === storedId) ?? vehicles?.[0] ?? null;

  useEffect(() => {
    if (active && active.id !== storedId) setActiveVehicleId(active.id);
  }, [active, storedId]);

  return active;
}
```

- [ ] **Step 6: Write the error copy**

`apps/mobile/src/features/shell/errorCopy.ts`:

```ts
import type { ApiError } from "@/shared/api/errors";

/**
 * Plan A's `ApiError` is a plain shape rather than an `Error` subclass, so a
 * consumer that wants its `kind` has to narrow. If `@/shared/api/errors`
 * already exports a guard, DELETE this one and import that — two copies of a
 * narrowing rule is how they drift.
 */
function isApiError(value: unknown): value is ApiError {
  return typeof value === "object" && value !== null && "kind" in value;
}

/**
 * What a person reads under an error title.
 *
 * The spec's taxonomy (§Error taxonomy) maps four kinds to four different
 * things to say, and prefers the server's own message where one exists —
 * the API answers in Bahasa Indonesia by default. "Data kamu aman" is added
 * because §53 says an error state reassures about the data rather than
 * describing the failure.
 */
export function errorBody(error: unknown): string {
  if (!isApiError(error)) return "Ada gangguan. Data kamu aman — coba beberapa saat lagi.";
  if (error.kind === "offline") return "Tidak ada koneksi. Data kamu aman — coba lagi setelah online.";
  return `${error.message} Data kamu aman.`;
}
```

- [ ] **Step 7: Run the gate**

```bash
bun run format
make mb-check
```

Expected: `EXIT=0`, ending in `mobile gate green`. Read the exit code, not the piped output.

### Brief blocks — Task 1

This task writes no UI, so the `frontend-design` line does not apply to it. It applies to every
task after this one.

```
1. Every make target runs from the REPOSITORY ROOT.
2. Mobile gate: `make mb-check` (fmt-check -> expo typed routes -> tsc --noEmit
   -> expo lint). Whole repo: `make check`.
3. Bun, never npm. `bun add --filter` DOES NOT EXIST — use
   `bun add --cwd apps/mobile <pkg>` or `bun x expo install <pkg>`.
   `bun install --frozen-lockfile` must stay EXIT=0 with bun.lock unchanged.
4. Prettier runs from the ROOT only (`bun run format`). Markdown excluded.
5. ** expo-router SDK 56+ VENDORS ITS OWN NAVIGATION. ** Do NOT install
   @react-navigation/native — the router throws "no longer compatible with
   react-navigation". ThemeProvider/DarkTheme/DefaultTheme come FROM
   "expo-router".
6. _layout.tsx already overrides the navigation container background to
   transparent so AmGround shows through. Do not undo it. A tab bar must not
   reintroduce an opaque background that hides the ground.
7. NEVER put a changing `key` on a View wrapping {children} at the app root.
8. apps/mobile has NO test runner and this work does not add one. tsconfig
   strict; `@typescript-eslint/no-explicit-any: error`.
9. The AM-15 design system is complete and MUST be used — apps/mobile/src/app/
   catalog.tsx is the worked example.
10. Never set allowFontScaling={false}.
11. Root .env belongs to the BACKEND; apps/mobile reads only EXPO_PUBLIC_*.
12. CI workflows are path-filtered per app.
```

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

## Task 2: The tab group and the Home tab

**Invoke `frontend-design` first.** The font/palette gate does **not** apply here: this surface has
a committed design system (`docs/design.md`, `packages/tokens`, the AM-15 primitives in
`apps/mobile/src/components`). `impeccable` refines *within* it and does not re-open it. Read
`apps/mobile/src/app/catalog.tsx` in full before writing a component — it is the worked example of
every primitive you will use here.

**Files:**
- Create: `apps/mobile/src/components/shell/TabStack.tsx`
- Create: `apps/mobile/src/components/shell/TabScreen.tsx`
- Create: `apps/mobile/src/components/shell/index.ts`
- Create or modify: `apps/mobile/src/app/(app)/_layout.tsx` — **read it first.** Plan A delivers
  the `(app)` route group and its auth gate. If the file exists, keep the gate exactly as it is
  and replace only the navigator element it renders (`<Slot />` / `<Stack />`) with the `<Tabs>`
  below. If it does not exist, create it with the `<Tabs>` alone; Plan A's gate will wrap it.
- Create: `apps/mobile/src/app/(app)/home/_layout.tsx`
- Create: `apps/mobile/src/app/(app)/home/index.tsx`
- Create: `apps/mobile/src/features/garage/VehicleCard.tsx`
- Modify: `apps/mobile/package.json`, `bun.lock` — one dependency, via `bun x expo install`.
- Test: none (no runner).

**Interfaces:**
- Consumes: Task 1's `useVehicles`, `useActiveVehicle`, `Vehicle`, `formatRupiah`,
  `formatKilometres`, `formatShortDate`, `errorBody`; Plan A's `useSession`,
  `setActiveVehicleId`; `useQueryClient` from `@tanstack/react-query`.
- Produces:
  ```ts
  export function TabStack(): React.JSX.Element;              // components/shell
  export interface TabScreenProps { readonly children: React.ReactNode }
  export function TabScreen(props: TabScreenProps): React.JSX.Element;
  export interface VehicleCardProps { readonly vehicle: Vehicle }
  export function VehicleCard(props: VehicleCardProps): React.JSX.Element;
  // route: /home
  ```

**TDD: no** — verify by running. There is no test runner, and the deliverable is a rendered
surface whose failure modes (an opaque tab bar over the ground, content under the bar, a state that
never appears) are visible and are invisible to a type checker.

**Acceptance criteria:**
- Every tab is a directory with its own `_layout.tsx` rendering `<Stack>`; `popToTopOnBlur` is
  **not set** anywhere (**AC1**).
- The tab bar is `chrome` material, the ground is visible behind it, and no screen's last row sits
  underneath it.
- Home renders four distinct states — loading, error, no-vehicle, and populated — and none of them
  invents a number.
- With two or more cars, the active one can be changed from Home through `AmSelect`, and the
  choice survives leaving and returning to the tab (**AC3**, the switcher half).
- `make mb-check` green; `bun install --frozen-lockfile` still `EXIT=0`.

- [ ] **Step 1: Add the icon dependency**

```bash
bun x expo install @expo/vector-icons
bun install --frozen-lockfile   # must stay EXIT=0 against the updated bun.lock
```

Why a dependency at all, and why this one: `docs/design.md` §17 requires an icon plus an
always-visible label per tab, and §49 requires one consistent outline pack. Nothing installed
provides one — `expo-symbols` is SF Symbols on iOS with an `unstable_`, async path on Android, which
is two code paths in the app's most-used chrome. `@expo/vector-icons` is the Expo-standard,
font-based, zero-native-config answer, and importing the single-family subpath
(`@expo/vector-icons/Ionicons`) bundles only that family's font. Ionicons is the one family whose
outline set covers all five concepts, `car-sport-outline` included; Feather has no car.

- [ ] **Step 2: Write the per-tab stack, once**

`apps/mobile/src/components/shell/TabStack.tsx`:

```tsx
import { Stack } from "expo-router";

/**
 * A tab's own navigator.
 *
 * This is the whole of AM-16 AC1's stack half: because each tab is a
 * DIRECTORY whose `_layout.tsx` renders this, each tab owns a stack that
 * stays mounted while another tab is on screen, and `popToTopOnBlur` defaults
 * to `false`
 * (expo-router/build/react-navigation/bottom-tabs/types.d.ts:200). Nothing in
 * this repository may set it to `true`.
 *
 * `contentStyle` is transparent for the same reason the root layout's is:
 * AmGround is the bottom layer and an opaque screen hides it.
 */
export function TabStack() {
  return (
    <Stack
      screenOptions={{ headerShown: false, contentStyle: { backgroundColor: "transparent" } }}
    />
  );
}
```

- [ ] **Step 3: Write the screen wrapper**

`apps/mobile/src/components/shell/TabScreen.tsx`:

```tsx
import { useBottomTabBarHeight } from "expo-router/js-tabs";
import type { ReactNode } from "react";
import { ScrollView, StyleSheet } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { useTheme } from "@/theme";

export interface TabScreenProps {
  readonly children: ReactNode;
}

/**
 * What every tab's content sits in.
 *
 * The tab bar is absolutely positioned so AmGround shows through it, which
 * takes it out of the layout flow — so every screen owes its own bottom
 * inset, and owing it once here is better than owing it in five screens.
 *
 * `flexGrow: 1` on the content container lets a short screen centre itself
 * with a plain `flex: 1` child, which is what the empty tabs do.
 */
export function TabScreen({ children }: TabScreenProps) {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const tabBarHeight = useBottomTabBarHeight();

  return (
    <ScrollView
      contentContainerStyle={[
        styles.grow,
        {
          padding: theme.pagePadding,
          paddingTop: insets.top + theme.space[4],
          paddingBottom: tabBarHeight + theme.space[6],
          gap: theme.space[5],
        },
      ]}
    >
      {children}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  grow: { flexGrow: 1 },
});
```

`apps/mobile/src/components/shell/index.ts`:

```ts
export { TabScreen } from "./TabScreen";
export type { TabScreenProps } from "./TabScreen";
export { TabStack } from "./TabStack";
```

- [ ] **Step 4: Write the tabs navigator**

`apps/mobile/src/app/(app)/_layout.tsx` — if the file already exists, keep Plan A's gate and
replace only the navigator it renders:

```tsx
import Ionicons from "@expo/vector-icons/Ionicons";
import { Tabs } from "expo-router/js-tabs";
import { StyleSheet } from "react-native";

import { AmMaterial } from "@/components/material";
import { useTheme } from "@/theme";

/**
 * The app shell.
 *
 * `Tabs` is imported from `expo-router/js-tabs`, not from "expo-router": the
 * same export there is deprecated in SDK 57 (expo-router/build/exports.d.ts).
 * `NativeTabs` from `expo-router/unstable-native-tabs` was considered and
 * rejected — it renders the platform's own tab bar, which cannot carry the
 * `chrome` material and would paint over AmGround, and it is `unstable_`.
 *
 * AC1 is structural, not arranged here: every tab is a directory with its own
 * `_layout.tsx` rendering `<Stack>` (see components/shell/TabStack.tsx), and
 * `popToTopOnBlur` is left at its default of `false`.
 *
 * Tab ORDER is the order of these children —
 * expo-router/build/useScreens.js:63 uses the declared order when there is
 * one. Screens are added to this list by the tasks that create them.
 */
export const unstable_settings = { anchor: "home" };

export default function AppLayout() {
  const theme = useTheme();

  return (
    <Tabs
      screenOptions={{
        headerShown: false,
        // The ground is the app's bottom layer; a scene with a fill hides it.
        sceneStyle: { backgroundColor: "transparent" },
        // §17: active icon and label are the brand accent. `accentText` rather
        // than `accent` because this one value colours the LABEL too, and raw
        // #ED491C is 3.77:1 as text on white.
        tabBarActiveTintColor: theme.color.accentText,
        tabBarInactiveTintColor: theme.color.textSecondary,
        // §17 again: labels are always visible, never icon-only.
        tabBarLabelStyle: theme.type.micro,
        tabBarStyle: styles.bar,
        tabBarBackground: () => (
          // `{null}` children: AmMaterial requires the prop, and a tab-bar
          // background is a fill with nothing inside it.
          <AmMaterial role="chrome" radius="xs" style={StyleSheet.absoluteFill}>
            {null}
          </AmMaterial>
        ),
      }}
    >
      <Tabs.Screen
        name="home"
        options={{
          title: "Beranda",
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="home-outline" color={color} size={size} />
          ),
        }}
      />
    </Tabs>
  );
}

const styles = StyleSheet.create({
  // Absolute so the ground shows through the bar. The edge comes from
  // AmMaterial, so the navigator's own hairline and elevation are removed
  // rather than drawn on top of it.
  bar: {
    position: "absolute",
    backgroundColor: "transparent",
    borderTopWidth: 0,
    elevation: 0,
  },
});
```

- [ ] **Step 5: Write the Home stack**

`apps/mobile/src/app/(app)/home/_layout.tsx`:

```tsx
import { TabStack } from "@/components/shell";

export default function HomeLayout() {
  return <TabStack />;
}
```

- [ ] **Step 6: Write the vehicle card**

`apps/mobile/src/features/garage/VehicleCard.tsx`:

```tsx
import { StyleSheet, Text, View } from "react-native";

import { AmBadge, AmCard } from "@/components/display";
import { numeric, useTheme } from "@/theme";

import { formatKilometres, formatRupiah, formatShortDate } from "./format";
import type { Vehicle } from "./types";

export interface VehicleCardProps {
  readonly vehicle: Vehicle;
}

interface RowProps {
  readonly label: string;
  readonly value: string;
}

function Row({ label, value }: RowProps) {
  const theme = useTheme();
  return (
    <View style={styles.row}>
      <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>{label}</Text>
      <Text style={[theme.type.body, numeric, { color: theme.color.textPrimary }]}>{value}</Text>
    </View>
  );
}

/**
 * The active car, and what its service history adds up to.
 *
 * `working`, not `surface`: §77 puts anything read to make a decision on the
 * solid material, and a service cost read outdoors at a workshop is the case
 * that rule exists for.
 *
 * Every number here comes from the server. A car with no history says so in
 * words rather than showing a row of zeroes, and a null cost is omitted
 * rather than rendered as "Rp 0" — a zero the person did not spend is
 * invented data.
 */
export function VehicleCard({ vehicle }: VehicleCardProps) {
  const theme = useTheme();
  const summary = vehicle.summary;
  const meta = [
    vehicle.year?.toString(),
    vehicle.colour,
    vehicle.mileage_km === null ? null : formatKilometres(vehicle.mileage_km),
  ].filter((part): part is string => Boolean(part));

  return (
    <AmCard role="working">
      <View style={{ gap: theme.space[3] }}>
        <View style={{ gap: theme.space[1] }}>
          <Text
            accessibilityRole="header"
            style={[theme.type.h2, { color: theme.color.textPrimary }]}
          >
            {vehicle.name}
          </Text>
          {meta.length > 0 ? (
            <Text style={[theme.type.caption, numeric, { color: theme.color.textSecondary }]}>
              {meta.join(" · ")}
            </Text>
          ) : null}
        </View>

        {summary && summary.service_count > 0 ? (
          <View style={{ gap: theme.space[2] }}>
            <Row label="Servis tercatat" value={summary.service_count.toString()} />
            {summary.total_cost ? (
              <Row label="Total biaya" value={formatRupiah(summary.total_cost)} />
            ) : null}
            {summary.last_service_date ? (
              <Row label="Servis terakhir" value={formatShortDate(summary.last_service_date)} />
            ) : null}
          </View>
        ) : (
          <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
            Belum ada riwayat servis untuk mobil ini.
          </Text>
        )}

        {summary && (summary.overdue_count > 0 || summary.due_soon_count > 0) ? (
          <View style={[styles.wrap, { gap: theme.space[2] }]}>
            {summary.overdue_count > 0 ? (
              <AmBadge
                tone="danger"
                icon="!"
                label={`${summary.overdue_count} servis terlambat`}
              />
            ) : null}
            {summary.due_soon_count > 0 ? (
              <AmBadge tone="warning" icon="•" label={`${summary.due_soon_count} servis segera`} />
            ) : null}
          </View>
        ) : null}
      </View>
    </AmCard>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", justifyContent: "space-between" },
  wrap: { flexDirection: "row", flexWrap: "wrap" },
});
```

- [ ] **Step 7: Write the Home screen**

`apps/mobile/src/app/(app)/home/index.tsx`:

```tsx
import { useQueryClient } from "@tanstack/react-query";
import { router } from "expo-router";
import { Text, View } from "react-native";

import { AmCard } from "@/components/display";
import { AmSelect } from "@/components/input";
import { TabScreen } from "@/components/shell";
import { AmEmptyState, AmErrorState, AmSkeleton } from "@/components/state";
import { useVehicles } from "@/features/garage/queries";
import { useActiveVehicle } from "@/features/garage/useActiveVehicle";
import { VehicleCard } from "@/features/garage/VehicleCard";
import { errorBody } from "@/features/shell/errorCopy";
import { setActiveVehicleId, useSession } from "@/shared/session/store";
import { useTheme } from "@/theme";

/**
 * §19: Home is not an infinite feed. Header, the selected vehicle, and what
 * its history says — and nothing this release cannot answer honestly.
 */
export default function HomeScreen() {
  const theme = useTheme();
  const { user } = useSession();
  const queryClient = useQueryClient();
  const vehicles = useVehicles();
  const active = useActiveVehicle(vehicles.data);

  const name = user?.displayName ?? user?.username;
  const options = (vehicles.data ?? []).map((vehicle) => ({
    value: vehicle.id,
    label: vehicle.name,
  }));

  // The shell is only reachable when GET /me says the account has a car, so an
  // empty list means the last one went away somewhere else. Re-running the
  // bootstrap gate is what puts the person back into the first-car wizard;
  // this screen must not grow its own copy of that route.
  const restartOnboarding = () => {
    void queryClient.invalidateQueries();
    router.replace("/");
  };

  return (
    <TabScreen>
      <Text accessibilityRole="header" style={[theme.type.h1, { color: theme.color.textPrimary }]}>
        {name ? `Halo, ${name}` : "Beranda"}
      </Text>

      {vehicles.isPending ? (
        <AmCard role="working">
          <View style={{ gap: theme.space[3] }}>
            <AmSkeleton height={28} width="70%" />
            <AmSkeleton height={14} width="45%" />
            <AmSkeleton height={14} />
            <AmSkeleton height={14} width="80%" />
          </View>
        </AmCard>
      ) : null}

      {vehicles.isError ? (
        <AmCard role="working">
          <AmErrorState
            title="Garasi gagal dimuat"
            body={errorBody(vehicles.error)}
            onRetry={() => void vehicles.refetch()}
          />
        </AmCard>
      ) : null}

      {vehicles.isSuccess && !active ? (
        <AmCard role="working">
          <AmEmptyState
            title="Belum ada mobil di garasi"
            body="Semua isi aplikasi ini berangkat dari mobil kamu. Tambahkan satu dulu, sisanya menyusul."
            actionLabel="Tambah mobil"
            onAction={restartOnboarding}
          />
        </AmCard>
      ) : null}

      {active ? <VehicleCard vehicle={active} /> : null}

      {/* One car has nothing to switch to. §61: a control that cannot do
          anything is worse than no control. */}
      {options.length > 1 ? (
        <AmSelect
          label="Mobil aktif"
          value={active?.id ?? null}
          options={options}
          onChange={setActiveVehicleId}
        />
      ) : null}
    </TabScreen>
  );
}
```

- [ ] **Step 8: Run the gate**

```bash
bun run format
make mb-check
```

Expected: `EXIT=0`. If `tsc` rejects `router.replace("/")`, the typed-routes manifest is stale —
`make mb-check` regenerates it as its second step, so re-run it once before investigating.

- [ ] **Step 9: Visual verification — mandatory, all four states, both themes**

Run the app (`make dev`, which opens the iOS simulator; the dev client must have been built once
with `make mb-run-dev p=ios`). Toggle the theme from outside the app so no debug control has to
exist inside it:

```bash
xcrun simctl ui booted appearance dark
xcrun simctl ui booted appearance light
```

Open and screenshot each of these, **in both themes**:

| State | How to reach it |
|---|---|
| Loading | Cold-start the app with the API running; screenshot before the response lands. If it is too fast, stop the API, start the app, then start the API. |
| Error (offline) | Turn off the Mac's network, or stop the API (`ctrl-c` on `make dev`'s api), pull to refresh. Expect "Tidak ada koneksi", never a raw error string. |
| No vehicle | An account whose garage is empty. Expect the empty state and its one action — no zeroes, no fabricated card. |
| Populated | An account with a car that has service history. Check the rupiah grouping (`Rp 4.200.000`), the date (`12 Agu 2026`), the kilometres (`146.120 km`), and that overdue/due-soon badges appear only when the counts are above zero. |

Then check the shell itself:

- The graphite ground is visible **through** the tab bar — the bar is glass on iOS 26+, and a solid
  `chrome` fill elsewhere, which is the contract rather than a fallback.
- The last row of a long Home screen scrolls clear of the tab bar; nothing sits underneath it.
- The tab label and icon are orange when active and secondary-grey when not.
- Increase the system font size (Settings → Accessibility → Display & Text Size → Larger Text) and
  confirm nothing clips — the app never sets `allowFontScaling={false}`.

**AC1, scroll half** — this is the check, written as steps a person performs. It needs a second tab,
so run it again at the end of Task 3 and record the result there:

1. Open Beranda and scroll to the bottom.
2. Switch to another tab.
3. Switch back to Beranda.
4. The scroll position is where you left it, not the top.

### Brief blocks — Task 2

```
1. Every make target runs from the REPOSITORY ROOT.
2. Mobile gate: `make mb-check` (fmt-check -> expo typed routes -> tsc --noEmit
   -> expo lint). Whole repo: `make check`.
3. Bun, never npm. `bun add --filter` DOES NOT EXIST — use
   `bun add --cwd apps/mobile <pkg>` or `bun x expo install <pkg>`.
   `bun install --frozen-lockfile` must stay EXIT=0 with bun.lock unchanged.
4. Prettier runs from the ROOT only (`bun run format`). Markdown excluded.
5. ** expo-router SDK 56+ VENDORS ITS OWN NAVIGATION. ** Do NOT install
   @react-navigation/native — the router throws "no longer compatible with
   react-navigation". ThemeProvider/DarkTheme/DefaultTheme come FROM
   "expo-router".
6. _layout.tsx already overrides the navigation container background to
   transparent so AmGround shows through. Do not undo it. A tab bar must not
   reintroduce an opaque background that hides the ground.
7. NEVER put a changing `key` on a View wrapping {children} at the app root.
8. apps/mobile has NO test runner and this work does not add one. tsconfig
   strict; `@typescript-eslint/no-explicit-any: error`.
9. The AM-15 design system is complete and MUST be used — apps/mobile/src/app/
   catalog.tsx is the worked example.
10. Never set allowFontScaling={false}.
11. Root .env belongs to the BACKEND; apps/mobile reads only EXPO_PUBLIC_*.
12. CI workflows are path-filtered per app.
```

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

## Task 3: Garasi, Jelajah, and Komunitas — three honest empty tabs

**Invoke `frontend-design` first.** The font/palette gate does **not** apply here: this surface has
a committed design system (`docs/design.md`, `packages/tokens`, the AM-15 primitives).
`impeccable` refines within it and does not re-open it.

The copy is the deliverable. Each of these says what will live there, why it is empty, and offers
the one thing a person can actually do today. `AmEmptyState` requires exactly one action — a
product rule, not an oversight (`AmEmptyState.tsx`, "the platform launches with no data at all") —
and for a tab whose feature does not exist, the only honest action is one that leads somewhere real
rather than deeper into the same absence. That is Beranda, the one tab with content.

**Files:**
- Create: `apps/mobile/src/app/(app)/garage/_layout.tsx`, `.../garage/index.tsx`
- Create: `apps/mobile/src/app/(app)/explore/_layout.tsx`, `.../explore/index.tsx`
- Create: `apps/mobile/src/app/(app)/community/_layout.tsx`, `.../community/index.tsx`
- Modify: `apps/mobile/src/app/(app)/_layout.tsx` — add three `<Tabs.Screen>` after `home`
- Test: none (no runner).

**Interfaces:**
- Consumes: `TabScreen`, `TabStack` (Task 2); `router` from `expo-router`.
- Produces: routes `/garage`, `/explore`, `/community`.

**TDD: no** — verify by running. Three screens of copy and one navigator edit; there is no runner
and nothing here has a contract a test could hold.

**Acceptance criteria:** four tabs in the declared order Beranda · Garasi · Jelajah · Komunitas;
each of the three renders a centred empty state whose action lands on Beranda; no tab invents a
count, a member, or a build; `make mb-check` green.

- [ ] **Step 1: The three stacks**

`apps/mobile/src/app/(app)/garage/_layout.tsx`:

```tsx
import { TabStack } from "@/components/shell";

export default function GarageLayout() {
  return <TabStack />;
}
```

`apps/mobile/src/app/(app)/explore/_layout.tsx`:

```tsx
import { TabStack } from "@/components/shell";

export default function ExploreLayout() {
  return <TabStack />;
}
```

`apps/mobile/src/app/(app)/community/_layout.tsx`:

```tsx
import { TabStack } from "@/components/shell";

export default function CommunityLayout() {
  return <TabStack />;
}
```

- [ ] **Step 2: Garasi**

`apps/mobile/src/app/(app)/garage/index.tsx`:

```tsx
import { router } from "expo-router";
import { StyleSheet, Text, View } from "react-native";

import { AmCard } from "@/components/display";
import { TabScreen } from "@/components/shell";
import { AmEmptyState } from "@/components/state";
import { useTheme } from "@/theme";

/**
 * AM-16's out-of-scope line is "isi setiap tab", so the garage screen itself
 * belongs to the garage epic. What ships here is the honest version of an
 * empty room: what it will hold, and where the thing you came for is today.
 */
export default function GarageScreen() {
  const theme = useTheme();
  return (
    <TabScreen>
      <Text accessibilityRole="header" style={[theme.type.h1, { color: theme.color.textPrimary }]}>
        Garasi
      </Text>
      <View style={styles.centre}>
        <AmCard role="working">
          <AmEmptyState
            title="Garasi lengkap belum dibuka"
            body="Nanti di sini ada semua mobil kamu — foto, modifikasi, dan riwayat servis lengkapnya. Untuk sekarang, mobil aktif kamu ada di Beranda."
            actionLabel="Buka Beranda"
            onAction={() => router.navigate("/home")}
          />
        </AmCard>
      </View>
    </TabScreen>
  );
}

const styles = StyleSheet.create({
  centre: { flex: 1, justifyContent: "center" },
});
```

- [ ] **Step 3: Jelajah**

`apps/mobile/src/app/(app)/explore/index.tsx`:

```tsx
import { router } from "expo-router";
import { StyleSheet, Text, View } from "react-native";

import { AmCard } from "@/components/display";
import { TabScreen } from "@/components/shell";
import { AmEmptyState } from "@/components/state";
import { useTheme } from "@/theme";

/**
 * Explore is community output — builds, parts, solutions from cars like
 * yours. There is no output yet because there are no garages yet, and the
 * spec's rule is that the platform launches empty and says so rather than
 * showing a wall of invented content.
 */
export default function ExploreScreen() {
  const theme = useTheme();
  return (
    <TabScreen>
      <Text accessibilityRole="header" style={[theme.type.h1, { color: theme.color.textPrimary }]}>
        Jelajah
      </Text>
      <View style={styles.centre}>
        <AmCard role="working">
          <AmEmptyState
            title="Jelajah belum ada isinya"
            body="Nanti di sini kamu bisa menemukan modifikasi, part, dan solusi dari mobil yang sama dengan punyamu. Isinya datang dari garasi anggota — belum ada satu pun yang terisi."
            actionLabel="Lihat mobil kamu"
            onAction={() => router.navigate("/home")}
          />
        </AmCard>
      </View>
    </TabScreen>
  );
}

const styles = StyleSheet.create({
  centre: { flex: 1, justifyContent: "center" },
});
```

- [ ] **Step 4: Komunitas**

`apps/mobile/src/app/(app)/community/index.tsx`:

```tsx
import { router } from "expo-router";
import { StyleSheet, Text, View } from "react-native";

import { AmCard } from "@/components/display";
import { TabScreen } from "@/components/shell";
import { AmEmptyState } from "@/components/state";
import { useTheme } from "@/theme";

/**
 * The community epic has no implementation, so this says what will be here
 * and points at the one thing a person can do today. No member count, no
 * sample club, no "1.2rb anggota" — a fabricated community is the exact
 * thing the project's own rule forbids seeding.
 */
export default function CommunityScreen() {
  const theme = useTheme();
  return (
    <TabScreen>
      <Text accessibilityRole="header" style={[theme.type.h1, { color: theme.color.textPrimary }]}>
        Komunitas
      </Text>
      <View style={styles.centre}>
        <AmCard role="working">
          <AmEmptyState
            title="Komunitas belum dimulai"
            body="Nanti di sini ada klub, diskusi, dan tanya-jawab sesama pemilik mobil. Yang pertama mengisi garasinya jadi yang pertama punya sesuatu untuk dibagikan."
            actionLabel="Lihat mobil kamu"
            onAction={() => router.navigate("/home")}
          />
        </AmCard>
      </View>
    </TabScreen>
  );
}

const styles = StyleSheet.create({
  centre: { flex: 1, justifyContent: "center" },
});
```

- [ ] **Step 5: Declare the three tabs, in order, after `home`**

In `apps/mobile/src/app/(app)/_layout.tsx`, immediately after the `home` screen:

```tsx
      <Tabs.Screen
        name="garage"
        options={{
          title: "Garasi",
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="car-sport-outline" color={color} size={size} />
          ),
        }}
      />
      <Tabs.Screen
        name="explore"
        options={{
          title: "Jelajah",
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="compass-outline" color={color} size={size} />
          ),
        }}
      />
      <Tabs.Screen
        name="community"
        options={{
          title: "Komunitas",
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="people-outline" color={color} size={size} />
          ),
        }}
      />
```

- [ ] **Step 6: Run the gate**

```bash
bun run format
make mb-check
```

Expected: `EXIT=0`.

- [ ] **Step 7: Visual verification, and the AC1 evidence**

Screenshot each of the three tabs in **both themes** (`xcrun simctl ui booted appearance dark|light`).
Check that the empty state is vertically centred, that the tab bar does not cover the action
button, and that the four labels read Beranda · Garasi · Jelajah · Komunitas in that order.

**AC1, both halves, performed in this order and recorded in `## Execution status`:**

1. Open Beranda and scroll to the bottom of the screen.
2. Tap Garasi. Tap Jelajah. Tap Komunitas.
3. Tap Beranda. **The scroll position is where you left it** — not the top.
4. Tap Garasi again. **Its screen is the one you left**, not a fresh mount (the empty state does
   not re-animate).
5. Background the app (Home button), reopen it. The tab you were on is still the tab you are on.

**Say plainly what step 4 does not prove.** No tab has a second screen in Plan C, so the *stack*
half of AC1 is verified structurally — each tab has its own `Stack` and `popToTopOnBlur` is left at
its default `false` — rather than observed. The first tab that gains a pushed screen re-runs this
check. Put that sentence in the AM-16 comment rather than letting "AC1 ✓" imply more than was seen.

### Brief blocks — Task 3

```
1. Every make target runs from the REPOSITORY ROOT.
2. Mobile gate: `make mb-check` (fmt-check -> expo typed routes -> tsc --noEmit
   -> expo lint). Whole repo: `make check`.
3. Bun, never npm. `bun add --filter` DOES NOT EXIST — use
   `bun add --cwd apps/mobile <pkg>` or `bun x expo install <pkg>`.
   `bun install --frozen-lockfile` must stay EXIT=0 with bun.lock unchanged.
4. Prettier runs from the ROOT only (`bun run format`). Markdown excluded.
5. ** expo-router SDK 56+ VENDORS ITS OWN NAVIGATION. ** Do NOT install
   @react-navigation/native — the router throws "no longer compatible with
   react-navigation". ThemeProvider/DarkTheme/DefaultTheme come FROM
   "expo-router".
6. _layout.tsx already overrides the navigation container background to
   transparent so AmGround shows through. Do not undo it. A tab bar must not
   reintroduce an opaque background that hides the ground.
7. NEVER put a changing `key` on a View wrapping {children} at the app root.
8. apps/mobile has NO test runner and this work does not add one. tsconfig
   strict; `@typescript-eslint/no-explicit-any: error`.
9. The AM-15 design system is complete and MUST be used — apps/mobile/src/app/
   catalog.tsx is the worked example.
10. Never set allowFontScaling={false}.
11. Root .env belongs to the BACKEND; apps/mobile reads only EXPO_PUBLIC_*.
12. CI workflows are path-filtered per app.
```

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

## Task 4: The Profile tab — identity and sign-out

**Invoke `frontend-design` first.** The font/palette gate does **not** apply here: this surface has
a committed design system (`docs/design.md`, `packages/tokens`, the AM-15 primitives).
`impeccable` refines within it and does not re-open it.

**AM-51 AC4 — "session actually ends" — is verified on this screen even though the ticket belongs
to Plan B.** Plan B builds the login screen; nothing in Plan B can sign out, because there is no
signed-in surface to do it from until this tab exists. Say so in both tickets rather than letting
AM-51 close on a criterion nobody exercised.

**Files:**
- Create: `apps/mobile/src/app/(app)/profile/_layout.tsx`
- Create: `apps/mobile/src/app/(app)/profile/index.tsx`
- Modify: `apps/mobile/src/app/(app)/_layout.tsx` — add the fifth `<Tabs.Screen>`, last
- Test: none (no runner).

**Interfaces:**
- Consumes: `useSession` (`@/shared/session/store`), `signOut` (`@/shared/session/signOut`),
  `TabScreen`, `TabStack`, `AmAvatar`, `AmBottomSheet`, `AmButton`, `AmCard`, `AmSkeleton`.
- Produces: route `/profile`.

**TDD: no** — verify by running. The one branch worth testing is `signOut`'s epoch transaction,
and that is Plan A's code with Plan A's verdict.

**Acceptance criteria:** the tab shows the signed-in person's display name, username, and email
from `GET /me` (via `useSession`, no second request); sign-out asks for confirmation in an
`AmBottomSheet`, never a native `Alert`; confirming calls Plan A's `signOut()` exactly once and
the app lands on the signed-out surface; force-quitting and reopening after sign-out does **not**
return to the shell; `make mb-check` green.

- [ ] **Step 1: The stack**

`apps/mobile/src/app/(app)/profile/_layout.tsx`:

```tsx
import { TabStack } from "@/components/shell";

export default function ProfileLayout() {
  return <TabStack />;
}
```

- [ ] **Step 2: The screen**

`apps/mobile/src/app/(app)/profile/index.tsx`:

```tsx
import { useState } from "react";
import { Text, View } from "react-native";

import { AmAvatar, AmBottomSheet, AmCard } from "@/components/display";
import { AmButton } from "@/components/input";
import { TabScreen } from "@/components/shell";
import { AmSkeleton } from "@/components/state";
import { signOut } from "@/shared/session/signOut";
import { useSession } from "@/shared/session/store";
import { useTheme } from "@/theme";

/**
 * Who you are, and the way out.
 *
 * The identity comes from the session that the bootstrap gate already
 * fetched — one `GET /me` per launch, not one per screen that wants a name.
 *
 * Signing out is confirmed in a bottom sheet rather than a native dialog:
 * §45 and AM-27 make AmBottomSheet the pattern for every sheet, picker, and
 * confirmation in this app.
 */
export default function ProfileScreen() {
  const theme = useTheme();
  const { user } = useSession();
  const [confirming, setConfirming] = useState(false);
  const [leaving, setLeaving] = useState(false);

  // The gate means a signed-out person never reaches this screen, so a null
  // user is the moment before the session resolves — a skeleton, not an error.
  if (!user) {
    return (
      <TabScreen>
        <AmCard role="working">
          <View style={{ gap: theme.space[3] }}>
            <AmSkeleton height={56} width={56} radius="pill" />
            <AmSkeleton height={24} width="60%" />
            <AmSkeleton height={14} width="40%" />
          </View>
        </AmCard>
      </TabScreen>
    );
  }

  const displayName = user.displayName ?? user.username ?? user.email;
  const handle = user.username ? `@${user.username}` : null;

  return (
    <TabScreen>
      <Text accessibilityRole="header" style={[theme.type.h1, { color: theme.color.textPrimary }]}>
        Profil
      </Text>

      <AmCard role="working">
        <View style={{ gap: theme.space[3], alignItems: "center" }}>
          <AmAvatar name={displayName} size={72} />
          <View style={{ gap: theme.space[1], alignItems: "center" }}>
            <Text style={[theme.type.h2, { color: theme.color.textPrimary }]}>{displayName}</Text>
            {handle ? (
              <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>{handle}</Text>
            ) : null}
            <Text style={[theme.type.caption, { color: theme.color.textTertiary }]}>
              {user.email}
            </Text>
          </View>
        </View>
      </AmCard>

      <AmButton label="Keluar" variant="destructive" onPress={() => setConfirming(true)} />

      <AmBottomSheet
        visible={confirming}
        onClose={() => setConfirming(false)}
        title="Keluar dari akun?"
      >
        <View style={{ gap: theme.space[3] }}>
          <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
            Data garasi kamu tetap tersimpan. Kamu perlu masuk lagi dengan email dan kata sandi
            untuk membukanya.
          </Text>
          <AmButton
            label="Keluar"
            variant="destructive"
            loading={leaving}
            onPress={() => {
              // `loading` also blocks the second tap: sign-out is one
              // transaction with one redirect (spec, §The session contract),
              // and two of them racing is the defect that contract exists to
              // prevent.
              setLeaving(true);
              void signOut();
            }}
          />
          <AmButton label="Batal" variant="secondary" onPress={() => setConfirming(false)} />
        </View>
      </AmBottomSheet>
    </TabScreen>
  );
}
```

- [ ] **Step 3: Declare the fifth tab, last**

In `apps/mobile/src/app/(app)/_layout.tsx`, after `community`:

```tsx
      <Tabs.Screen
        name="profile"
        options={{
          title: "Profil",
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="person-outline" color={color} size={size} />
          ),
        }}
      />
```

- [ ] **Step 4: Run the gate**

```bash
bun run format
make mb-check
```

Expected: `EXIT=0`.

- [ ] **Step 5: Visual verification, and the AM-51 AC4 check**

Screenshot in **both themes**: the profile card, the confirmation sheet open, and the sheet's
loading state on the destructive button.

- An account **with** a username and display name, and one **without** either — the second must
  fall back to the email without leaving an empty line or a stray `@`.
- The sheet closes on the scrim tap and on the drag-down, both of which `AmBottomSheet` provides.
- Both sheet buttons clear 44 pt.

Then AM-51 AC4, as steps:

1. Sign in, reach Profil, tap **Keluar**, confirm.
2. The app lands on the signed-out surface, once — not twice, and not on a flash of the shell.
3. Force-quit the app and reopen it. It asks for a password; it does **not** return to the shell.
4. Sign in again with the same account. The garage that appears is that account's — no data from
   the previous session survives (the per-account cache key, spec §Storage split).

### Brief blocks — Task 4

```
1. Every make target runs from the REPOSITORY ROOT.
2. Mobile gate: `make mb-check` (fmt-check -> expo typed routes -> tsc --noEmit
   -> expo lint). Whole repo: `make check`.
3. Bun, never npm. `bun add --filter` DOES NOT EXIST — use
   `bun add --cwd apps/mobile <pkg>` or `bun x expo install <pkg>`.
   `bun install --frozen-lockfile` must stay EXIT=0 with bun.lock unchanged.
4. Prettier runs from the ROOT only (`bun run format`). Markdown excluded.
5. ** expo-router SDK 56+ VENDORS ITS OWN NAVIGATION. ** Do NOT install
   @react-navigation/native — the router throws "no longer compatible with
   react-navigation". ThemeProvider/DarkTheme/DefaultTheme come FROM
   "expo-router".
6. _layout.tsx already overrides the navigation container background to
   transparent so AmGround shows through. Do not undo it. A tab bar must not
   reintroduce an opaque background that hides the ground.
7. NEVER put a changing `key` on a View wrapping {children} at the app root.
8. apps/mobile has NO test runner and this work does not add one. tsconfig
   strict; `@typescript-eslint/no-explicit-any: error`.
9. The AM-15 design system is complete and MUST be used — apps/mobile/src/app/
   catalog.tsx is the worked example.
10. Never set allowFontScaling={false}.
11. Root .env belongs to the BACKEND; apps/mobile reads only EXPO_PUBLIC_*.
12. CI workflows are path-filtered per app.
```

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

## Task 5: The global add action

**Invoke `frontend-design` first.** The font/palette gate does **not** apply here: this surface has
a committed design system (`docs/design.md`, `packages/tokens`, the AM-15 primitives).
`impeccable` refines within it and does not re-open it.

**Read this before writing anything.** AM-16 AC2 names four things a person can add — modifikasi,
servis, problem, foto — and **not one of their forms exists**; each belongs to a different epic,
and this plan does not build them. The spec's rule is that an entry whose form is missing is
**absent**, not present-and-broken. Applied honestly that empties the sheet, and a "+" that opens
onto nothing is exactly the dead end `AmEmptyState`'s design note forbids. So the registry is the
feature: one array decides what the sheet lists **and** whether the button exists at all. Today the
array is empty and the button renders nothing. Adding an entry is a one-line change with no other
edit anywhere.

**Files:**
- Create: `apps/mobile/src/features/shell/addActions.ts`
- Create: `apps/mobile/src/components/shell/AddButton.tsx`
- Modify: `apps/mobile/src/components/shell/TabScreen.tsx` — host the button
- Modify: `apps/mobile/src/components/shell/index.ts` — export it
- Test: none (no runner).

**Interfaces:**
- Consumes: Task 1's `useVehicles`, `useActiveVehicle`; Plan A's `setActiveVehicleId`;
  `AmBottomSheet`, `AmButton`, `AmSelect`; `useBottomTabBarHeight` from `expo-router/js-tabs`;
  `Href` and `router` from `expo-router`.
- Produces:
  ```ts
  export interface AddAction { readonly key: string; readonly label: string; readonly href: Href }
  export const ADD_ACTIONS: readonly AddAction[];   // empty today
  export function AddButton(): React.JSX.Element | null;
  ```

**TDD: no** — verify by running. The one rule worth a test (`ADD_ACTIONS` empty ⇒ no button) is a
render assertion, and there is no runner to hold it.

**Acceptance criteria:**
- With `ADD_ACTIONS` empty, **nothing renders** — no button, and no `GET /vehicles` fired from
  tabs that do not otherwise need it (**AC2**, honestly).
- With one entry present, the button appears on every tab, opens `AmBottomSheet`, and the sheet's
  vehicle is the active one, changeable through `AmSelect`, and the change survives closing the
  sheet (**AC3**).
- No entry ever renders disabled, greyed, or "coming soon".
- `make mb-check` green with the temporary entry **removed**.

- [ ] **Step 1: Write the registry**

`apps/mobile/src/features/shell/addActions.ts`:

```ts
import type { Href } from "expo-router";

export interface AddAction {
  readonly key: string;
  readonly label: string;
  readonly href: Href;
}

/**
 * What "+" can add today.
 *
 * AM-16 AC2 names four — modifikasi, servis, problem, foto — and none of
 * their forms exist; each belongs to its own epic. The spec's rule is that an
 * entry whose destination is missing is ABSENT rather than present and
 * broken, so this array is empty, and AddButton renders nothing while it is.
 * A "+" that opens onto an empty sheet is a dead end, which is the one thing
 * the empty-state rules in this codebase exist to prevent.
 *
 * Adding an entry here is the entire integration: the sheet's contents, the
 * pre-filled vehicle, and the button's existence all read from this array.
 *
 *   modifikasi  build epic
 *   servis      service-history epic
 *   problem     known-issues epic
 *   foto        vehicle-photos epic
 *
 * The explicit type annotation matters: without it the empty literal infers
 * `never[]` and the first entry added fails to type-check for the wrong
 * reason.
 */
export const ADD_ACTIONS: readonly AddAction[] = [];
```

- [ ] **Step 2: Write the button**

`apps/mobile/src/components/shell/AddButton.tsx`:

```tsx
import { router } from "expo-router";
import { useBottomTabBarHeight } from "expo-router/js-tabs";
import { useState } from "react";
import { StyleSheet, View } from "react-native";

import { AmBottomSheet } from "@/components/display";
import { AmButton, AmSelect } from "@/components/input";
import { useVehicles } from "@/features/garage/queries";
import { useActiveVehicle } from "@/features/garage/useActiveVehicle";
import { ADD_ACTIONS } from "@/features/shell/addActions";
import { setActiveVehicleId } from "@/shared/session/store";
import { useTheme } from "@/theme";

/**
 * The global add action (AM-16 AC2, AC3).
 *
 * Two components rather than one early return inside hooks: `ADD_ACTIONS` is
 * a module constant, so this outer component decides once whether the feature
 * exists at all, and the inner one — which holds every hook, including the
 * vehicles query — is never mounted while the registry is empty. Written as a
 * single component, the empty case would still fire a query on every tab.
 */
export function AddButton() {
  if (ADD_ACTIONS.length === 0) return null;
  return <AddButtonContent />;
}

function AddButtonContent() {
  const theme = useTheme();
  const tabBarHeight = useBottomTabBarHeight();
  const vehicles = useVehicles();
  const active = useActiveVehicle(vehicles.data);
  const [open, setOpen] = useState(false);

  const options = (vehicles.data ?? []).map((vehicle) => ({
    value: vehicle.id,
    label: vehicle.name,
  }));

  return (
    <>
      <View
        style={[
          styles.float,
          { right: theme.pagePadding, bottom: tabBarHeight + theme.space[4] },
        ]}
      >
        <AmButton label="Tambah" variant="accent" onPress={() => setOpen(true)} />
      </View>

      <AmBottomSheet
        visible={open}
        onClose={() => setOpen(false)}
        // AC3: the sheet says which car it is adding to before it says what.
        title={active ? `Tambah ke ${active.name}` : "Tambah"}
      >
        <View style={{ gap: theme.space[3] }}>
          {options.length > 1 ? (
            <AmSelect
              label="Mobil"
              value={active?.id ?? null}
              options={options}
              onChange={setActiveVehicleId}
            />
          ) : null}
          {ADD_ACTIONS.map((action) => (
            <AmButton
              key={action.key}
              label={action.label}
              variant="secondary"
              onPress={() => {
                setOpen(false);
                router.navigate(action.href);
              }}
            />
          ))}
        </View>
      </AmBottomSheet>
    </>
  );
}

const styles = StyleSheet.create({
  float: { position: "absolute" },
});
```

- [ ] **Step 3: Host it on every tab**

`apps/mobile/src/components/shell/TabScreen.tsx` in full, replacing Task 2's version — the button
floats over the scroll view rather than scrolling with it:

```tsx
import { useBottomTabBarHeight } from "expo-router/js-tabs";
import type { ReactNode } from "react";
import { ScrollView, StyleSheet, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { useTheme } from "@/theme";

import { AddButton } from "./AddButton";

export interface TabScreenProps {
  readonly children: ReactNode;
}

/**
 * What every tab's content sits in.
 *
 * The tab bar is absolutely positioned so AmGround shows through it, which
 * takes it out of the layout flow — so every screen owes its own bottom
 * inset, and owing it once here is better than owing it in five screens.
 *
 * `flexGrow: 1` on the content container lets a short screen centre itself
 * with a plain `flex: 1` child, which is what the empty tabs do.
 */
export function TabScreen({ children }: TabScreenProps) {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const tabBarHeight = useBottomTabBarHeight();

  const padding = {
    padding: theme.pagePadding,
    paddingTop: insets.top + theme.space[4],
    paddingBottom: tabBarHeight + theme.space[6],
    gap: theme.space[5],
  };

  return (
    <View style={styles.fill}>
      <ScrollView contentContainerStyle={[styles.grow, padding]}>{children}</ScrollView>
      {/* The add action belongs to the shell, not to a screen — every tab
          gets it, and it renders nothing while no form exists to add to. */}
      <AddButton />
    </View>
  );
}

const styles = StyleSheet.create({
  fill: { flex: 1 },
  grow: { flexGrow: 1 },
});
```

`apps/mobile/src/components/shell/index.ts` in full:

```ts
export { AddButton } from "./AddButton";
export { TabScreen } from "./TabScreen";
export type { TabScreenProps } from "./TabScreen";
export { TabStack } from "./TabStack";
```

- [ ] **Step 4: Run the gate**

```bash
bun run format
make mb-check
```

Expected: `EXIT=0`. Nothing new renders — that is the correct outcome, not a failed step.

- [ ] **Step 5: Verify the mechanism with a temporary entry, then revert it**

The button is invisible today, so the only way to know the path works is to give it a destination
that already exists. `/catalog` is a real route.

1. Temporarily set, in `addActions.ts`:
   ```ts
   export const ADD_ACTIONS: readonly AddAction[] = [
     { key: "katalog", label: "Katalog komponen", href: "/catalog" },
   ];
   ```
2. Run the app. Screenshot, in **both themes**:
   - the floating **Tambah** button clear of the tab bar on Beranda and on Komunitas;
   - the sheet open, titled `Tambah ke <nama mobil>` — **AC3's pre-fill**;
   - the sheet with the **Mobil** select open on an account with two or more cars, and the title
     changed after picking the other one — **AC3's "changeable"**;
   - the entry tapped, landing on the catalog screen.
3. Confirm the vehicle change persisted: close the sheet, go to Beranda, and the card is the car
   you picked in the sheet.
4. **Revert step 1** so `ADD_ACTIONS` is empty again, re-run `bun run format && make mb-check`, and
   confirm with `git diff` that nothing of the temporary entry survives.
5. Record in `## Execution status` that AC2 and AC3 were verified this way and that the shipped
   build renders no add button — the AM-16 comment says the same thing.

**One risk this step may surface.** `AmSelect` opens its own `AmBottomSheet`, so the vehicle picker
is a `Modal` inside a `Modal`. If it renders clipped or refuses to open on iOS, the fix is to give
the add sheet two modes inside its single sheet — a list mode and a vehicle mode swapped by
state — rather than nesting. Do not fix it by reaching for a native picker.

### Brief blocks — Task 5

```
1. Every make target runs from the REPOSITORY ROOT.
2. Mobile gate: `make mb-check` (fmt-check -> expo typed routes -> tsc --noEmit
   -> expo lint). Whole repo: `make check`.
3. Bun, never npm. `bun add --filter` DOES NOT EXIST — use
   `bun add --cwd apps/mobile <pkg>` or `bun x expo install <pkg>`.
   `bun install --frozen-lockfile` must stay EXIT=0 with bun.lock unchanged.
4. Prettier runs from the ROOT only (`bun run format`). Markdown excluded.
5. ** expo-router SDK 56+ VENDORS ITS OWN NAVIGATION. ** Do NOT install
   @react-navigation/native — the router throws "no longer compatible with
   react-navigation". ThemeProvider/DarkTheme/DefaultTheme come FROM
   "expo-router".
6. _layout.tsx already overrides the navigation container background to
   transparent so AmGround shows through. Do not undo it. A tab bar must not
   reintroduce an opaque background that hides the ground.
7. NEVER put a changing `key` on a View wrapping {children} at the app root.
8. apps/mobile has NO test runner and this work does not add one. tsconfig
   strict; `@typescript-eslint/no-explicit-any: error`.
9. The AM-15 design system is complete and MUST be used — apps/mobile/src/app/
   catalog.tsx is the worked example.
10. Never set allowFontScaling={false}.
11. Root .env belongs to the BACKEND; apps/mobile reads only EXPO_PUBLIC_*.
12. CI workflows are path-filtered per app.
```

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

From the spec's anti-goals, cut to what this plan could plausibly violate:

- **No tab content for Explore and Community beyond an honest empty state.** The same reading
  applies to Garage: AM-16's own out-of-scope line is "isi setiap tab".
- **No invented counts, no seeded vehicles, no placeholder community numbers.** A null cost is
  omitted, never rendered as `Rp 0`. A car with no history says so in words.
- **No add-forms.** An entry whose destination does not exist is absent, never disabled, greyed,
  or labelled "segera hadir".
- **No native picker, dialog, or `Alert`** — `AmBottomSheet` is the pattern, and `AmSelect` is the
  pattern for picking one value from a list.
- **No second copy of anything Plan A owns**: no second HTTP client, no second token read, no
  second session store, no second error taxonomy, no second sign-out path.
- **No token in MMKV, in the query cache, in a log, or in a URL.** Nothing in this plan touches a
  token; if a task finds itself reading one, it has wandered into Plan A.
- **No second redirect on session expiry.** Sign-out is one transaction with one redirect; this
  plan calls `signOut()` and does not navigate afterwards.
- **No new design value.** No hex, no font size, no spacing, no radius that does not come from
  `useTheme()`.
- **No `@react-navigation/*` dependency.** expo-router 57 vendors its own navigation.

---

## Execution mode

The run-shape verdict §28 requires, written when the plan was saved.

**1. What runs in parallel, and what is serialised on what.** Almost all of it is serial, and by
analysis rather than by habit. Tasks 2, 3, 4, and 5 all edit
`apps/mobile/src/app/(app)/_layout.tsx` — Task 2 creates it, Tasks 3 and 4 insert `<Tabs.Screen>`
entries into it in a specific order, and Task 5 does not, but Task 5's `TabScreen` edit collides
with Task 2's creation of the same file. Every task from 2 onward also consumes Task 1's exports by
exact name. There is exactly one honest parallel pair: **Task 3 and Task 4** touch disjoint route
directories and collide only on the `_layout.tsx` screen list — dispatching them together means one
of them re-reads that file after the other lands. That is a small enough win that the correct call
is to run all five in order and say so, rather than to manufacture a queue.

**2. What the writers cannot discover for themselves.** The environment card above, pasted verbatim
into every brief. Plus five facts found while planning that no amount of reading the spec would
give:

- `Tabs` exported from `"expo-router"` is **deprecated** in SDK 57
  (`expo-router/build/exports.d.ts`); the live import is `expo-router/js-tabs`.
- `popToTopOnBlur` defaults to `false`
  (`expo-router/build/react-navigation/bottom-tabs/types.d.ts:200`) — that default *is* AC1's stack
  half, so nothing may set it.
- Tab order comes from the order of declared `<Tabs.Screen>` children
  (`expo-router/build/useScreens.js:63`), not from file names.
- Neither `@expo/vector-icons` nor `react-native-svg` is installed; `expo-symbols` is iOS-first
  with an `unstable_` Android path. Task 2 adds one font-based icon dependency and says why.
- `GET /vehicles` already carries each car's service rollup, so the Home screen needs **one**
  request, not two — `GET /vehicles/{id}/summary` returns a different, richer answer that nothing
  in this plan renders.

**3. Where the risk concentrates.** Three places.

- **Task 2's `(app)/_layout.tsx`**, because Plan A may already own that file and a careless
  overwrite removes the auth gate — the one defect here that is a security defect. The brief says
  read it first and replace only the navigator element.
- **Task 1's two Plan A assumptions** (what `apiRequest` resolves; where the active-vehicle hooks
  live). Wrong, they surface as type errors in Task 2 rather than as bad behaviour — cheap, but
  they invalidate the code blocks in three later tasks, so step 1 verifies them before anything is
  written and corrects this file.
- **Task 5's honesty.** The temptation is to ship a visible "+" that opens onto nothing so the
  ticket looks complete. Everything in Task 5 is arranged to make that hard to do by accident.

Nothing here touches money, auth logic, a migration, a column, or a public contract — the auth
*gate* is the only thing on the floor list this plan can break, and only by deleting it.

**4. What the plan is missing.** Nothing structural: every task carries `Files:`, `Interfaces:`, a
`TDD:` verdict with its reason, acceptance criteria traced to AM-16's ACs, the environment card, and
the quality gate. Two things are deliberately deferred rather than absent, and both are named where
they land: the *stack* half of AC1 is verified structurally because no tab has a second screen
(Task 3, step 7), and AC2's add entries do not exist to be shipped (Task 5).

**Reviewer tier and lenses.** `opus` is not required by the floor list; `sonnet` is the right tier
for these diffs, with one exception — the Task 2 diff touching `(app)/_layout.tsx` gets `opus`,
because that file carries the auth gate. Every reviewer brief asks the three questions this plan can
actually fail: does any screen render a number the server did not send; does any control exist
whose destination does not; and does any style carry a value that did not come from `useTheme()`.

---

## Execution status

| Task | Status | Notes |
|---|---|---|
| 1 — Vehicle data layer and formatters | not started | |
| 2 — Tab group and Home tab | not started | |
| 3 — Garasi, Jelajah, Komunitas | not started | |
| 4 — Profile tab | not started | |
| 5 — Global add action | not started | |

Record here, per task: corrections the plan got wrong, deliberate cuts, defects found and how they
were closed, and the screenshots taken. This file is the resume map after compaction; the chat is
not.

---

## Review findings ledger

None yet. Each finding lands here the moment it arrives, with: task, severity
(`structural` · `correctness` · `test-integrity` · `hygiene`), file and line, the concrete failure
scenario, and the smallest fix. Findings are worked in one pass after the final task — except a
finding that changes a column, constraint, or public contract, which is fixed immediately.

| Task | Severity | File:line | Failure scenario | Smallest fix | Closed by |
|---|---|---|---|---|---|
