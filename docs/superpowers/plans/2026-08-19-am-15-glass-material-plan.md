# AM-15 — Design system and glass material for `apps/mobile`

> **For agentic workers:** REQUIRED SUB-SKILL: use `executing-plans-hybrid` to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** give `apps/mobile` one material system and the base component primitives — tokens (both themes), a ground, a three-role material, input/display/state primitives with the `Am` prefix, and an internal catalogue screen that proves contrast, touch targets, and large-text behaviour — and stop before any feature-specific component.

**Architecture:** the shared `packages/tokens` package stays the single source of values. It gains a **material** group (composited colours per role per theme, edge, ground stops), a **derive** module (`relativeLuminance` / `contrastRatio` / `composite` / `mix` / `withAlpha` / `parseTypeShorthand`) and a test that asserts every material × text-role pair clears WCAG AA against both a black and a white backdrop. `apps/mobile` declares the package as a dependency and wraps it in a React-Native-shaped theme layer (`useTheme()`), on top of which sit `AmGround`, `AmMaterial`, and eleven primitives. The material renders as **tint + edge** by default — which is the Android < 31 reality — and upgrades to iOS Liquid Glass through the already-installed `expo-glass-effect` when, and only when, the runtime says it is available.

**Tech Stack:** React Native 0.86.2 · Expo SDK 57 · TypeScript strict · Bun workspaces · `expo-glass-effect` (installed) · `expo-font` (installed) · `react-native-reanimated` 4.5.1 + `react-native-gesture-handler` 2.32 (installed) · `packages/tokens` (plain ESM JavaScript + `node:test`, run by `bun test`).

**Spec:** [`docs/superpowers/specs/2026-08-19-am-15-glass-material-design.md`](../specs/2026-08-19-am-15-glass-material-design.md) — read it alongside this plan. It survived a cross-model BLOCK and a grill against `docs/design.md`; its decisions are binding and are not re-opened here. Where this plan supplies a number the spec left to implementation, the number is computed and recorded, never guessed.

**Ticket:** [AM-15](https://oksasatyaa.atlassian.net/browse/AM-15) · subtasks [AM-25](https://oksasatyaa.atlassian.net/browse/AM-25) · [AM-26](https://oksasatyaa.atlassian.net/browse/AM-26) · [AM-27](https://oksasatyaa.atlassian.net/browse/AM-27) · [AM-28](https://oksasatyaa.atlassian.net/browse/AM-28) · [AM-29](https://oksasatyaa.atlassian.net/browse/AM-29)

---

## Global constraints

Copied from the spec, from AM-15's own acceptance criteria, and from `docs/design.md`. Every task's requirements implicitly include this section.

- **The binding token is the composited colour that passes WCAG AA — never an opacity value.** Percentages in the spec and in this plan describe how a surface ends up looking; they are rendering inputs. Two surfaces at the same coverage over different grounds are different colours and only one may pass. If a design ever needs a coverage that breaks the contract, **the surface becomes solid** — the contract wins, always.
- **Three material roles, distinguished by coverage, not by blur radius.** `chrome` (app bar, tab bar, floating AI entry — the most glass, short primary-token labels only) · `surface` (content cards, sheets, list panels — high coverage, reads as workshop milk-glass) · `working` (**solid, zero transparency**: service history, fitment results, forms, AI evidence, AI warnings, confidence badges, and the eight §73 signature components).
- **Secondary and tertiary text may never sit on a material whose backdrop is unknown.** Verified below: tertiary would need a black scrim at 93.3% over white, which is opaque in all but name. `chrome` carries the primary text token only.
- **The ground is pure code** — a graphite gradient accepting one optional tint colour, falling back to neutral. No image asset, no blur, no photograph. **Where the tint comes from is not built here**; colour extraction is deferred to the garage epic.
- **The edge is a 1px highlight on the top edge only plus an inset shadow at the bottom.** A uniform 1px white border on all four sides is **forbidden by name** — it is the single most recognisable signature of templated glassmorphism.
- **Orange is never the material.** Solid = pressable, glass = container. Orange appears as content on glass or as a solid fill, never as the material itself, and never with a glow (§50 bans "constant glowing orange effects").
- **Live blur is an enhancement, never the foundation.** `expo-blur` defaults to `blurMethod: 'none'` on Android, which is a tint and not a blur; a real blur needs Android 12+. The no-blur rendering must look intentional, not degraded. **This plan installs no blur library at all** — see the decision in Task 4.
- **Touch targets are ≥ 44 × 44 pt without the caller adding padding** (§61, AM-15 AC3).
- **Component names carry the `Am` prefix** (§69): `AmButton`, `AmCard`, `AmChip`, `AmBadge`, `AmTextField`, `AmEmptyState`, `AmBottomSheet`, `AmAppBar`. New names introduced here follow the same shape: `AmGround`, `AmMaterial`, `AmSelect`, `AmAvatar`, `AmErrorState`, `AmSkeleton`, `AmToast`.
- **No raw values in a component** (AM-15 AC1). No hex codes, no font sizes, no spacing numbers written as literals. They come from `useTheme()`. If a value is missing, it is added to `packages/tokens`, not inlined.
- **The 85 / 10 / 5 ratio is a rule about UI tokens, imagery excluded** (AM-15 AC4, and the spec's revision to §74). A red car cannot be allowed to falsify it.
- **Product-facing strings are Bahasa Indonesia.** Code, comments, commit messages, this plan, and `docs/` are English.
- **Bun, never npm.** `bun install --frozen-lockfile` must leave `bun.lock` unchanged; never a nested lockfile under `apps/mobile`.

---

## Environment card — Block G

**Paste this verbatim into every writer brief and every reviewer brief.** None of it is in the spec, and each line is a real trap in this repository's shape.

```
ENVIRONMENT — AnakMobil AM-15 glass material, read before running anything

1. Every `make` target runs from the REPOSITORY ROOT, never from apps/mobile
   and never from packages/tokens.

2. Two gates, and they are different chains for different directories:
     packages/tokens changes ->  make ds-check
        = bun run --filter @anakmobil/tokens check
        = bun scripts/build.mjs  &&  bun test test/
     apps/mobile changes     ->  make mb-check
        = fmt-check (prerequisite) -> bun run --filter @anakmobil/mobile check
        = expo customize tsconfig.json -> tsc --noEmit -> expo lint
   A change touching both runs BOTH. `make check` runs everything.

3. Prettier does NOT read the root .prettierignore when run from inside a
   workspace — it resolves the ignore file relative to the working directory.
   That is why `fmt-check` is a Make PREREQUISITE (`mb-check: fmt-check`)
   rather than a line inside a workspace `check` script. Do not undo that.
   Run `bun run format` (root) before the gate; `bun run format:check` is what
   CI runs.

4. `bun test` runs `node:test` files unchanged. packages/tokens/test/
   tokens.test.mjs already imports from 'node:test' and passes (10 tests).
   A new test file dropped in packages/tokens/test/ is picked up by the
   existing `bun test test/` glob — no config change needed.

5. @anakmobil/tokens is NOT currently linked into apps/mobile. There is no
   node_modules/@anakmobil there, and importing it from apps/mobile fails
   today. Bun only links a workspace package that is DECLARED as a dependency.
   The precedent is apps/landing/package.json: `"@anakmobil/tokens": "*"`,
   which Bun resolves to a symlink ../../../../packages/tokens. Adding the
   dependency therefore requires a `bun install` from the root and a
   re-verification that `bun install --frozen-lockfile` still exits 0.

6. packages/tokens is `"type": "module"` with `"main": "./src/tokens.js"` and
   an `exports` map. Its `./css` and `./theme` entries are CSS artifacts and
   MUST NOT be imported from React Native. Metro enables package `exports`
   resolution by default on recent Expo SDKs, so the `exports` map is what
   resolves — VERIFY this at runtime the first time it is imported (Task 2
   Step 4), do not assume it.

7. apps/mobile does NOT have `"type": "module"` and must not gain one — the
   Expo toolchain assumes CommonJS interop. Importing an ESM workspace package
   is fine; Metro transpiles it.

8. Routes live in src/app/ (SDK-57 template layout), alias `@/*` -> `./src/*`
   and `@/assets/*` -> `./assets/*`, already configured in tsconfig.json.
   Non-route code lives in src/ outside src/app/ — Expo Router only treats
   src/app/ as the route tree.

9. apps/mobile has NO test runner. Its `check` script is tsc + eslint only.
   Do not add one for this ticket: the one piece of pure input->output logic
   here (the contrast arithmetic) lives in packages/tokens, which already has
   a runner and its own CI job.

10. eslint.config.js sets @typescript-eslint/no-explicit-any: error. tsconfig
    extends expo/tsconfig.base, which is already `strict: true`.

11. Installed and usable with no new dependency:
      expo-glass-effect  GlassView, GlassContainer, isLiquidGlassAvailable(),
                         isGlassEffectAPIAvailable().  iOS only — the non-iOS
                         build of GlassView is literally `<View {...props} />`
                         and both predicates return false off-iOS.
      expo-font          useFonts()
      expo-image         Image (Avatar)
      react-native-reanimated 4.5.1 + react-native-worklets 0.10.1
      react-native-gesture-handler 2.32
      react-native-safe-area-context 5.7
    NOT installed: expo-blur, expo-linear-gradient, @gorhom/bottom-sheet.
    None of them is needed — see Task 4.

12. React Native 0.86.2 gives us, with no library:
      style.experimental_backgroundImage  accepts a CSS linear-gradient string
      style.boxShadow                     accepts a CSS shadow string, `inset`
                                          supported (BoxShadowValue.inset)
      Text style.fontVariant: ['tabular-nums']
      AccessibilityInfo.isReduceTransparencyEnabled() / 'reduceTransparencyChanged'
    fontWeight accepts '100'..'900' and the numbers 100..900 — but NOT 650.
    The mobile type scale's 650 steps therefore render at 600; see Task 3.

13. `make dev` starts db + API + landing + Metro and opens the iOS simulator.
    `make dev m=none` skips the app. `make mb-run-dev p=ios|android` is the
    native device build (OWNER — needs Xcode / Android Studio and a device).

14. CI: .github/workflows/mobile.yml is path-filtered on apps/mobile/** and
    mirrors mb-check. .github/workflows/frontend.yml is path-filtered on
    apps/landing/** AND packages/tokens/** and mirrors fe-check (which
    includes ds-check). A token change reddens the frontend job, not the
    mobile one — both must be watched to green on this branch.

15. Changing a value in packages/tokens changes what apps/landing renders,
    because dist/tokens.css is regenerated from the same source. That is
    intended for the tertiary-text repair in Task 1 and must be stated in the
    commit message, not discovered by the reviewer.
```

---

## JS/TS quality gate — Block Q (NOT Sonar)

**Paste verbatim into every brief that writes JavaScript or TypeScript.** This repository runs **no SonarQube** for JS/TS. Telling an implementer to invoke Sonar sends them after a tool that is not installed.

```
# AM-15 quality gate — write compliant from the first commit (NO Sonar)

Gate chain, checking EXIT CODES (piped output is not evidence):
  bun install --frozen-lockfile   # bun.lock must be unchanged afterwards
  bun run format                  # from the ROOT, before checking
  make ds-check                   # packages/tokens: build then node:test
  make mb-check                   # fmt-check -> typed routes -> tsc -> eslint

- TypeScript `strict` is on and inherited from expo/tsconfig.base. No
  `@ts-ignore`, no `@ts-expect-error` without a one-line reason above it.
- NO explicit `any` — eslint `@typescript-eslint/no-explicit-any: error`.
  Prefer `unknown` plus narrowing.
- React component props are `readonly` and declared as an exported interface
  named `<Component>Props`.
- NO raw design values in a component: no hex string, no font size, no
  spacing or radius number written as a literal. Everything comes from
  `useTheme()`. A missing value is added to packages/tokens.
- Every interactive element has a minimum 44x44 pt hit area WITHOUT the
  caller adding padding — `minHeight`/`minWidth` on the pressable itself, or
  `hitSlop` when the visual is deliberately smaller.
- Never set `allowFontScaling={false}`. Large system text must reflow.
- Product-facing strings are Bahasa Indonesia; identifiers, comments, and
  commit messages are English.
- packages/tokens stays plain ESM JavaScript with a hand-written .d.ts. A key
  added to tokens.js MUST get its declaration in tokens.d.ts — the enforcement
  is apps/mobile's `tsc --noEmit`, which fails with "has no exported member"
  the moment mobile imports something the .d.ts does not declare.
- No new runtime dependency without saying, in the commit message, what
  platform feature it replaces and why the platform feature was not enough.

When fixing one instance, scan sibling files for the same shape and fix
forward. When reviewing, check the diff against this list BEFORE marking
compliant.
```

---

## Algorithmic complexity — N/A for the whole ticket, with one deliberate exception

Stated so its absence is a decision rather than an oversight. This is tokens, styling, and eleven presentational components; there is no loop over a collection that grows, no query, no cache, and no data-structure choice on any hot path.

The one place arithmetic appears at all is `packages/tokens/src/derive.js`, whose functions are O(1) over a fixed three-channel colour and are called a handful of times at module load or once per ground render. The contrast **test** iterates the material matrix — six roles × at most three text roles × three backdrops — which is a constant, not an `n`. No task carries a Big-O annotation because no task has an algorithm to annotate.

---

## Run-shape verdict (§28)

**1 · What runs in parallel, and what is serialised on what.**

The dependency map, drawn from every task's `Files:` and `Interfaces:` before task 1 is dispatched:

```
T1 tokens + derive + contrast test     (packages/tokens)
      |
      +--> T2 mobile theme bridge      (apps/mobile/src/theme)     [consumes T1 exports]
      |         |
      |         +--> T3 typography + Inter                         [same theme dir - serialise]
      |                   |
      |                   +--> T4 AmGround + AmMaterial            [consumes T2+T3]
      |                             |
      |                             +--> T5 input primitives    ---+
      |                             +--> T6 display primitives  ---+--> T8 catalogue --> T9 OWNER device
      |                             +--> T7 state primitives    ---+
      |
      +--> T10 docs/design.md revision  (docs/)                     [needs T1's final numbers only]
```

- **T1 gates everything.** Every downstream task reads a token it defines. Nothing starts before it lands.
- **T2 and T3 are serialised on each other** — both write `apps/mobile/src/theme/` and T3 fills in the `type` slot of the `Theme` object T2 defines. Two writers there race the same file.
- **T5, T6, T7 run concurrently.** Their file sets are disjoint (`src/components/input/`, `src/components/display/`, `src/components/state/`), none imports another, and all three consume only the T4 interface. This is the ticket's one genuine fan-out and it is worth taking: three writers finish in roughly the time of the slowest.
- **T10 runs concurrently with T4–T8** from the moment T1's numbers exist. It touches only `docs/design.md`, which no other task opens.
- **T8 consumes all three primitive groups** and cannot start until they land. **T9 is OWNER-executed** and cannot be dispatched at all.

Expected shape: T1 → T2 → T3 → T4 → {T5, T6, T7, T10 in flight together} → T8 → T9. Three to four writers at the peak, which is inside the 3–5 band.

**2 · What the writers cannot discover for themselves.** Block G. Written once, pasted verbatim into every brief. The five that would otherwise cost a writer a wasted cycle: the tokens package is not linked into mobile yet (item 5), Prettier's working-directory trap (item 3), mobile has no test runner (item 9), the RN 0.86 platform features that remove the need for a gradient or blur library (items 11–12), and `fontWeight` rejecting 650 (item 12).

**3 · Where the risk concentrates.** **T1.** It changes two published token values (`light.textTertiary`, `dark.textTertiary`) that `apps/landing` already renders, and it fixes the composited colours that every later task's contrast claim rests on. A wrong number there is not a styling bug — it is eleven components built on a surface that fails AA, discovered on the catalogue screen at the end. T1 is the only `TDD: yes` task in the plan, and that is why.

Second: **T4**, because the material renderer is the single choke point where the platform ladder, the reduced-transparency path, and the edge treatment meet. A defect there reproduces in every primitive.

**4 · What the plan is missing.** Nothing deferred to improvisation: every task below carries a `TDD:` verdict with its reason, concrete acceptance criteria, `Files:`, and `Interfaces:`. The `Tidak boleh ada` block carries the spec's anti-goals verbatim.

**TDD across the ticket.** One `yes` (T1 — the contrast arithmetic is pure input→output with a checkable contract, and it is the mechanism the whole design rests on). Every other task is `no — verify by running`: styling and component composition are verified by opening the catalogue on a simulator and reading the recorded contrast table, not by a unit test that would assert the same literals twice.

---

## Tidak boleh ada

The spec's anti-goals, carried verbatim so a task cannot quietly grow past its brief. Anything on this list appearing in a diff is a finding, not an improvement.

- **No image assets are requested for this ticket.** The ground is code; the missing-photo placeholder is an SVG silhouette (§48 already requires a "neutral silhouette", never a stock car implying the wrong model).
- **No blur on long scrolling lists, and no per-item blur anywhere.**
- **No stacking glass on glass beyond two layers.**
- **No animated blur, no animated glass, no refraction or "liquid" decoration.**
- **No colour-extraction pipeline** — deferred to the garage epic. `AmGround` accepts a tint it is given and falls back to neutral; nothing in this ticket produces that tint.
- **No feature-specific components.** AM-15 builds primitives; `AmVehicleCard`, `AmBuildCard`, `AmFitmentCard`, `AmProblemCard`, `AmServiceCard`, `AmConfidenceBadge`, and `AmEvidenceCard` belong to their own epics.
- **No backoffice / dense-table components** (E13).

Three more, added by this plan because the shape of the work invites them:

- **No glass on the eight §73 signature components, on AI warnings, or on confidence badges** — they are `working`, solid, always. None of them is built here, but `AmMaterial` must make the wrong choice impossible to express accidentally, and `AmBadge` must not default to a translucent role.
- **No `AmAppBar` and no tab bar.** Both are `chrome` surfaces and both belong to the app-shell story. The catalogue renders one `chrome` sample so the role is exercised and verifiable; it does not become navigation.
- **No new runtime dependency for gradients, blur, or sheets.** RN 0.86 supplies `experimental_backgroundImage`, `boxShadow` with `inset`, and `Modal`; Reanimated and Gesture Handler are already installed. `expo-blur`, `expo-linear-gradient`, and `@gorhom/bottom-sheet` are each a rung the platform already covers.

---

## The numbers this plan is built on

Computed at plan time with the exact sRGB relative-luminance formula, and re-derived by the test in Task 1 rather than trusted from here. Recorded so a reviewer can check the plan's arithmetic without re-deriving it.

### Correction to the spec's scrim table

The spec's table gives the black-scrim opacity needed over a white backdrop as 83.3% / 94.1% / 99.4% for primary / secondary / tertiary. **Those figures are `1 − L`, the linear luminance ceiling read as if it were an sRGB alpha.** Converting the ceiling back through the sRGB transfer function gives the real alphas:

| Text role | Luminance | Max background L for 4.5:1 | Real black-scrim alpha over white | Spec says |
|---|---|---|---|---|
| primary `#F5F7F8` | 0.9271 | 0.1671 | **55.4%** | 83.3% |
| secondary `#AAB2BA` | 0.4393 | 0.0587 | **73.1%** | 94.1% |
| tertiary `#737D87` | 0.2006 | 0.0057 | **93.3%** | 99.4% |

**Every conclusion the spec drew from that table survives**, which is why this is a correction and not a re-opening: 93.3% is still opaque in all but name, tertiary is still hopeless on an unknown backdrop, and a surface carrying text is still nearly opaque. Only the printed percentages were wrong. Task 10 corrects them in the spec's own words when it revises `docs/design.md`.

### Pre-existing contrast failures in `docs/design.md`, found while computing the above

None of these is caused by glass. All of them block AM-15 AC2 ("seluruh teks memenuhi rasio kontras minimal AA") and AM-25's definition of done, and all of them are repaired in Task 1.

| Pair | Ratio | Verdict |
|---|---|---|
| `dark.textTertiary #737D87` on `dark.surface #151A20` | 4.18 | fails 4.5 |
| `dark.textTertiary #737D87` on `dark.surfaceRaised #1B2128` | 3.87 | fails 4.5 |
| `light.textTertiary #8A939D` on `light.surface #FFFFFF` | 3.12 | fails 4.5 |
| `light.textTertiary #8A939D` on `light.surfaceSubtle #F1F3F5` | 2.80 | fails 4.5 **and** the 3:1 large-text floor |
| white on `accent-500 #ED491C` (§42's "strongest brand CTA") | 3.77 | fails 4.5 |
| `semantic.success #168A52` as text on `#151A20` / `#FFFFFF` | 3.99 / 4.38 | fails both |
| `semantic.danger #D63B3B` as text on `#151A20` | 3.79 | fails |
| `semantic.info #2678D9` as text on `#151A20` / `#FFFFFF` | 3.97 / 4.40 | fails both |
| `semantic.warning #D58A00` as text on `#FFFFFF` | 2.81 | fails badly |

### The material matrix this plan implements

`solid` is the composite over the app's own ground and is what renders whenever transparency is off. `overWhite` / `overBlack` are the two extremes the contract is asserted against.

| Role | tint | coverage | solid | overWhite | overBlack | text allowed |
|---|---|---|---|---|---|---|
| `dark.chrome` | `#0E1217` | 80% | `#0E1217` | `#3E4145` | `#0B0E12` | primary |
| `dark.surface` | `#151A20` | 92% | `#14191F` | `#282C32` | `#13181D` | primary, secondary |
| `dark.working` | `#151A20` | 100% | `#151A20` | — | — | primary, secondary, tertiary |
| `light.chrome` | `#FBFCFD` | 80% | `#FAFBFC` | `#FCFDFD` | `#C9CACA` | primary |
| `light.surface` | `#FCFDFD` | 92% | `#FCFDFD` | `#FCFDFD` | `#E8E9E9` | primary, secondary |
| `light.working` | `#FFFFFF` | 100% | `#FFFFFF` | — | — | primary, secondary, tertiary |

Every allowed pair, at every backdrop:

| Role | primary | secondary | tertiary |
|---|---|---|---|
| `dark.chrome` | 9.55 / 18.00 / 17.49 | not allowed | not allowed |
| `dark.surface` | 13.06 / 16.62 / 16.44 | 6.54 / 8.32 / 8.23 | not allowed |
| `dark.working` | 16.28 | 8.15 | 5.97 |
| `light.chrome` | 16.81 / 10.43 / 16.54 | not allowed | not allowed |
| `light.surface` | 16.81 / 14.09 / 16.81 | 5.72 / 4.80 / 5.72 | not allowed |
| `light.working` | 17.13 | 5.83 | 5.49 |

Triples are `overWhite / overBlack / overGround`. **All pairs pass 4.5:1.**

Two decisions inside that table which the spec left to implementation, with the arithmetic that forced them:

- **`light.surface` uses an off-white tint `#FCFDFD` at 92%, not `#F1F3F5`.** §41's subtle grey at 92% gives secondary text only **4.40** over a dark backdrop — a fail. Every light tint darker than about `#FAFBFC` fails the same way, and reaching AA with a darker tint needs ~96% coverage, which is solid in all but name. This is also how "light mode tints dark, never white-on-white" is honoured without breaking the contract: the composite is never pure white, and **the edge, not the fill, is what separates the surface from the page** — which is the spec's own thesis about where glass gets its form.
- **`working` keeps §46's border, not the glass edge.** "Use borders before shadows" is superseded for `chrome` and `surface` and survives intact for `working`. The spec's §15 rewrite says so rather than claiming the rule was replaced wholesale.

### Token repairs and additions Task 1 makes

| Token | From | To | Why |
|---|---|---|---|
| `dark.textTertiary` | `#737D87` | `#8E98A2` | 3.87 → 5.53 on `surfaceRaised`, 5.97 on `surface` |
| `light.textTertiary` | `#8A939D` | `#616A74` | 2.80 → 4.94 on `surfaceSubtle`, 5.49 on `surface` |
| `onAccent` (new) | — | `#0F141A` | graphite-950 on `accent-500` = **4.91**; white was 3.77. Keeps the brand orange exactly as §74/§76 specify and changes only the label. |
| `semanticText.dark` (new) | — | success `#1FA463` · warning `#D58A00` · danger `#EC6363` · info `#4A93E8` | 5.44 / 6.22 / — / 5.52 on `dark.working` · *(danger corrected in execution: the planned `#E45B5B` reached only 4.26 on `surfaceSubtle`, which the plan's own test asserts; `#EC6363` gives 4.68 subtle / 5.44 working)* |
| `semanticText.light` (new) | — | success `#137747` · warning `#8F5C00` · danger `#C22C2C` · info `#1F63B5` | 5.59 / 5.68 / 5.69 / 5.98 on white; 5.02 / 5.10 / 5.12 / 5.38 on `surfaceSubtle` |

The base `semantic` values are unchanged and stay what they are: **fills, borders, and icons**. They are never used as text. That is the whole reason `semanticText` exists as a separate group, and it is also why no primitive in this plan renders a saturated pill with a white label — a badge is a neutral fill plus a semantic border, a semantic icon, and `semanticText`, which satisfies §61's "do not communicate status by colour alone" for free.

---

## File structure

| File | Responsibility | Task |
|---|---|---|
| `packages/tokens/src/derive.js` | pure colour + type maths: `relativeLuminance`, `contrastRatio`, `meetsAA`, `composite`, `mix`, `withAlpha`, `parseTypeShorthand` | 1 |
| `packages/tokens/src/derive.d.ts` | hand-written types for the above | 1 |
| `packages/tokens/src/tokens.js` | adds `material`, `edge`, `ground`, `onAccent`, `semanticText`; repairs both `textTertiary` values | 1 |
| `packages/tokens/src/tokens.d.ts` | declarations for the new groups | 1 |
| `packages/tokens/package.json` | adds the `./derive` export entry | 1 |
| `packages/tokens/test/derive.test.mjs` | the maths, against known WCAG values | 1 |
| `packages/tokens/test/material.test.mjs` | the matrix: every role × allowed text role × {white, black, ground} ≥ 4.5 | 1 |
| `packages/tokens/test/tokens.test.mjs` | extended: new groups obey the hex rule, both themes keep matching keys | 1 |
| `packages/tokens/scripts/build.mjs` | one comment: the material group is deliberately **not** emitted to CSS | 1 |
| `apps/mobile/package.json` | adds `@anakmobil/tokens` and the Inter font package | 2, 3 |
| `apps/mobile/src/theme/types.ts` | `Theme`, `MaterialRecipe`, `MaterialRole`, `TextRole`, `TypeName` | 2 |
| `apps/mobile/src/theme/theme.ts` | builds the light and dark `Theme` objects from the tokens | 2, 3 |
| `apps/mobile/src/theme/ThemeProvider.tsx` | context + `useTheme()`, system scheme with an override for the catalogue | 2 |
| `apps/mobile/src/theme/capability.ts` | `useMaterialCapability()` — the platform ladder and reduced-transparency | 2 |
| `apps/mobile/src/theme/typography.ts` | parsed RN text styles, the 650→600 mapping, tabular figures | 3 |
| `apps/mobile/src/theme/fonts.ts` | `useAppFonts()` — Inter cuts through `expo-font` | 3 |
| `apps/mobile/src/components/material/AmGround.tsx` | the ground: gradient, optional tint, neutral fallback | 4 |
| `apps/mobile/src/components/material/AmMaterial.tsx` | the three roles, the edge, the ladder, `working` always solid | 4 |
| `apps/mobile/src/components/input/AmButton.tsx` | variants, states, ≥44pt | 5 |
| `apps/mobile/src/components/input/AmTextField.tsx` | label, states, ≥16 input text | 5 |
| `apps/mobile/src/components/input/AmSelect.tsx` | opens `AmBottomSheet`, never a native picker | 5 |
| `apps/mobile/src/components/display/AmCard.tsx` | role-aware card, `working` default for data | 6 |
| `apps/mobile/src/components/display/AmChip.tsx` | selectable, ≥44pt hit area | 6 |
| `apps/mobile/src/components/display/AmBadge.tsx` | neutral fill + semantic border/icon/text, never colour alone | 6 |
| `apps/mobile/src/components/display/AmAvatar.tsx` | `expo-image` with an initials fallback | 6 |
| `apps/mobile/src/components/display/AmBottomSheet.tsx` | gesture- and button-dismissible, no native dialog | 6 |
| `apps/mobile/src/components/state/AmEmptyState.tsx` | always carries exactly one action | 7 |
| `apps/mobile/src/components/state/AmErrorState.tsx` | §53 tone, retry action | 7 |
| `apps/mobile/src/components/state/AmSkeleton.tsx` | drawn on the same material as what it replaces | 7 |
| `apps/mobile/src/components/state/AmToast.tsx` | provider + `useToast()`, semantic border not fill | 7 |
| `apps/mobile/src/app/_layout.tsx` | wraps the tree in `ThemeProvider` + `ToastProvider` + `AmGround` | 2, 7 |
| `apps/mobile/src/app/catalog.tsx` | every primitive, every state, theme + no-blur toggles | 8 |
| `apps/mobile/src/app/index.tsx` | one link to the catalogue; healthcheck otherwise untouched | 8 |
| `docs/design.md` | §7, §8, §9, §15, §40, §41, §42, §46, §47, §50, §66, §67, §72, §74, §76 + a new Material System section | 10 |

---

## Task 1: Colour maths, the material tokens, and the contrast contract

The one task with a checkable input→output contract, and the one everything else stands on. It adds a pure maths module, uses it to repair two shipped token values and add the material group, and writes the test that makes "this surface passes AA" a fact the gate re-checks rather than a claim in a document.

**Files:**
- Create: `packages/tokens/src/derive.js`, `packages/tokens/src/derive.d.ts`
- Create: `packages/tokens/test/derive.test.mjs`, `packages/tokens/test/material.test.mjs`
- Modify: `packages/tokens/src/tokens.js`, `packages/tokens/src/tokens.d.ts`
- Modify: `packages/tokens/package.json` (one `exports` entry)
- Modify: `packages/tokens/test/tokens.test.mjs` (extend the existing hex sweep)
- Modify: `packages/tokens/scripts/build.mjs` (one comment, no behaviour change)

**Interfaces:**
- Consumes: nothing.
- Produces, from `@anakmobil/tokens`: `material` (`{ light, dark }` → `{ chrome, surface, working }` → `{ tint: string, coverage: number, solid: string, allowsText: readonly TextRole[] }`), `edge` (`{ light, dark }` → `{ highlight: string, insetShadow: string, borderWidth: number }`), `ground` (`{ light, dark }` → `{ stops: readonly {color: string, at: number}[], tintStrength: number }`), `onAccent: string`, `semanticText` (`{ light, dark }` → `{ success, warning, danger, info }`), and repaired `light.textTertiary` / `dark.textTertiary`.
- Produces, from `@anakmobil/tokens/derive`: `relativeLuminance(hex): number`, `contrastRatio(a, b): number`, `meetsAA(fg, bg, large?): boolean`, `composite(tint, coverage, backdrop): string`, `mix(a, b, weight): string`, `withAlpha(hex, alpha): string`, `parseTypeShorthand(value): { fontWeight: number, fontSize: number, lineHeight: number }`.

**TDD: yes** — this is pure input→output logic with a contract that can be stated before it is written: given a text colour and a composited surface colour, does the pair reach 4.5:1? The failure mode is silent and expensive (eleven components built on a surface that fails AA, discovered at the end), and the arithmetic is exactly the kind "looks right" cannot catch. Red first, and the red must be for the intended reason.

**Minimality check.** One new module, not three: `derive.js` holds the colour maths *and* `parseTypeShorthand` because both are "things you compute from a token" and splitting them buys nothing at this size. No colour library is added — the WCAG formula is nine lines. The material group is **not** emitted into `dist/tokens.css` or `dist/theme.css`: it is mobile-only, and emitting it would put dead custom properties into the backoffice's Tailwind theme.

### Steps

- [ ] **Step 1: write the failing maths test**

Create `packages/tokens/test/derive.test.mjs`:

```js
import assert from "node:assert/strict";
import test from "node:test";

import {
  composite,
  contrastRatio,
  meetsAA,
  mix,
  parseTypeShorthand,
  relativeLuminance,
  withAlpha,
} from "../src/derive.js";

test("relative luminance matches the WCAG reference values", () => {
  // The two anchors the whole formula is pinned to.
  assert.equal(relativeLuminance("#FFFFFF").toFixed(4), "1.0000");
  assert.equal(relativeLuminance("#000000").toFixed(4), "0.0000");
  // A mid grey sits below 0.5 because the transfer function is not linear —
  // getting this wrong is the classic way a contrast checker silently lies.
  assert.equal(relativeLuminance("#808080").toFixed(4), "0.2159");
});

test("contrast ratio is symmetric and hits the known extremes", () => {
  assert.equal(contrastRatio("#000000", "#FFFFFF").toFixed(2), "21.00");
  assert.equal(contrastRatio("#FFFFFF", "#000000").toFixed(2), "21.00");
  assert.equal(contrastRatio("#777777", "#777777").toFixed(2), "1.00");
});

test("meetsAA uses 4.5 for body text and 3.0 for large text", () => {
  // #767676 on white is the canonical 4.54 boundary case.
  assert.equal(meetsAA("#767676", "#FFFFFF"), true);
  assert.equal(meetsAA("#777777", "#FFFFFF"), false);
  assert.equal(meetsAA("#777777", "#FFFFFF", true), true);
});

test("composite blends a tint over a backdrop at a coverage", () => {
  assert.equal(composite("#000000", 0.5, "#FFFFFF"), "#808080");
  assert.equal(composite("#151A20", 1, "#FFFFFF"), "#151A20");
  assert.equal(composite("#151A20", 0, "#FFFFFF"), "#FFFFFF");
  // The number the dark surface role is built on.
  assert.equal(composite("#151A20", 0.92, "#FFFFFF"), "#282C32");
  assert.equal(composite("#151A20", 0.92, "#000000"), "#13181D");
});

test("mix is composite with the arguments named for a tint", () => {
  assert.equal(mix("#FFFFFF", "#000000", 0.5), "#808080");
  assert.equal(mix("#ED491C", "#0E1217", 0), "#0E1217");
});

test("withAlpha renders an rgba string a React Native style accepts", () => {
  assert.equal(withAlpha("#151A20", 0.92), "rgba(21, 26, 32, 0.92)");
  assert.equal(withAlpha("#FFFFFF", 1), "rgba(255, 255, 255, 1)");
});

test("parseTypeShorthand splits the CSS font shorthand the tokens use", () => {
  assert.deepEqual(parseTypeShorthand("700 32px/38px"), {
    fontWeight: 700,
    fontSize: 32,
    lineHeight: 38,
  });
  assert.deepEqual(parseTypeShorthand("400 14px/21px"), {
    fontWeight: 400,
    fontSize: 14,
    lineHeight: 21,
  });
});

test("parseTypeShorthand rejects a malformed value instead of guessing", () => {
  // A silent NaN here would render invisible text, so it must throw.
  assert.throws(() => parseTypeShorthand("bold 16px"), /type shorthand/);
});
```

- [ ] **Step 2: run it and confirm it fails for the intended reason**

```bash
bun run --filter @anakmobil/tokens test
```

Expected: FAIL with a module-resolution error naming `../src/derive.js` — *not* an assertion failure and not a syntax error in the test. If the failure is anything else, fix that first; a red test that is red for the wrong reason proves nothing.

- [ ] **Step 3: write the maths module**

Create `packages/tokens/src/derive.js`:

```js
/**
 * Values you compute from the tokens rather than store.
 *
 * Plain JavaScript with a hand-written `.d.ts`, matching `tokens.js` — Node
 * and Bun import it with no build step, and React Native imports it through
 * Metro unchanged.
 *
 * The contrast maths is the WCAG 2.x definition, not an approximation. It is
 * here rather than in a dependency because it is nine lines and because the
 * material system's whole contract is asserted against it: a library that
 * rounds differently would move a surface across the AA boundary silently.
 */

const HEX = /^#[0-9A-Fa-f]{6}$/;

/** Split #RRGGBB into three 0-255 channels. */
function channels(hex) {
  if (!HEX.test(hex)) throw new Error(`not a six-digit hex colour: "${hex}"`);
  return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
}

/** Reassemble three 0-255 channels into an uppercase #RRGGBB. */
function toHex(rgb) {
  return (
    "#" +
    rgb
      .map((v) =>
        Math.round(Math.min(255, Math.max(0, v)))
          .toString(16)
          .padStart(2, "0")
          .toUpperCase(),
      )
      .join("")
  );
}

/** The sRGB inverse transfer function, per channel. */
function linearize(value) {
  const s = value / 255;
  return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}

/**
 * WCAG relative luminance, 0 (black) to 1 (white).
 *
 * This is NOT the same scale as an opacity, and conflating the two is how a
 * scrim table ends up quoting 99.4% where the real alpha is 93.3%.
 */
export function relativeLuminance(hex) {
  const [r, g, b] = channels(hex).map(linearize);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG contrast ratio, 1 to 21. Symmetric in its arguments. */
export function contrastRatio(a, b) {
  const [high, low] = [relativeLuminance(a), relativeLuminance(b)].sort((x, y) => y - x);
  return (high + 0.05) / (low + 0.05);
}

/** AA is 4.5:1 for body text, 3:1 for large text (>=18.66px bold or >=24px). */
export function meetsAA(foreground, background, large = false) {
  return contrastRatio(foreground, background) >= (large ? 3 : 4.5);
}

/**
 * The colour a `tint` at `coverage` actually ends up being over `backdrop`.
 *
 * This is what the material system binds to. The coverage is a rendering
 * input; the returned colour is the contract.
 */
export function composite(tint, coverage, backdrop) {
  const t = channels(tint);
  const b = channels(backdrop);
  return toHex(t.map((c, i) => c * coverage + b[i] * (1 - coverage)));
}

/** `composite` with the arguments named for blending one colour into another. */
export function mix(base, other, weight) {
  return composite(other, weight, base);
}

/** An `rgba(r, g, b, a)` string, which React Native accepts anywhere a colour goes. */
export function withAlpha(hex, alpha) {
  const [r, g, b] = channels(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

const TYPE_SHORTHAND = /^(\d{3})\s+(\d+)px\/(\d+)px$/;

/**
 * Split a `"700 32px/38px"` type token into the three numbers a React Native
 * TextStyle wants. Throws rather than returning NaN: a NaN fontSize renders
 * nothing at all, which is far harder to trace than a build-time throw.
 */
export function parseTypeShorthand(value) {
  const match = TYPE_SHORTHAND.exec(value);
  if (!match) throw new Error(`unrecognised type shorthand: "${value}"`);
  return {
    fontWeight: Number(match[1]),
    fontSize: Number(match[2]),
    lineHeight: Number(match[3]),
  };
}
```

Create `packages/tokens/src/derive.d.ts`:

```ts
/**
 * Types for the derived-value helpers. Hand-written so `derive.js` can stay
 * plain JavaScript that Node, Bun, and Metro all import without a build step.
 *
 * Keep in sync with `derive.js`.
 */

export declare function relativeLuminance(hex: string): number;
export declare function contrastRatio(a: string, b: string): number;
export declare function meetsAA(foreground: string, background: string, large?: boolean): boolean;
export declare function composite(tint: string, coverage: number, backdrop: string): string;
export declare function mix(base: string, other: string, weight: number): string;
export declare function withAlpha(hex: string, alpha: number): string;

export interface ParsedType {
  readonly fontWeight: number;
  readonly fontSize: number;
  readonly lineHeight: number;
}

export declare function parseTypeShorthand(value: string): ParsedType;
```

- [ ] **Step 4: run the maths test to green**

```bash
bun run --filter @anakmobil/tokens test
```

Expected: the eight `derive` tests pass alongside the ten existing `tokens` tests — 18 pass, 0 fail.

- [ ] **Step 5: write the failing material-matrix test**

This is the test that makes the design's central claim checkable. Create `packages/tokens/test/material.test.mjs`:

```js
import assert from "node:assert/strict";
import test from "node:test";

import { composite, contrastRatio } from "../src/derive.js";
import { dark, ground, light, material, onAccent, semanticText } from "../src/tokens.js";

const THEMES = { light, dark };

/** The two backdrops a translucent surface can be worst-cased against. */
const EXTREMES = { white: "#FFFFFF", black: "#000000" };

test("every material role composites to the colour it claims", () => {
  for (const [themeName, roles] of Object.entries(material)) {
    const groundBase = ground[themeName].stops[0].color;
    for (const [roleName, role] of Object.entries(roles)) {
      assert.equal(
        role.solid,
        composite(role.tint, role.coverage, groundBase),
        `${themeName}.${roleName}.solid disagrees with its own tint and coverage`,
      );
    }
  }
});

test("working roles are solid — zero transparency, no exceptions", () => {
  // The rule that keeps a service record, a fitment result, and an AI safety
  // warning legible in direct sun at a workshop.
  assert.equal(material.light.working.coverage, 1);
  assert.equal(material.dark.working.coverage, 1);
});

test("secondary and tertiary text never sit on chrome", () => {
  // chrome is the one role whose backdrop is genuinely unknown.
  assert.deepEqual(material.light.chrome.allowsText, ["primary"]);
  assert.deepEqual(material.dark.chrome.allowsText, ["primary"]);
});

test("tertiary text never sits on a translucent role", () => {
  for (const roles of Object.values(material)) {
    for (const [name, role] of Object.entries(roles)) {
      if (role.coverage < 1) {
        assert.ok(
          !role.allowsText.includes("tertiary"),
          `${name} is translucent but claims to carry tertiary text`,
        );
      }
    }
  }
});

test("every material x allowed-text pair clears AA against both extremes", () => {
  const failures = [];
  for (const [themeName, roles] of Object.entries(material)) {
    const theme = THEMES[themeName];
    const text = {
      primary: theme.textPrimary,
      secondary: theme.textSecondary,
      tertiary: theme.textTertiary,
    };
    for (const [roleName, role] of Object.entries(roles)) {
      const backdrops =
        role.coverage === 1
          ? { solid: role.solid }
          : {
              ...Object.fromEntries(
                Object.entries(EXTREMES).map(([k, bd]) => [
                  k,
                  composite(role.tint, role.coverage, bd),
                ]),
              ),
              ground: role.solid,
            };
      for (const textRole of role.allowsText) {
        for (const [backdropName, backdrop] of Object.entries(backdrops)) {
          const ratio = contrastRatio(text[textRole], backdrop);
          if (ratio < 4.5) {
            failures.push(
              `${themeName}.${roleName} ${textRole} over ${backdropName} (${backdrop}) = ${ratio.toFixed(2)}`,
            );
          }
        }
      }
    }
  }
  assert.deepEqual(failures, [], `pairs below 4.5:1:\n${failures.join("\n")}`);
});

test("semantic text colours clear AA on their theme's working surface", () => {
  for (const [themeName, roles] of Object.entries(material)) {
    for (const [name, color] of Object.entries(semanticText[themeName])) {
      const ratio = contrastRatio(color, roles.working.solid);
      assert.ok(ratio >= 4.5, `${themeName} semanticText.${name} = ${ratio.toFixed(2)}`);
    }
  }
});

test("semantic text colours also clear AA on the subtle surface", () => {
  // Badges and toasts sit on surfaceSubtle as often as on the base surface.
  for (const [themeName, theme] of Object.entries(THEMES)) {
    for (const [name, color] of Object.entries(semanticText[themeName])) {
      const ratio = contrastRatio(color, theme.surfaceSubtle);
      assert.ok(
        ratio >= 4.5,
        `${themeName} semanticText.${name} on surfaceSubtle = ${ratio.toFixed(2)}`,
      );
    }
  }
});

test("the accent label clears AA on the accent fill", () => {
  // White on #ED491C is 3.77 and fails; graphite-950 is why onAccent exists.
  assert.ok(contrastRatio(onAccent, "#ED491C") >= 4.5);
});

test("tertiary text clears AA on every solid surface of its own theme", () => {
  for (const [themeName, theme] of Object.entries(THEMES)) {
    for (const surface of [theme.background, theme.surface, theme.surfaceSubtle, theme.surfaceRaised]) {
      const ratio = contrastRatio(theme.textTertiary, surface);
      assert.ok(ratio >= 4.5, `${themeName} tertiary on ${surface} = ${ratio.toFixed(2)}`);
    }
  }
});
```

- [ ] **Step 6: run it and confirm it fails for the intended reason**

```bash
bun run --filter @anakmobil/tokens test
```

Expected: FAIL on the import of `material`, `ground`, `onAccent`, and `semanticText` — they do not exist yet. Once they do, the last test (`tertiary text clears AA on every solid surface`) must still fail against the *current* `#737D87` / `#8A939D` values, which is the pre-existing defect this task repairs. Confirm you see that specific failure before repairing it — it is the proof the test has teeth.

- [ ] **Step 7: repair the two tertiary tokens**

In `packages/tokens/src/tokens.js`, change exactly two values and record why in place:

```js
export const light = {
  background: "#F7F8FA",
  surface: "#FFFFFF",
  surfaceSubtle: "#F1F3F5",
  surfaceRaised: "#FFFFFF",
  textPrimary: "#171C22",
  textSecondary: "#5D6670",
  // Repaired 2026-08-19 (AM-15). The documented #8A939D reached only 3.12:1
  // on white and 2.80:1 on surfaceSubtle — below the large-text floor, let
  // alone AA. #616A74 gives 5.49 and 4.94. docs/design.md §7/§66 updated.
  textTertiary: "#616A74",
  border: "#E3E6E9",
  borderStrong: "#CDD2D7",
};

export const dark = {
  background: "#0E1217",
  surface: "#151A20",
  surfaceSubtle: "#202730",
  surfaceRaised: "#1B2128",
  textPrimary: "#F5F7F8",
  textSecondary: "#AAB2BA",
  // Repaired 2026-08-19 (AM-15). The documented #737D87 reached 4.18:1 on
  // surface and 3.87:1 on surfaceRaised. #8E98A2 gives 5.97 and 5.53.
  textTertiary: "#8E98A2",
  border: "#29313A",
  borderStrong: "#3A434E",
};
```

- [ ] **Step 8: add the material, edge, ground, accent-label, and semantic-text groups**

Append to `packages/tokens/src/tokens.js`, after `motion`:

```js
/**
 * The label colour on an orange fill.
 *
 * docs/design.md §42 recommends white on #ED491C. That pair is 3.77:1 and
 * fails AA. Graphite-950 on the same fill is 4.91:1, so the brand orange is
 * kept exactly as §74/§76 specify and only the label changes.
 */
export const onAccent = "#0F141A";

/**
 * The label colour on a graphite fill — §42's DEFAULT primary button, which
 * is graphite rather than orange. White on graphite-800 is 15.84:1.
 */
export const onGraphite = "#FFFFFF";

/**
 * Semantic colours WHEN USED AS TEXT.
 *
 * The `semantic` group above stays what it is — fills, borders, and icons.
 * As text on their own theme's surfaces those values fail AA (success 3.99
 * on dark, warning 2.81 on white, danger 3.79 on dark, info 3.97 on dark),
 * so text gets its own pair per theme. A component that needs both uses
 * `semantic` for the border and icon and `semanticText` for the words, which
 * also satisfies §61's "do not communicate status by colour alone" for free.
 */
export const semanticText = {
  light: {
    success: "#137747",
    warning: "#8F5C00",
    danger: "#C22C2C",
    info: "#1F63B5",
  },
  dark: {
    success: "#1FA463",
    warning: "#D58A00",
    danger: "#EC6363", // plan said #E45B5B; corrected in execution — 4.26 on surfaceSubtle fails the test this task writes
    info: "#4A93E8",
  },
};

/**
 * The glass material — three roles per theme, distinguished by how much they
 * cover rather than by three arbitrary blur radii.
 *
 * `tint` and `coverage` are RENDERING inputs. `solid` is the contract: it is
 * the colour the role actually becomes over the app's own ground, and it is
 * what renders whenever transparency is unavailable or switched off — which
 * is every Android below SDK 31 and every device with Reduce Transparency on.
 *
 * `allowsText` is not documentation. A translucent role's backdrop is not
 * knowable, so only text that clears 4.5:1 against BOTH a white and a black
 * backdrop may sit on it. packages/tokens/test/material.test.mjs asserts it.
 *
 * `working` is solid on purpose: service history, fitment results, forms, AI
 * evidence, AI warnings, confidence badges, and the eight §73 signature
 * components are read to make a decision, outdoors, in direct sun. A surface
 * whose contrast varies with whatever is behind it is the wrong material for
 * a service record.
 *
 * Not emitted to CSS — see scripts/build.mjs. These are mobile-only.
 */
export const material = {
  light: {
    chrome: {
      tint: "#FBFCFD",
      coverage: 0.8,
      solid: "#FAFBFC",
      allowsText: ["primary"],
    },
    surface: {
      tint: "#FCFDFD",
      coverage: 0.92,
      solid: "#FCFDFD",
      allowsText: ["primary", "secondary"],
    },
    working: {
      tint: "#FFFFFF",
      coverage: 1,
      solid: "#FFFFFF",
      allowsText: ["primary", "secondary", "tertiary"],
    },
  },
  dark: {
    chrome: {
      tint: "#0E1217",
      coverage: 0.8,
      solid: "#0E1217",
      allowsText: ["primary"],
    },
    surface: {
      tint: "#151A20",
      coverage: 0.92,
      solid: "#14191F",
      allowsText: ["primary", "secondary"],
    },
    working: {
      tint: "#151A20",
      coverage: 1,
      solid: "#151A20",
      allowsText: ["primary", "secondary", "tertiary"],
    },
  },
};

/**
 * The edge — what actually gives glass its form.
 *
 * A 1px highlight on the TOP edge only, as if light caught the lip, plus an
 * inset shadow at the bottom for thickness: the instrument-panel read. A
 * uniform 1px white border on all four sides is forbidden by name; it is the
 * single most recognisable signature of templated glassmorphism.
 *
 * This is also what makes the design survive its own platform ladder — it
 * renders identically with or without blur.
 */
export const edge = {
  light: {
    highlight: "rgba(255, 255, 255, 0.90)",
    insetShadow: "inset 0 -8px 12px -8px rgba(15, 20, 26, 0.14)",
    borderWidth: 1,
    // The overlay behind a sheet. Dark in BOTH themes: it darkens whatever is
    // behind the sheet, and what is behind it is content, not the theme.
    scrim: "rgba(0, 0, 0, 0.45)",
  },
  dark: {
    highlight: "rgba(255, 255, 255, 0.14)",
    insetShadow: "inset 0 -8px 12px -8px rgba(0, 0, 0, 0.45)",
    borderWidth: 1,
    scrim: "rgba(0, 0, 0, 0.45)",
  },
};

/**
 * The ground — the bottom layer of the app.
 *
 * A graphite gradient, tinted with the dominant colour of the active vehicle
 * when there is one. Pure code: no image asset, no blur, no photograph. That
 * is what makes it identical on an iPhone and on an Android 10 phone.
 *
 * It is deliberately NOT the vehicle photograph. §47 requires "preserve
 * vehicle color" and "avoid excessive filters", and a heavily blurred,
 * scrimmed photo violates both — it also costs about 46 MiB per decoded
 * 4000x3000 image, approaching a gigabyte across twenty cached vehicles.
 *
 * `tintStrength` is how much of the vehicle colour reaches the middle stop.
 * WHERE THAT COLOUR COMES FROM IS NOT BUILT HERE — extraction is decided in
 * the garage epic, when vehicle photos actually exist. AmGround takes a tint
 * it is given and falls back to neutral graphite when there is none.
 */
export const ground = {
  light: {
    stops: [
      { color: "#FFFFFF", at: 0 },
      { color: "#F7F8FA", at: 0.45 },
      { color: "#EFF2F5", at: 1 },
    ],
    tintStrength: 0.08,
  },
  dark: {
    stops: [
      { color: "#151B22", at: 0 },
      { color: "#0E1217", at: 0.45 },
      { color: "#0B0E12", at: 1 },
    ],
    tintStrength: 0.14,
  },
};
```

Add the five new names to the `tokens` aggregate object at the bottom of the file:

```js
export const tokens = {
  brand,
  accent,
  semantic,
  onAccent,
  onGraphite,
  semanticText,
  light,
  dark,
  material,
  edge,
  ground,
  spacing,
  radius,
  typography,
  type,
  typeMobile,
  elevation,
  motion,
  layout,
};
```

- [ ] **Step 9: declare the new groups in `tokens.d.ts`**

Append to `packages/tokens/src/tokens.d.ts`, before the `tokens` aggregate declaration, and add the same five keys to that aggregate:

```ts
export type TextRole = "primary" | "secondary" | "tertiary";
export type MaterialRole = "chrome" | "surface" | "working";

export interface MaterialRecipe {
  /** The colour laid over the backdrop. A rendering input, never the contract. */
  readonly tint: string;
  /** 0-1. A rendering input. `1` means solid — zero transparency. */
  readonly coverage: number;
  /** The composited colour over the app's own ground. THIS is the contract. */
  readonly solid: string;
  /** Text roles proven to clear 4.5:1 on this material against any backdrop. */
  readonly allowsText: readonly TextRole[];
}

export interface EdgeTokens {
  readonly highlight: string;
  readonly insetShadow: string;
  readonly borderWidth: number;
  readonly scrim: string;
}

export interface GroundStop {
  readonly color: string;
  readonly at: number;
}

export interface GroundTokens {
  readonly stops: readonly GroundStop[];
  readonly tintStrength: number;
}

export interface SemanticTextColors {
  readonly success: string;
  readonly warning: string;
  readonly danger: string;
  readonly info: string;
}

export declare const onAccent: string;
export declare const onGraphite: string;
export declare const semanticText: Readonly<Record<"light" | "dark", SemanticTextColors>>;
export declare const material: Readonly<
  Record<"light" | "dark", Readonly<Record<MaterialRole, MaterialRecipe>>>
>;
export declare const edge: Readonly<Record<"light" | "dark", EdgeTokens>>;
export declare const ground: Readonly<Record<"light" | "dark", GroundTokens>>;
```

- [ ] **Step 10: expose `./derive` from the package**

In `packages/tokens/package.json`, add one entry to `exports` and one path to `files` is already covered (`src` is listed):

```jsonc
  "exports": {
    ".": {
      "types": "./src/tokens.d.ts",
      "default": "./src/tokens.js"
    },
    "./derive": {
      "types": "./src/derive.d.ts",
      "default": "./src/derive.js"
    },
    "./css": "./dist/tokens.css",
    "./theme": "./dist/theme.css"
  },
```

- [ ] **Step 11: record why the material is not emitted to CSS**

In `packages/tokens/scripts/build.mjs`, extend the module docstring — no code change:

```js
 * React Native needs neither: it imports `src/tokens.js` directly.
 *
 * The `material`, `edge`, and `ground` groups are deliberately NOT emitted
 * here. They are mobile-only (AM-15), and putting them into theme.css would
 * add custom properties to the backoffice's Tailwind theme that nothing
 * reads. When a web surface needs the material, emit it then.
```

- [ ] **Step 12: extend the existing token tests to cover the new groups**

In `packages/tokens/test/tokens.test.mjs`, widen the hex sweep and add one guard. Replace the first test and append the second:

```js
test("every colour is a six-digit uppercase hex", () => {
  const groups = {
    brand,
    accent,
    semantic,
    light,
    dark,
    "semanticText.light": semanticText.light,
    "semanticText.dark": semanticText.dark,
  };
  for (const [group, values] of Object.entries(groups)) {
    for (const [key, value] of Object.entries(values)) {
      assert.match(value, HEX, `${group}.${key} is "${value}"`);
    }
  }
  assert.match(onAccent, HEX, `onAccent is "${onAccent}"`);
});

test("the two material themes define exactly the same roles", () => {
  // A role in one theme and not the other renders as an undefined surface.
  assert.deepEqual(Object.keys(material.light).sort(), Object.keys(material.dark).sort());
  assert.deepEqual(Object.keys(edge.light).sort(), Object.keys(edge.dark).sort());
  assert.deepEqual(Object.keys(ground.light).sort(), Object.keys(ground.dark).sort());
});
```

Update that file's import line to add `edge, ground, material, onAccent, semanticText`.

- [ ] **Step 13: run the whole tokens gate to green**

```bash
bun run format
make ds-check
```

Expected: `EXIT=0`; `wrote dist/tokens.css and dist/theme.css`; every test passes. Record the pass count. Confirm `git diff packages/tokens/dist/` shows the two tertiary values changed and **nothing else** — no material variables leaked into the CSS.

- [ ] **Step 14: confirm the landing site still builds with the repaired tokens**

```bash
make fe-check
```

Expected: `EXIT=0`. The tertiary text on the landing page renders darker (light theme) — that is the repair, not a regression, and it goes in the commit message.

### Acceptance criteria

1. `packages/tokens/src/derive.js` exports all seven helpers, is plain ESM JavaScript with no dependency, and `derive.d.ts` declares each of them.
2. `packages/tokens/test/derive.test.mjs` passes, including the `#767676` / `#777777` AA boundary case and the `#808080` mid-grey luminance anchor.
3. `packages/tokens/test/material.test.mjs` passes, and **every** material role × allowed text role × backdrop pair clears 4.5:1. The test enumerates the failures rather than asserting a boolean, so a regression names the pair.
4. `dark.textTertiary` is `#8E98A2` and `light.textTertiary` is `#616A74`; the tertiary test passes against all four solid surfaces per theme. Reverting either value reddens the gate.
5. `material`, `edge` (including `scrim`), `ground`, `onAccent`, `onGraphite`, and `semanticText` exist in both `tokens.js` and `tokens.d.ts` and in the `tokens` aggregate. Adding a key to `tokens.js` without declaring it in `tokens.d.ts` is caught downstream by `apps/mobile`'s `tsc --noEmit`, which fails with "has no exported member" the moment mobile imports it — that is the real enforcement, not a test.
6. `packages/tokens/package.json` exposes `./derive`; `dist/tokens.css` and `dist/theme.css` contain **no** material, edge, or ground variables.
7. `make ds-check` and `make fe-check` are both `EXIT=0`.

**Block G and Block Q apply to this task.**

---

## Task 2: The mobile theme layer and the material capability resolver

Bring `@anakmobil/tokens` into `apps/mobile` for the first time, wrap it in a React-Native-shaped `Theme`, and write the resolver that decides which rung of the platform ladder this device is on. No visual component yet — this task's deliverable is the `useTheme()` and `useMaterialCapability()` contracts everything downstream consumes.

**Files:**
- Modify: `apps/mobile/package.json` — add `"@anakmobil/tokens": "*"`
- Regenerate: root `bun.lock`
- Create: `apps/mobile/src/theme/types.ts`
- Create: `apps/mobile/src/theme/theme.ts`
- Create: `apps/mobile/src/theme/ThemeProvider.tsx`
- Create: `apps/mobile/src/theme/capability.ts`
- Create: `apps/mobile/src/theme/index.ts`
- Modify: `apps/mobile/src/app/_layout.tsx` — wrap the tree in `ThemeProvider`

**Interfaces:**
- Consumes: everything Task 1 produced from `@anakmobil/tokens` and `@anakmobil/tokens/derive`.
- Produces: `useTheme(): Theme` · `useThemeControl(): { scheme, setScheme }` · `ThemeProvider` · `useMaterialCapability(): MaterialCapability` (`'liquid-glass' | 'tint'`) · `useCapabilityControl(): { forceTint, setForceTint }` · the exported types `Theme`, `MaterialRecipe`, `MaterialRole`, `TextRole`, `TypeName`, `MaterialCapability`. The `Theme.type` slot is declared here and **filled by Task 3**; until then it is typed and populated from the raw shorthand strings so `tsc` stays green.

**TDD: no** — verify by running. This is a context provider and a platform predicate; there is no input→output contract a red test would pin that `tsc` and the catalogue do not already. The one piece of arithmetic it touches (`composite`, `withAlpha`) is already tested in Task 1. Verified by: the import resolving at runtime on the simulator, and `useMaterialCapability()` reporting the expected rung on each platform.

**Minimality check.** Mobile consumes the shared package rather than getting its own token layer — reason and verification in Step 4. No state library: one `React.createContext` per concern. `useColorScheme()` from React Native supplies the system theme; the override exists solely because AM-15 AC2 requires checking both themes and toggling the device is not a check anyone repeats.

### Steps

- [ ] **Step 1: declare the dependency**

In `apps/mobile/package.json`, add to `dependencies`, keeping alphabetical order (it goes first, before `@expo/ui`):

```jsonc
  "dependencies": {
    "@anakmobil/tokens": "*",
    "@expo/ui": "~57.0.11",
    ...
  },
```

`"*"` is the workspace-protocol form `apps/landing` already uses; Bun resolves it to a symlink at `apps/mobile/node_modules/@anakmobil/tokens -> ../../../../packages/tokens`.

- [ ] **Step 2: link it and prove the lockfile stays stable**

From the repository root:

```bash
bun install
ls -la apps/mobile/node_modules/@anakmobil/
bun install --frozen-lockfile
git status --porcelain bun.lock
```

Expected: the `ls` shows a `tokens` symlink; the second install is `EXIT=0`; `git status` reports `bun.lock` **modified once by the first install and unchanged by the second**. No `package-lock.json` or `yarn.lock` appears under `apps/mobile/`.

- [ ] **Step 3: prove TypeScript resolves the package**

```bash
make mb-check
```

This cannot fail yet (nothing imports it), so it is a baseline, not the proof. The proof is Step 4.

- [ ] **Step 4: prove METRO resolves the package — the one assumption worth testing before building on it**

`packages/tokens` is `"type": "module"` with an `exports` map. Metro enables package-`exports` resolution by default on recent Expo SDKs, but that is the assumption this whole architecture rests on, so verify it before writing five files against it.

Create `apps/mobile/src/theme/index.ts` with a temporary probe:

```ts
export { material, spacing } from "@anakmobil/tokens";
export { contrastRatio } from "@anakmobil/tokens/derive";
```

Then, in `apps/mobile/src/app/index.tsx`, temporarily add above the existing render (remove before committing):

```tsx
import { contrastRatio, material } from "@/theme";
// Probe: proves Metro resolves the workspace package AND its ./derive entry.
console.log("tokens probe", material.dark.surface.solid, contrastRatio("#FFFFFF", "#000000"));
```

Start the app and read the Metro log:

```bash
make dev m=none
# then, in a second shell:
bun run --filter @anakmobil/mobile start
```

Expected in the Metro console: `tokens probe #14191F 21`.

**If it resolves — the common case — delete the probe and proceed.** Record the observed line in `## Execution status` as the evidence.

**If Metro cannot resolve `@anakmobil/tokens` or its `./derive` subpath**, do **not** duplicate any value. Instead: keep `apps/mobile/src/theme/index.ts` as a thin re-export module that pulls from the shared source by relative path (`../../../../packages/tokens/src/tokens.js`) and add `packages/tokens` to `metro.config.js`'s `watchFolders`, creating that config file from `expo/metro-config`'s default. Record the deviation in `## Execution status` and in the plan's ledger, because it changes item 6 of Block G for every later task.

- [ ] **Step 5: write the theme types**

Create `apps/mobile/src/theme/types.ts`:

```ts
import type {
  EdgeTokens,
  GroundTokens,
  MaterialRecipe,
  MaterialRole,
  SemanticTextColors,
  TextRole,
} from "@anakmobil/tokens";
import type { TextStyle } from "react-native";

export type { EdgeTokens, GroundTokens, MaterialRecipe, MaterialRole, TextRole };

export type ThemeName = "light" | "dark";

/** The mobile type scale, docs/design.md §11. Keys mirror `typeMobile`. */
export type TypeName =
  | "display"
  | "h1"
  | "h2"
  | "h3"
  | "title"
  | "body-lg"
  | "body"
  | "label"
  | "caption"
  | "micro";

export interface ThemeColors {
  readonly background: string;
  readonly surface: string;
  readonly surfaceSubtle: string;
  readonly surfaceRaised: string;
  readonly textPrimary: string;
  readonly textSecondary: string;
  readonly textTertiary: string;
  readonly border: string;
  readonly borderStrong: string;
  /** The brand accent. Never the material — content on it, or a solid fill. */
  readonly accent: string;
  /** The accent tuned for use AS TEXT in this theme. */
  readonly accentText: string;
  /** The label colour on an accent fill. White fails AA on #ED491C. */
  readonly onAccent: string;
  /** Graphite-800 — §42's DEFAULT primary button fill, theme-independent. */
  readonly graphite: string;
  /** The label on a graphite fill. 15.84:1, comfortably clear of AA. */
  readonly onGraphite: string;
  /** Fills, borders, icons. Never words — use `semanticText` for those. */
  readonly semantic: { readonly success: string; readonly warning: string; readonly danger: string; readonly info: string };
  readonly semanticText: SemanticTextColors;
}

export interface Theme {
  readonly name: ThemeName;
  readonly color: ThemeColors;
  readonly material: Readonly<Record<MaterialRole, MaterialRecipe>>;
  readonly edge: EdgeTokens;
  readonly ground: GroundTokens;
  readonly space: Readonly<Record<1 | 2 | 3 | 4 | 5 | 6 | 8 | 10 | 12 | 16 | 20 | 24, number>>;
  readonly radius: {
    readonly xs: number;
    readonly sm: number;
    readonly md: number;
    readonly lg: number;
    readonly xl: number;
    readonly "2xl": number;
    readonly pill: number;
  };
  readonly type: Readonly<Record<TypeName, TextStyle>>;
  readonly motion: {
    readonly micro: number;
    readonly standard: number;
    readonly sheet: number;
  };
  /** §61 / AM-15 AC3. A primitive enforces this itself; a caller never adds padding. */
  readonly touchTargetMin: number;
  readonly pagePadding: number;
}
```

- [ ] **Step 6: build the two themes**

Create `apps/mobile/src/theme/theme.ts`. The `type` slot is populated here from the raw shorthand via `parseTypeShorthand`; Task 3 replaces that call with the font-aware builder without changing the shape.

```ts
import {
  accent,
  brand,
  edge,
  ground,
  layout,
  material,
  motion,
  onAccent,
  onGraphite,
  radius,
  semantic,
  semanticText,
  spacing,
  typeMobile,
  dark as darkColors,
  light as lightColors,
} from "@anakmobil/tokens";
import { parseTypeShorthand } from "@anakmobil/tokens/derive";
import type { TextStyle } from "react-native";

import type { Theme, ThemeName, TypeName } from "./types";

/** `"210ms"` -> `210`. Reanimated and Animated both want a number. */
function ms(value: string): number {
  return Number.parseInt(value, 10);
}

/**
 * Turn the shared `typeMobile` shorthands into React Native text styles.
 *
 * Task 3 adds `fontFamily` and the 650 -> 600 weight mapping here. Until
 * then the styles carry weight, size, and line height only, which renders
 * correctly in the system font.
 */
function buildTypeScale(): Record<TypeName, TextStyle> {
  const entries = Object.entries(typeMobile).map(([name, shorthand]) => {
    const { fontWeight, fontSize, lineHeight } = parseTypeShorthand(shorthand);
    return [name, { fontSize, lineHeight, fontWeight: String(fontWeight) } as TextStyle];
  });
  return Object.fromEntries(entries) as Record<TypeName, TextStyle>;
}

const typeScale = buildTypeScale();

function build(name: ThemeName): Theme {
  const colors = name === "dark" ? darkColors : lightColors;
  return {
    name,
    color: {
      ...colors,
      // Orange as TEXT: #ED491C is 4.64 on the dark surface but only 3.77 on
      // white, so the light theme steps down to accent-700 (5.27).
      accent: accent[500],
      accentText: name === "dark" ? accent[500] : accent[700],
      onAccent,
      // §42's default primary button is graphite, not orange — orange is the
      // "strongest brand CTA", used selectively.
      graphite: brand[800],
      onGraphite,
      semantic,
      semanticText: semanticText[name],
    },
    material: material[name],
    edge: edge[name],
    ground: ground[name],
    space: spacing,
    radius,
    type: typeScale,
    motion: {
      micro: ms(motion.durationMicro),
      standard: ms(motion.durationStandard),
      sheet: ms(motion.durationSheet),
    },
    touchTargetMin: layout.touchTargetMin,
    pagePadding: layout.pagePaddingMobile,
  };
}

export const lightTheme = build("light");
export const darkTheme = build("dark");
export const themes: Record<ThemeName, Theme> = { light: lightTheme, dark: darkTheme };
```

- [ ] **Step 7: write the provider**

Create `apps/mobile/src/theme/ThemeProvider.tsx`:

```tsx
import { createContext, useContext, useMemo, useState, type ReactNode } from "react";
import { useColorScheme } from "react-native";

import { themes } from "./theme";
import type { Theme, ThemeName } from "./types";

/** `undefined` means "follow the device", which is the default. */
type SchemeOverride = ThemeName | undefined;

interface ThemeControl {
  readonly scheme: SchemeOverride;
  readonly setScheme: (scheme: SchemeOverride) => void;
  readonly resolved: ThemeName;
}

const ThemeContext = createContext<Theme | null>(null);
const ThemeControlContext = createContext<ThemeControl | null>(null);

export interface ThemeProviderProps {
  readonly children: ReactNode;
  /** Force a theme regardless of the device. Used by the component catalogue. */
  readonly initialScheme?: ThemeName;
}

export function ThemeProvider({ children, initialScheme }: ThemeProviderProps) {
  const system = useColorScheme();
  const [scheme, setScheme] = useState<SchemeOverride>(initialScheme);
  const resolved: ThemeName = scheme ?? (system === "dark" ? "dark" : "light");

  const theme = themes[resolved];
  const control = useMemo<ThemeControl>(
    () => ({ scheme, setScheme, resolved }),
    [scheme, resolved],
  );

  return (
    <ThemeControlContext.Provider value={control}>
      <ThemeContext.Provider value={theme}>{children}</ThemeContext.Provider>
    </ThemeControlContext.Provider>
  );
}

export function useTheme(): Theme {
  const theme = useContext(ThemeContext);
  if (!theme) throw new Error("useTheme must be used inside <ThemeProvider>");
  return theme;
}

export function useThemeControl(): ThemeControl {
  const control = useContext(ThemeControlContext);
  if (!control) throw new Error("useThemeControl must be used inside <ThemeProvider>");
  return control;
}
```

- [ ] **Step 8: write the capability resolver**

Create `apps/mobile/src/theme/capability.ts`. This is the platform ladder as code.

```ts
import { isGlassEffectAPIAvailable, isLiquidGlassAvailable } from "expo-glass-effect";
import { createContext, useContext, useEffect, useState } from "react";
import { AccessibilityInfo } from "react-native";

/**
 * Which rung of the platform ladder this device is on.
 *
 *   'liquid-glass'  iOS 26+, native Liquid Glass through expo-glass-effect
 *   'tint'          everything else: tint + edge, no blur
 *
 * There is no third rung, and that is deliberate. expo-blur on Android
 * defaults to blurMethod: 'none', which renders a semi-transparent tint and
 * is not a blur; a real blur needs Android 12+ and costs a dependency for a
 * surface this ticket does not build. So 'tint' IS the design, not a
 * degradation — the ground is a gradient and the edge does the shaping, both
 * of which render identically with or without blur.
 */
export type MaterialCapability = "liquid-glass" | "tint";

interface CapabilityControl {
  /** The catalogue's "force no blur" switch — the Android < 31 reality on demand. */
  readonly forceTint: boolean;
  readonly setForceTint: (force: boolean) => void;
}

export const CapabilityControlContext = createContext<CapabilityControl | null>(null);

export function useCapabilityControl(): CapabilityControl {
  // Deliberately tolerant: outside the catalogue there is no control, and a
  // primitive must not crash because nobody offered it a switch.
  return useContext(CapabilityControlContext) ?? { forceTint: false, setForceTint: () => {} };
}

export function useMaterialCapability(): MaterialCapability {
  const { forceTint } = useCapabilityControl();
  const [reduceTransparency, setReduceTransparency] = useState(false);

  useEffect(() => {
    let alive = true;
    // iOS-only setting; resolves false on Android, which is the right answer
    // there because Android has no system-wide transparency switch.
    AccessibilityInfo.isReduceTransparencyEnabled()
      .then((value) => {
        if (alive) setReduceTransparency(value);
      })
      .catch(() => {
        if (alive) setReduceTransparency(false);
      });
    const sub = AccessibilityInfo.addEventListener("reduceTransparencyChanged", (value) => {
      setReduceTransparency(value);
    });
    return () => {
      alive = false;
      sub.remove();
    };
  }, []);

  if (forceTint || reduceTransparency) return "tint";
  // Both predicates: isLiquidGlassAvailable() reports the design is active,
  // isGlassEffectAPIAvailable() guards the iOS 26 betas that ship the design
  // without the API and crash on GlassView (expo/expo#40911).
  return isLiquidGlassAvailable() && isGlassEffectAPIAvailable() ? "liquid-glass" : "tint";
}
```

- [ ] **Step 9: write the barrel and wire the provider into the layout**

Replace the probe in `apps/mobile/src/theme/index.ts`:

```ts
export { ThemeProvider, useTheme, useThemeControl } from "./ThemeProvider";
export { CapabilityControlContext, useCapabilityControl, useMaterialCapability } from "./capability";
export { darkTheme, lightTheme, themes } from "./theme";
export type * from "./types";
export type { MaterialCapability } from "./capability";
```

Update `apps/mobile/src/app/_layout.tsx`:

```tsx
import { Stack } from "expo-router";

import { ThemeProvider } from "@/theme";

// The one route file Expo Router requires. A bare Stack, no navigation
// structure — tabs, nested stacks, and real routes are their own later story.
// ThemeProvider sits above it so every screen and every primitive resolves
// the same theme; AmGround joins in Task 4 and ToastProvider in Task 7.
export default function RootLayout() {
  return (
    <ThemeProvider>
      <Stack screenOptions={{ headerShown: false }} />
    </ThemeProvider>
  );
}
```

- [ ] **Step 10: remove the probe and run the gate**

```bash
git diff apps/mobile/src/app/index.tsx   # must show no probe left behind
bun run format
make mb-check
```

Expected: `EXIT=0` from `mb-check`, and `git diff` on `index.tsx` shows nothing (the probe is gone).

### Acceptance criteria

1. `apps/mobile/package.json` declares `"@anakmobil/tokens": "*"`; `apps/mobile/node_modules/@anakmobil/tokens` is a symlink to `packages/tokens`; `bun install --frozen-lockfile` is `EXIT=0` and leaves `bun.lock` unchanged.
2. Metro resolves both `@anakmobil/tokens` and `@anakmobil/tokens/derive` at runtime — evidenced by the recorded probe line `tokens probe #14191F 21`, or by the recorded `metro.config.js` deviation if it did not.
3. `useTheme()` returns a `Theme` with all eleven slots populated; calling it outside the provider throws a named error rather than returning `null`.
4. `useThemeControl().setScheme('dark' | 'light' | undefined)` switches the theme without a remount, and `undefined` restores the device setting.
5. `useMaterialCapability()` returns `'tint'` whenever `forceTint` is set **or** Reduce Transparency is on, and `'liquid-glass'` only when both `expo-glass-effect` predicates are true. It updates live when Reduce Transparency is toggled in Settings.
6. No hex value, font size, spacing number, or duration is written as a literal anywhere under `apps/mobile/src/theme/` — every value traces to `@anakmobil/tokens`.
7. `make mb-check` is `EXIT=0` and the probe is fully removed.

**Block G and Block Q apply to this task.**

---

## Task 3: Typography — Inter, the mobile scale, and tabular figures

Load Inter, map the mobile type scale onto React Native text styles, and make automotive numerals line up. Small, but it must land before any primitive is styled: every component built against a different font has different metrics, and re-checking contrast and touch targets after a font swap is the whole verification pass run twice.

**Files:**
- Modify: `apps/mobile/package.json` — add the Inter font package
- Regenerate: root `bun.lock`
- Create: `apps/mobile/src/theme/fonts.ts`
- Create: `apps/mobile/src/theme/typography.ts`
- Modify: `apps/mobile/src/theme/theme.ts` — `buildTypeScale` becomes font-aware
- Modify: `apps/mobile/src/theme/index.ts` — export `useAppFonts`, `numeric`
- Modify: `apps/mobile/src/app/_layout.tsx` — hold the splash screen until fonts load

**Interfaces:**
- Consumes: Task 2's `Theme`, `TypeName`, and `buildTypeScale`.
- Produces: `useAppFonts(): boolean` (`true` once fonts are ready or have failed) · `numeric: TextStyle` (the tabular-figure style for spec data) · a `Theme.type` whose every entry now carries `fontFamily`.

**TDD: no** — verify by running. Font loading has no assertable contract in this app (there is no test runner here by design, Block G item 9); the check is that the catalogue renders in Inter and that `146,120 KM` above `225/40 R18` has its digits in a column. `parseTypeShorthand`, the only logic involved, is already tested in Task 1.

**Minimality check.** `@expo-google-fonts/inter` rather than hand-vendored TTFs: it is the SDK-blessed path, `expo-font` is already installed, and vendoring means committing four binaries and a licence. **Four cuts only** — 400, 500, 600, 700 — not the ten the family ships; each unused cut is bundle weight for nothing. No `expo-font` config plugin and no native font linking: `useFonts` at runtime is enough for a dev-client build and avoids a prebuild step.

### Steps

- [ ] **Step 1: install the font package and see what it actually ships**

```bash
# Corrected in execution: `bun add` has no --filter flag (it tried resolving
# @anakmobil/mobile from the npm registry, 404). --cwd is the equivalent.
bun add --cwd apps/mobile @expo-google-fonts/inter
ls apps/mobile/node_modules/@expo-google-fonts/inter | head -30
```

Expected: the package resolves and exposes per-weight entry points (`Inter_400Regular`, `Inter_500Medium`, `Inter_600SemiBold`, `Inter_700Bold`). Record what is actually there — if the package's shape differs from this, adapt the import in Step 2 to what it exposes rather than to what this plan assumed, and note the difference in `## Execution status`.

Then prove the lockfile is stable:

```bash
bun install --frozen-lockfile && git status --porcelain bun.lock
```

Expected: `EXIT=0`, `bun.lock` unchanged by the second run.

- [ ] **Step 2: write the font loader**

Create `apps/mobile/src/theme/fonts.ts`:

```ts
import {
  Inter_400Regular,
  Inter_500Medium,
  Inter_600SemiBold,
  Inter_700Bold,
  useFonts,
} from "@expo-google-fonts/inter";

/**
 * The four Inter cuts the mobile scale actually uses.
 *
 * docs/design.md §11 lists weights 400, 500, 600, 650, and 700 for mobile.
 * 650 has no static cut and React Native's `fontWeight` does not accept it
 * (the type union is '100'..'900' and 100..900 — 650 is not a member), so
 * the two 650 steps, H3 and Title, render at 600. Recorded in
 * docs/design.md §11 rather than left as a silent substitution. The desktop
 * scale keeps 650 and 750 because the web ships the variable cut.
 */
export const FONT_FAMILY = {
  400: "Inter_400Regular",
  500: "Inter_500Medium",
  600: "Inter_600SemiBold",
  700: "Inter_700Bold",
} as const;

export type FontWeightKey = keyof typeof FONT_FAMILY;

/**
 * Map a design weight onto a cut that exists. 650 -> 600 is the only
 * substitution and it is the one documented above.
 */
export function resolveWeight(weight: number): FontWeightKey {
  if (weight >= 700) return 700;
  if (weight >= 600) return 600;
  if (weight >= 500) return 500;
  return 400;
}

/** `true` once the fonts are ready — or have failed, in which case the system font stands in. */
export function useAppFonts(): boolean {
  const [loaded, error] = useFonts({
    Inter_400Regular,
    Inter_500Medium,
    Inter_600SemiBold,
    Inter_700Bold,
  });
  // A font that will not load must not hold the app at a blank splash screen
  // forever. The system font is a worse look, not a broken one.
  return loaded || error !== null;
}
```

- [ ] **Step 3: write the typography helpers**

Create `apps/mobile/src/theme/typography.ts`:

```ts
import { typeMobile } from "@anakmobil/tokens";
import { parseTypeShorthand } from "@anakmobil/tokens/derive";
import type { TextStyle } from "react-native";

import { FONT_FAMILY, resolveWeight } from "./fonts";
import type { TypeName } from "./types";

/**
 * Automotive spec data — `18x8.5 ET40`, `225/40 R18`, `146,120 KM`,
 * `Rp 14.500.000` (docs/design.md §12). Tabular figures make a column of
 * these line up; without them a list of mileages is visually ragged and
 * genuinely harder to scan.
 *
 * Spread onto a Text style, never used alone: it carries no size or weight.
 */
export const numeric: TextStyle = { fontVariant: ["tabular-nums"] };

/**
 * The mobile type scale as React Native text styles.
 *
 * `fontFamily` carries the weight because that is how a static-cut font
 * family works on both platforms; `fontWeight` is set alongside it so that
 * the system font still renders at roughly the right weight if Inter fails
 * to load.
 */
export function buildTypeScale(): Record<TypeName, TextStyle> {
  const entries = Object.entries(typeMobile).map(([name, shorthand]) => {
    const { fontWeight, fontSize, lineHeight } = parseTypeShorthand(shorthand);
    const cut = resolveWeight(fontWeight);
    const style: TextStyle = {
      fontFamily: FONT_FAMILY[cut],
      fontWeight: String(cut) as TextStyle["fontWeight"],
      fontSize,
      lineHeight,
    };
    return [name, style];
  });
  return Object.fromEntries(entries) as Record<TypeName, TextStyle>;
}
```

- [ ] **Step 4: point the theme at it**

In `apps/mobile/src/theme/theme.ts`, delete the local `buildTypeScale` function and its now-unused `parseTypeShorthand` / `typeMobile` / `TextStyle` imports, and import the real one:

```ts
import { buildTypeScale } from "./typography";

const typeScale = buildTypeScale();
```

Everything else in the file is unchanged — the `Theme.type` slot's shape did not move.

- [ ] **Step 5: hold the splash screen until the fonts are ready**

Update `apps/mobile/src/app/_layout.tsx`:

```tsx
import { Stack } from "expo-router";
import * as SplashScreen from "expo-splash-screen";
import { useEffect } from "react";

import { ThemeProvider, useAppFonts } from "@/theme";

// Keep the splash screen up rather than flashing the system font for a frame
// and then reflowing every line when Inter arrives.
void SplashScreen.preventAutoHideAsync();

export default function RootLayout() {
  const fontsReady = useAppFonts();

  useEffect(() => {
    if (fontsReady) void SplashScreen.hideAsync();
  }, [fontsReady]);

  if (!fontsReady) return null;

  return (
    <ThemeProvider>
      <Stack screenOptions={{ headerShown: false }} />
    </ThemeProvider>
  );
}
```

- [ ] **Step 6: export the new pieces**

Add to `apps/mobile/src/theme/index.ts`:

```ts
export { FONT_FAMILY, resolveWeight, useAppFonts } from "./fonts";
export { buildTypeScale, numeric } from "./typography";
```

- [ ] **Step 7: run the gate and look at it**

```bash
bun run format
make mb-check
make dev m=ios
```

Expected: `mb-check` is `EXIT=0`; the app opens past the splash screen with the healthcheck screen rendering in Inter rather than the system font. (The healthcheck screen still uses its own inline styles — Task 8 leaves it alone apart from one link.)

### Acceptance criteria

1. `@expo-google-fonts/inter` is a dependency; exactly four cuts are registered (400, 500, 600, 700); `bun install --frozen-lockfile` is `EXIT=0` with `bun.lock` unchanged.
2. `useAppFonts()` returns `true` on success **and** on failure, and the splash screen hides in both cases — a font that will not load never leaves the app on a blank screen.
3. Every entry of `theme.type` carries a `fontFamily` from `FONT_FAMILY`; the two 650 steps (`h3`, `title`) resolve to `Inter_600SemiBold`, and the substitution is written down in `fonts.ts` and, in Task 10, in `docs/design.md` §11.
4. `numeric` applies `fontVariant: ['tabular-nums']` and nothing else, so it composes onto any scale step.
5. `parseTypeShorthand` is called exactly once per scale step, at module load, not per render.
6. `make mb-check` is `EXIT=0` and the app renders in Inter on the iOS simulator.

**Block G and Block Q apply to this task.**

---

## Task 4: `AmGround` and `AmMaterial` — the ground, the three roles, and the edge

The material system itself, and the choke point every primitive renders through. Both components are pure code: a gradient with no image asset, an edge that renders identically with and without blur, and a ladder that upgrades to Liquid Glass on iOS 26 without ever depending on it.

**Files:**
- Create: `apps/mobile/src/components/material/AmGround.tsx`
- Create: `apps/mobile/src/components/material/AmMaterial.tsx`
- Create: `apps/mobile/src/components/material/index.ts`
- Modify: `apps/mobile/src/app/_layout.tsx` — the ground wraps the stack

**Interfaces:**
- Consumes: Task 2's `useTheme`, `useMaterialCapability`; Task 1's `withAlpha`, `mix`.
- Produces: `<AmGround tint?: string>` · `<AmMaterial role: MaterialRole, radius?: keyof Theme['radius'], edge?: boolean, style?, children>` · `useMaterialTextColor(role, textRole): string` — a helper that returns the right text token for a role **and throws in development if the role does not allow that text role**, so a component cannot silently put tertiary text on chrome.

**TDD: no** — verify by running. This is rendering: gradients, borders, and a shadow. Its one factual claim — that each role composites to a colour passing AA — was pinned by Task 1's test against the token values these components consume. The remaining verification is visual and belongs to Task 8's catalogue, which is exactly where the no-blur review happens.

**Minimality check.** **No `expo-linear-gradient`**: RN 0.86's `experimental_backgroundImage` takes a CSS `linear-gradient` string, which is rung 4 of the ladder (native platform feature) against rung 5 (a new dependency). **No `expo-blur`**: it would add a dependency whose Android default is not a blur, for a `chrome` surface this ticket does not build — the app bar and tab bar belong to the app-shell story, and that story can add it when it has a surface to put it on. **No grain.** The spec describes the ground as "a gradient plus fine grain"; grain has no asset-free implementation in React Native short of a shader, and the anti-goals forbid an image asset. The gradient plus the edge ships; the grain is deferred with a `ponytail:` note rather than faked with stacked radial gradients that would cost exactly the Android performance the ladder protects.

### Steps

- [ ] **Step 1: write the ground**

Create `apps/mobile/src/components/material/AmGround.tsx`:

```tsx
import { mix } from "@anakmobil/tokens/derive";
import type { ReactNode } from "react";
import { StyleSheet, View, type ViewStyle } from "react-native";

import { useTheme } from "@/theme";

export interface AmGroundProps {
  readonly children: ReactNode;
  /**
   * The dominant colour of the active vehicle, when there is one. A red car
   * gives a copper-tinted garage; a white car a cool one.
   *
   * WHERE THIS COMES FROM IS NOT BUILT HERE. Extraction is decided in the
   * garage epic, when vehicle photos actually exist. Absent, the ground falls
   * back to neutral graphite, which is the launch state.
   */
  readonly tint?: string;
  readonly style?: ViewStyle;
}

/**
 * The bottom layer of the app: a graphite gradient, optionally tinted.
 *
 * Pure code — a gradient, no image asset, no blur, no photograph. That is
 * what makes it render identically on an iPhone and on an Android 10 phone,
 * and it is why the no-blur rendering of everything above it is a legitimate
 * variant rather than a degraded one.
 *
 * ponytail: the spec describes the ground as "a gradient plus fine grain".
 * The grain is deliberately not implemented — React Native has no asset-free
 * noise, and the anti-goals forbid an image asset. Add it with a shader if a
 * device check ever says the flat gradient bands.
 */
export function AmGround({ children, tint, style }: AmGroundProps) {
  const theme = useTheme();
  const { stops, tintStrength } = theme.ground;

  // Only the middle stop takes the vehicle colour. Tinting the ends washes
  // the whole screen and stops it reading as graphite.
  const middle = Math.floor(stops.length / 2);
  const gradient = stops
    .map((stop, index) => {
      const color = tint && index === middle ? mix(stop.color, tint, tintStrength) : stop.color;
      return `${color} ${Math.round(stop.at * 100)}%`;
    })
    .join(", ");

  return (
    <View
      style={[
        styles.fill,
        { experimental_backgroundImage: `linear-gradient(180deg, ${gradient})` },
        // A flat fallback in the same family, so a platform that ignores the
        // gradient shows graphite rather than white.
        { backgroundColor: stops[middle].color },
        style,
      ]}
    >
      {children}
    </View>
  );
}

const styles = StyleSheet.create({
  fill: { flex: 1 },
});
```

- [ ] **Step 2: write the material**

Create `apps/mobile/src/components/material/AmMaterial.tsx`:

```tsx
import { withAlpha } from "@anakmobil/tokens/derive";
import { GlassView } from "expo-glass-effect";
import type { ReactNode } from "react";
import { StyleSheet, View, type ViewStyle } from "react-native";

import { useMaterialCapability, useTheme, type MaterialRole, type TextRole, type Theme } from "@/theme";

export interface AmMaterialProps {
  readonly role: MaterialRole;
  readonly children: ReactNode;
  readonly radius?: keyof Theme["radius"];
  /** The glass edge. Off for a flush surface inside another material. */
  readonly edge?: boolean;
  readonly style?: ViewStyle;
}

/**
 * The one place a surface is drawn.
 *
 * Three roles, distinguished by how much they cover:
 *
 *   chrome    app bar, tab bar, floating AI entry — the most glass
 *   surface   content cards, sheets, list panels — reads as milk-glass
 *   working   data, forms, AI evidence and warnings — SOLID, always
 *
 * `working` is never translucent, on any platform, under any capability. It
 * is the material for everything read to make a decision, and those screens
 * are used outdoors at a workshop in direct sun. It also keeps §46's border
 * rather than the glass edge — "use borders before shadows" is superseded
 * for chrome and surface and survives intact here.
 */
export function AmMaterial({ role, children, radius = "lg", edge = true, style }: AmMaterialProps) {
  const theme = useTheme();
  const capability = useMaterialCapability();
  const recipe = theme.material[role];
  const borderRadius = theme.radius[radius];

  const solid = recipe.coverage === 1;
  const glass = !solid && capability === "liquid-glass";

  const shell: ViewStyle = {
    borderRadius,
    overflow: "hidden",
    // The edge, and the reason the design survives its own platform ladder:
    // a 1px highlight on the TOP edge only, plus a bottom inset shadow for
    // thickness. NEVER a uniform border on all four sides.
    ...(edge && !solid
      ? {
          borderTopWidth: theme.edge.borderWidth,
          borderTopColor: theme.edge.highlight,
          boxShadow: theme.edge.insetShadow,
        }
      : {}),
    ...(solid && edge
      ? { borderWidth: theme.edge.borderWidth, borderColor: theme.color.border }
      : {}),
  };

  if (glass) {
    return (
      <GlassView
        glassEffectStyle="regular"
        tintColor={withAlpha(recipe.tint, recipe.coverage)}
        colorScheme={theme.name}
        style={[shell, style]}
      >
        {children}
      </GlassView>
    );
  }

  // The tint rung: every Android below SDK 31, every iOS below 26, and every
  // device with Reduce Transparency on. `solid` is the composited colour that
  // already passes AA, so this path is not a fallback — it is the contract.
  return <View style={[shell, { backgroundColor: recipe.solid }, style]}>{children}</View>;
}

const TEXT_TOKEN = {
  primary: "textPrimary",
  secondary: "textSecondary",
  tertiary: "textTertiary",
} as const;

/**
 * The text colour a role is allowed to carry.
 *
 * Throws in development when a caller asks for a text role the material
 * cannot hold — tertiary on chrome, say — because that combination does not
 * fail visibly, it fails at 2.9:1 in bright sun on somebody's phone.
 */
export function useMaterialTextColor(role: MaterialRole, textRole: TextRole): string {
  const theme = useTheme();
  const recipe = theme.material[role];
  if (__DEV__ && !recipe.allowsText.includes(textRole)) {
    throw new Error(
      `${theme.name}.${role} cannot carry ${textRole} text — it allows ${recipe.allowsText.join(", ")}`,
    );
  }
  return theme.color[TEXT_TOKEN[textRole]];
}
```

`StyleSheet` is imported only if a static style is actually needed; as written above it is not, so drop it from the import line rather than leaving an unused symbol for `expo lint` to flag.

- [ ] **Step 3: write the barrel and put the ground under the app**

Create `apps/mobile/src/components/material/index.ts`:

```ts
export { AmGround } from "./AmGround";
export type { AmGroundProps } from "./AmGround";
export { AmMaterial, useMaterialTextColor } from "./AmMaterial";
export type { AmMaterialProps } from "./AmMaterial";
```

Update `apps/mobile/src/app/_layout.tsx` so the ground is the bottom layer and the stack renders transparently over it:

```tsx
    <ThemeProvider>
      <AmGround>
        <Stack
          screenOptions={{
            headerShown: false,
            contentStyle: { backgroundColor: "transparent" },
          }}
        />
      </AmGround>
    </ThemeProvider>
```

with `import { AmGround } from "@/components/material";` added.

- [ ] **Step 4: run the gate and look at both rungs**

```bash
bun run format
make mb-check
make dev m=ios
```

Expected: `mb-check` is `EXIT=0`; the healthcheck screen now sits on a graphite gradient rather than a flat background. On an iOS 26 simulator the capability resolves to `liquid-glass`; on an older one it resolves to `tint` — check both if two simulators are available, and record which was observed. The full role-by-role visual review is Task 8's job.

### Acceptance criteria

1. `AmGround` renders a three-stop `linear-gradient` through `experimental_backgroundImage`, with a flat `backgroundColor` fallback in the same colour family. It imports no gradient library and no image.
2. `AmGround` with no `tint` renders neutral graphite; with a `tint` it mixes that colour into the **middle stop only**, at the theme's `tintStrength`. Nothing in this ticket supplies a tint.
3. `AmMaterial` with `role="working"` renders a solid `View` at `recipe.solid` **on every capability**, including `liquid-glass` — there is no code path that makes `working` translucent.
4. The edge is `borderTopWidth` plus `boxShadow` with `inset`, never `borderWidth` on all four sides. `grep -n "borderWidth" apps/mobile/src/components/material/AmMaterial.tsx` shows it only on the solid `working` branch, which is §46's card border and not the glass edge.
5. `useMaterialTextColor('chrome', 'secondary')` throws in development with a message naming the allowed roles; `useMaterialTextColor('working', 'tertiary')` returns the theme's tertiary token.
6. `GlassView` is reached only when `useMaterialCapability()` returns `'liquid-glass'`; turning on Reduce Transparency switches every surface to the `tint` path live, with no reload.
7. No `expo-blur`, `expo-linear-gradient`, or `@gorhom/bottom-sheet` appears in `apps/mobile/package.json`.
8. `make mb-check` is `EXIT=0`.

**Block G and Block Q apply to this task.**

---
## Task 5: Input primitives — `AmButton`, `AmTextField`, `AmSelect` (AM-26)

Every state, every touch target ≥ 44 pt without the caller adding padding, and input text large enough to read. Runs concurrently with Tasks 6 and 7 — the three file sets are disjoint and none imports another.

**Files:**
- Create: `apps/mobile/src/components/input/AmButton.tsx`
- Create: `apps/mobile/src/components/input/AmTextField.tsx`
- Create: `apps/mobile/src/components/input/AmSelect.tsx`
- Create: `apps/mobile/src/components/input/index.ts`

**Interfaces:**
- Consumes: `useTheme` (Task 2), `numeric` (Task 3), `AmMaterial` (Task 4), and — for `AmSelect` only — `AmBottomSheet` from Task 6. See Step 4 for how that cross-group edge is handled without serialising the two tasks.
- Produces: `<AmButton variant='primary'|'accent'|'secondary'|'ghost'|'destructive' size='sm'|'md'|'lg' loading? disabled? onPress label>` · `<AmTextField label value onChangeText error? hint? disabled? secureTextEntry? keyboardType?>` · `<AmSelect<T> label value options onChange placeholder?>` where `options: readonly { value: T; label: string }[]`.

**TDD: no** — verify by running. These are styled pressables and a text input; the assertions worth making are visual (does the disabled state read as disabled?) and dimensional (is the hit area 44 pt?), both of which the catalogue answers directly and a unit test would answer by restating the same constants.

**Minimality check.** No form library, no `react-hook-form`: `AmTextField` is controlled and owns nothing but its focus state. `AmSelect` does not implement a picker — it opens `AmBottomSheet`, which is exactly what §45 asks for and what AM-27 requires ("Sheet dipakai untuk seluruh picker dan filter, bukan dialog bawaan sistem"). No `Pressable` wrapper abstraction: RN's `Pressable` already gives the pressed state.

### Steps

- [ ] **Step 1: write `AmButton`**

Create `apps/mobile/src/components/input/AmButton.tsx`:

```tsx
import { ActivityIndicator, Pressable, StyleSheet, Text, View, type ViewStyle } from "react-native";

import { useTheme } from "@/theme";

export type AmButtonVariant = "primary" | "accent" | "secondary" | "ghost" | "destructive";
export type AmButtonSize = "sm" | "md" | "lg";

export interface AmButtonProps {
  readonly label: string;
  readonly onPress: () => void;
  readonly variant?: AmButtonVariant;
  readonly size?: AmButtonSize;
  readonly disabled?: boolean;
  readonly loading?: boolean;
  readonly style?: ViewStyle;
}

/** docs/design.md §43. `md` is 44 — the accessibility floor is also a size. */
const HEIGHT: Record<AmButtonSize, number> = { sm: 36, md: 44, lg: 52 };

export function AmButton({
  label,
  onPress,
  variant = "primary",
  size = "md",
  disabled = false,
  loading = false,
  style,
}: AmButtonProps) {
  const theme = useTheme();
  const inert = disabled || loading;

  // Solid = pressable, glass = container. A button is never the material.
  const fills: Record<AmButtonVariant, { background: string; label: string; border?: string }> = {
    // §42's default: graphite, not orange. White on graphite-800 is 15.84:1.
    primary: { background: theme.color.graphite, label: theme.color.onGraphite },
    // §42's "strongest brand CTA", used selectively. White on #ED491C is
    // 3.77 and fails AA, so the label is onAccent (graphite-950) at 4.91.
    accent: { background: theme.color.accent, label: theme.color.onAccent },
    secondary: {
      background: "transparent",
      label: theme.color.textPrimary,
      border: theme.color.borderStrong,
    },
    ghost: { background: "transparent", label: theme.color.accentText },
    // §42: destructive uses semantic danger, never orange.
    destructive: { background: theme.color.semantic.danger, label: theme.color.onGraphite },
  };
  const fill = fills[variant];

  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={{ disabled: inert, busy: loading }}
      accessibilityLabel={label}
      disabled={inert}
      onPress={onPress}
      style={({ pressed }) => [
        styles.base,
        {
          minHeight: Math.max(HEIGHT[size], theme.touchTargetMin),
          minWidth: theme.touchTargetMin,
          paddingHorizontal: theme.space[5],
          borderRadius: theme.radius.md,
          backgroundColor: fill.background,
          borderWidth: fill.border ? 1 : 0,
          borderColor: fill.border,
          // No glow: §50 bans constant glowing orange effects, and the first
          // mockup of this design violated it. Press is opacity only.
          opacity: inert ? 0.45 : pressed ? 0.82 : 1,
        },
        style,
      ]}
    >
      {loading ? (
        <View style={[styles.row, { gap: theme.space[2] }]}>
          <ActivityIndicator color={fill.label} size="small" />
          <Text style={[theme.type.label, { color: fill.label }]}>{label}</Text>
        </View>
      ) : (
        <Text style={[theme.type.label, { color: fill.label }]} numberOfLines={1}>
          {label}
        </Text>
      )}
    </Pressable>
  );
}

const styles = StyleSheet.create({
  base: { alignItems: "center", justifyContent: "center" },
  // No `gap` here: a spacing value belongs to the theme, and StyleSheet.create
  // runs outside the hook. Layout-only keys stay; design values go inline.
  row: { flexDirection: "row", alignItems: "center" },
});
```

`theme.color.graphite` and `theme.color.onGraphite` come from Task 2's `ThemeColors` and Task 1's `brand[800]` / `onGraphite` — no literal is introduced here. If either is missing, that is a Task 1 or Task 2 defect to raise, not a reason to inline a hex.

**Two of the five variants are deliberately not orange, and that is §42 rather than caution.** `primary` is graphite because §42 names graphite as the default and orange as the *strongest* CTA, "used selectively"; `destructive` is semantic danger because §42 says so explicitly, and using orange for a delete would make both meanings unreadable. Only `accent` is orange, and only its label changed.

- [ ] **Step 2: write `AmTextField`**

Create `apps/mobile/src/components/input/AmTextField.tsx`:

```tsx
import { useState } from "react";
import { StyleSheet, Text, TextInput, View, type KeyboardTypeOptions, type ViewStyle } from "react-native";

import { useTheme } from "@/theme";

export interface AmTextFieldProps {
  readonly label: string;
  readonly value: string;
  readonly onChangeText: (value: string) => void;
  readonly placeholder?: string;
  readonly hint?: string;
  readonly error?: string;
  readonly disabled?: boolean;
  readonly secureTextEntry?: boolean;
  readonly keyboardType?: KeyboardTypeOptions;
  readonly style?: ViewStyle;
}

/**
 * docs/design.md §44: height 48-52, radius 12, neutral border, and the label
 * is ALWAYS visible — structured automotive data must never rely on a
 * placeholder that vanishes the moment someone types.
 *
 * The input text is `body-lg` (16px). AM-26's definition of done frames this
 * as stopping iOS auto-zoom; a native React Native app has no auto-zoom (that
 * is mobile Safari behaviour), so here 16 is a legibility floor rather than a
 * zoom guard. The number is the same either way.
 */
export function AmTextField({
  label,
  value,
  onChangeText,
  placeholder,
  hint,
  error,
  disabled = false,
  secureTextEntry,
  keyboardType,
  style,
}: AmTextFieldProps) {
  const theme = useTheme();
  const [focused, setFocused] = useState(false);

  const borderColor = error
    ? theme.color.semantic.danger
    : focused
      ? theme.color.accent
      : theme.color.border;

  return (
    <View style={[{ gap: theme.space[2] }, style]}>
      <Text style={[theme.type.label, { color: theme.color.textSecondary }]}>{label}</Text>
      <TextInput
        accessibilityLabel={label}
        accessibilityHint={hint}
        editable={!disabled}
        value={value}
        onChangeText={onChangeText}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        placeholder={placeholder}
        placeholderTextColor={theme.color.textTertiary}
        secureTextEntry={secureTextEntry}
        keyboardType={keyboardType}
        style={[
          theme.type["body-lg"],
          styles.input,
          {
            minHeight: 52,
            paddingHorizontal: theme.space[4],
            borderRadius: theme.radius.md,
            borderWidth: 1,
            borderColor,
            // A form is `working`: solid, read to make a decision.
            backgroundColor: theme.material.working.solid,
            color: theme.color.textPrimary,
            opacity: disabled ? 0.5 : 1,
          },
        ]}
      />
      {error ? (
        <Text style={[theme.type.caption, { color: theme.color.semanticText.danger }]}>{error}</Text>
      ) : hint ? (
        <Text style={[theme.type.caption, { color: theme.color.textTertiary }]}>{hint}</Text>
      ) : null}
    </View>
  );
}

const styles = StyleSheet.create({
  input: { textAlignVertical: "center" },
});
```

- [ ] **Step 3: write `AmSelect`**

Create `apps/mobile/src/components/input/AmSelect.tsx`:

```tsx
import { useState } from "react";
import { Pressable, StyleSheet, Text, View, type ViewStyle } from "react-native";

import { AmBottomSheet } from "@/components/display";
import { useTheme } from "@/theme";

export interface AmSelectOption<T extends string> {
  readonly value: T;
  readonly label: string;
}

export interface AmSelectProps<T extends string> {
  readonly label: string;
  readonly value: T | null;
  readonly options: readonly AmSelectOption<T>[];
  readonly onChange: (value: T) => void;
  readonly placeholder?: string;
  readonly disabled?: boolean;
  readonly style?: ViewStyle;
}

/**
 * A select opens a bottom sheet, never a native picker or dialog.
 *
 * §45 lists the bottom-sheet picker as the pattern for vehicle specs, and
 * AM-27's definition of done makes it a requirement: every picker and filter
 * in the app goes through AmBottomSheet.
 */
export function AmSelect<T extends string>({
  label,
  value,
  options,
  onChange,
  placeholder = "Pilih",
  disabled = false,
  style,
}: AmSelectProps<T>) {
  const theme = useTheme();
  const [open, setOpen] = useState(false);
  const selected = options.find((option) => option.value === value);

  return (
    <View style={[{ gap: theme.space[2] }, style]}>
      <Text style={[theme.type.label, { color: theme.color.textSecondary }]}>{label}</Text>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={label}
        accessibilityValue={{ text: selected?.label ?? placeholder }}
        accessibilityState={{ disabled, expanded: open }}
        disabled={disabled}
        onPress={() => setOpen(true)}
        style={({ pressed }) => [
          styles.trigger,
          {
            minHeight: 52,
            paddingHorizontal: theme.space[4],
            borderRadius: theme.radius.md,
            borderWidth: 1,
            borderColor: theme.color.border,
            backgroundColor: theme.material.working.solid,
            opacity: disabled ? 0.5 : pressed ? 0.82 : 1,
          },
        ]}
      >
        <Text
          style={[
            theme.type["body-lg"],
            { color: selected ? theme.color.textPrimary : theme.color.textTertiary },
          ]}
          numberOfLines={1}
        >
          {selected?.label ?? placeholder}
        </Text>
      </Pressable>

      <AmBottomSheet visible={open} onClose={() => setOpen(false)} title={label}>
        {options.map((option) => (
          <Pressable
            key={option.value}
            accessibilityRole="radio"
            accessibilityState={{ selected: option.value === value }}
            onPress={() => {
              onChange(option.value);
              setOpen(false);
            }}
            style={({ pressed }) => [
              styles.option,
              {
                minHeight: theme.touchTargetMin,
                paddingHorizontal: theme.space[4],
                borderRadius: theme.radius.sm,
                backgroundColor: pressed ? theme.color.surfaceSubtle : "transparent",
              },
            ]}
          >
            <Text
              style={[
                theme.type["body-lg"],
                {
                  color:
                    option.value === value ? theme.color.accentText : theme.color.textPrimary,
                },
              ]}
            >
              {option.label}
            </Text>
          </Pressable>
        ))}
      </AmBottomSheet>
    </View>
  );
}

const styles = StyleSheet.create({
  trigger: { justifyContent: "center" },
  option: { justifyContent: "center" },
});
```

- [ ] **Step 4: handle the one cross-group dependency**

`AmSelect` imports `AmBottomSheet` from Task 6. That is the only edge between the three parallel primitive tasks, and it does **not** justify serialising them. Two rules make it safe:

1. Task 6's brief carries `AmBottomSheet`'s exact signature (below), and so does this one. Both writers code against the same declaration, not against each other's file.
2. If Task 6 has not landed when this task reaches Step 3, write `AmSelect` against the signature anyway and let `tsc` fail on the missing module until Task 6 lands. Do **not** write a placeholder sheet — a second sheet implementation is precisely the duplication this ordering exists to avoid.

The frozen signature, identical in both briefs:

```ts
export interface AmBottomSheetProps {
  readonly visible: boolean;
  readonly onClose: () => void;
  readonly title: string;
  readonly children: React.ReactNode;
}
export function AmBottomSheet(props: AmBottomSheetProps): React.JSX.Element | null;
```

- [ ] **Step 5: write the barrel and run the gate**

Create `apps/mobile/src/components/input/index.ts`:

```ts
export { AmButton } from "./AmButton";
export type { AmButtonProps, AmButtonSize, AmButtonVariant } from "./AmButton";
export { AmTextField } from "./AmTextField";
export type { AmTextFieldProps } from "./AmTextField";
export { AmSelect } from "./AmSelect";
export type { AmSelectOption, AmSelectProps } from "./AmSelect";
```

```bash
bun run format
make mb-check
```

Expected: `EXIT=0` once Task 6 has landed. Until then the only permitted failure is `Cannot find module '@/components/display'`.

### Acceptance criteria

1. `AmButton` renders all five variants and all four states (normal, pressed, disabled, loading), and its `minHeight` is never below `theme.touchTargetMin` even at `size="sm"` — a 36 pt visual sits in a 44 pt target with no padding from the caller.
2. The accent button's label is `theme.color.onAccent`, not white. `contrastRatio(onAccent, accent[500])` is 4.91; white would be 3.77.
3. No glow, no shadow, and no orange background on any variant other than `accent`. Press feedback is opacity only.
4. `AmTextField` shows the label at all times, renders normal / focused / error / disabled distinctly, sets the input text to `theme.type['body-lg']` (16 px), and draws on `material.working.solid`.
5. `AmTextField`'s error message uses `semanticText.danger`, never the raw `semantic.danger` — the raw value fails AA as text on the dark surface (3.79).
6. `AmSelect` opens `AmBottomSheet` and never a native picker, `Alert`, or `ActionSheetIOS`. The selected option is marked by a trailing check glyph **and** colour **and** `accessibilityState.selected` — never colour alone (§61 / WCAG 1.4.1; wording corrected in execution, ledger #20).
7. Every option row is ≥ 44 pt tall.
8. No hex string, font size, spacing, or radius literal survives as a STYLE VALUE anywhere in `apps/mobile/src/components/input/` — verify with `grep -nE "#[0-9A-Fa-f]{6}|fontSize: [0-9]|padding[A-Za-z]*: [0-9]" apps/mobile/src/components/input/`; hits inside comments are exempt (the plan's own documentation of the contrast math lives in a comment — wording corrected in execution, ledger #22).
9. `make mb-check` is `EXIT=0`.

**Block G and Block Q apply to this task.**

---

## Task 6: Display primitives — `AmCard`, `AmChip`, `AmBadge`, `AmAvatar`, `AmBottomSheet` (AM-27)

The components repeated across garage, build, community, and Explore. The sheet is the one with real behaviour: it must close by gesture as well as by button, and it must not be a native dialog. Runs concurrently with Tasks 5 and 7.

**Files:**
- Create: `apps/mobile/src/components/display/AmCard.tsx`
- Create: `apps/mobile/src/components/display/AmChip.tsx`
- Create: `apps/mobile/src/components/display/AmBadge.tsx`
- Create: `apps/mobile/src/components/display/AmAvatar.tsx`
- Create: `apps/mobile/src/components/display/AmBottomSheet.tsx`
- Create: `apps/mobile/src/components/display/index.ts`

**Interfaces:**
- Consumes: `useTheme` (Task 2), `AmMaterial` (Task 4), `expo-image`, `react-native-reanimated`, `react-native-gesture-handler`.
- Produces: `<AmCard role?='surface'|'working' padding?>` · `<AmChip label selected? onPress?>` · `<AmBadge tone='success'|'warning'|'danger'|'info'|'neutral' label icon?>` · `<AmAvatar name uri? size?>` · `<AmBottomSheet visible onClose title>` — signature frozen in Task 5 Step 4.

**TDD: no** — verify by running. Presentational, plus one gesture. The sheet's dismissal behaviour is verified by using it on the catalogue screen; a test that drove the pan gesture would test Reanimated, not this code.

**Minimality check.** No `@gorhom/bottom-sheet`: RN's `Modal` plus the already-installed Reanimated and Gesture Handler is the smaller path, and the sheet needs one snap point, not a snap-point engine. `AmCard` is a thin wrapper over `AmMaterial` and adds only padding — it exists because §46 defines a card's padding and radius, not because it needs behaviour. `AmBadge` uses a neutral fill: a wall of saturated pills is slop, and it also means no white-on-semantic pair has to clear AA.

### Steps

- [ ] **Step 1: write `AmCard`**

Create `apps/mobile/src/components/display/AmCard.tsx`:

```tsx
import type { ReactNode } from "react";
import type { ViewStyle } from "react-native";

import { AmMaterial } from "@/components/material";
import { useTheme, type Theme } from "@/theme";

export interface AmCardProps {
  readonly children: ReactNode;
  /**
   * `surface` is the default card (docs/design.md §46, revised). `working`
   * is for anything read to make a decision — service history, fitment
   * results, AI evidence, AI warnings — and is solid.
   */
  readonly role?: "surface" | "working";
  readonly padding?: keyof Theme["space"];
  readonly radius?: keyof Theme["radius"];
  readonly style?: ViewStyle;
}

export function AmCard({
  children,
  role = "surface",
  padding = 4,
  radius = "lg",
  style,
}: AmCardProps) {
  const theme = useTheme();
  return (
    <AmMaterial role={role} radius={radius} style={[{ padding: theme.space[padding] }, style]}>
      {children}
    </AmMaterial>
  );
}
```

- [ ] **Step 2: write `AmChip`**

Create `apps/mobile/src/components/display/AmChip.tsx`:

```tsx
import { Pressable, StyleSheet, Text, type ViewStyle } from "react-native";

import { useTheme } from "@/theme";

export interface AmChipProps {
  readonly label: string;
  readonly selected?: boolean;
  readonly onPress?: () => void;
  readonly disabled?: boolean;
  readonly style?: ViewStyle;
}

/**
 * §45's fast-selection control: Manual / Automatic / CVT / DCT, or
 * Daily / Track / Stance / Touring / Show.
 *
 * A chip is visually short but its hit area is not — §61 and AM-15 AC3 want
 * 44 pt, and hitSlop supplies the difference so a row of chips still looks
 * like a row of chips.
 */
export function AmChip({ label, selected = false, onPress, disabled = false, style }: AmChipProps) {
  const theme = useTheme();
  const pad = (theme.touchTargetMin - 32) / 2;

  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={{ selected, disabled }}
      accessibilityLabel={label}
      disabled={disabled || !onPress}
      onPress={onPress}
      hitSlop={{ top: pad, bottom: pad, left: 0, right: 0 }}
      style={({ pressed }) => [
        styles.chip,
        {
          minHeight: 32,
          paddingHorizontal: theme.space[3],
          borderRadius: theme.radius.pill,
          borderWidth: 1,
          // Selected is orange: §17 and AM-15 AC4 both name the selected
          // state as one of orange's four legitimate uses.
          borderColor: selected ? theme.color.accent : theme.color.border,
          backgroundColor: selected ? theme.color.accent : theme.color.surfaceSubtle,
          opacity: disabled ? 0.45 : pressed ? 0.82 : 1,
        },
        style,
      ]}
    >
      <Text
        style={[
          theme.type.label,
          { color: selected ? theme.color.onAccent : theme.color.textPrimary },
        ]}
        numberOfLines={1}
      >
        {label}
      </Text>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  chip: { alignItems: "center", justifyContent: "center" },
});
```

- [ ] **Step 3: write `AmBadge`**

Create `apps/mobile/src/components/display/AmBadge.tsx`:

```tsx
import { StyleSheet, Text, View, type ViewStyle } from "react-native";

import { useTheme } from "@/theme";

export type AmBadgeTone = "success" | "warning" | "danger" | "info" | "neutral";

export interface AmBadgeProps {
  readonly label: string;
  readonly tone?: AmBadgeTone;
  /** A short glyph carried alongside the colour. §61: never colour alone. */
  readonly icon?: string;
  readonly style?: ViewStyle;
}

/**
 * A status marker on a NEUTRAL fill with a semantic border, icon, and text.
 *
 * Not a saturated pill. Two reasons, and both are rules rather than taste:
 * §61 forbids communicating status by colour alone, so the label and the
 * glyph have to carry the meaning anyway; and the raw semantic values fail
 * AA as text on their own surfaces, which is why `semanticText` exists.
 *
 * A badge is always solid — never glass. Confidence badges specifically are
 * `working` because a semantic colour shifts in perception on a variable
 * backdrop, and a confidence signal that shifts is a confidence signal that
 * lies. AmConfidenceBadge itself belongs to the AI epic, on top of this.
 */
export function AmBadge({ label, tone = "neutral", icon, style }: AmBadgeProps) {
  const theme = useTheme();
  const border = tone === "neutral" ? theme.color.borderStrong : theme.color.semantic[tone];
  const color = tone === "neutral" ? theme.color.textSecondary : theme.color.semanticText[tone];

  return (
    <View
      accessible
      accessibilityRole="text"
      accessibilityLabel={label}
      style={[
        styles.badge,
        {
          paddingHorizontal: theme.space[2],
          paddingVertical: theme.space[1],
          gap: theme.space[1],
          borderRadius: theme.radius.pill,
          borderWidth: 1,
          borderColor: border,
          backgroundColor: theme.material.working.solid,
        },
        style,
      ]}
    >
      {icon ? <Text style={[theme.type.micro, { color }]}>{icon}</Text> : null}
      <Text style={[theme.type.micro, { color }]} numberOfLines={1}>
        {label}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  badge: { flexDirection: "row", alignItems: "center", alignSelf: "flex-start" },
});
```

- [ ] **Step 4: write `AmAvatar`**

Create `apps/mobile/src/components/display/AmAvatar.tsx`:

```tsx
import { Image } from "expo-image";
import { StyleSheet, Text, View, type ViewStyle } from "react-native";

import { useTheme } from "@/theme";

export interface AmAvatarProps {
  readonly name: string;
  readonly uri?: string;
  readonly size?: number;
  readonly style?: ViewStyle;
}

/** Up to two letters, which is what reads at 40 pt. */
function initials(name: string): string {
  return name
    .trim()
    .split(/\s+/)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("");
}

/**
 * §47's 1:1 community avatar. Solid, never glass — an avatar over a variable
 * backdrop stops being a recognisable face.
 *
 * With no photo it falls back to initials on a neutral fill rather than a
 * stock silhouette of a person, matching §48's reasoning for vehicles: a
 * placeholder must not imply something that is not true.
 */
export function AmAvatar({ name, uri, size = 40, style }: AmAvatarProps) {
  const theme = useTheme();
  const shape: ViewStyle = {
    width: size,
    height: size,
    borderRadius: theme.radius.pill,
    borderWidth: 1,
    borderColor: theme.color.border,
    backgroundColor: theme.color.surfaceSubtle,
  };

  if (uri) {
    return (
      <Image
        source={{ uri }}
        accessibilityLabel={name}
        contentFit="cover"
        style={[shape, style]}
      />
    );
  }

  return (
    <View accessibilityRole="image" accessibilityLabel={name} style={[styles.fallback, shape, style]}>
      <Text style={[theme.type.label, { color: theme.color.textSecondary }]}>{initials(name)}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  fallback: { alignItems: "center", justifyContent: "center" },
});
```

- [ ] **Step 5: write `AmBottomSheet`**

Create `apps/mobile/src/components/display/AmBottomSheet.tsx`. The signature is frozen — Task 5 codes against it.

```tsx
import type { ReactNode } from "react";
import { useEffect } from "react";
import { Modal, Pressable, StyleSheet, Text, View } from "react-native";
import { Gesture, GestureDetector, GestureHandlerRootView } from "react-native-gesture-handler";
import Animated, {
  runOnJS,
  useAnimatedStyle,
  useSharedValue,
  withTiming,
} from "react-native-reanimated";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmMaterial } from "@/components/material";
import { useTheme } from "@/theme";

export interface AmBottomSheetProps {
  readonly visible: boolean;
  readonly onClose: () => void;
  readonly title: string;
  readonly children: ReactNode;
}

/** Past this many points of downward drag, releasing dismisses. */
const DISMISS_AT = 96;

/**
 * The sheet every picker and filter in the app goes through.
 *
 * `Modal` is used only as a transparent host for the overlay — the sheet
 * itself is our own view, so this is not the "native dialog" §45 and AM-27
 * rule out. Closable by dragging down and by the button, both required.
 *
 * Its material is `surface`: a sheet is a container, and containers are
 * glass. Anything inside it that is read to make a decision uses `working`.
 */
export function AmBottomSheet({ visible, onClose, title, children }: AmBottomSheetProps) {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const translateY = useSharedValue(0);

  useEffect(() => {
    if (visible) translateY.value = 0;
  }, [visible, translateY]);

  const pan = Gesture.Pan()
    .onChange((event) => {
      // Downward only — dragging up must not detach the sheet from the edge.
      translateY.value = Math.max(0, translateY.value + event.changeY);
    })
    .onEnd(() => {
      if (translateY.value > DISMISS_AT) {
        translateY.value = withTiming(600, { duration: theme.motion.sheet }, () => {
          runOnJS(onClose)();
        });
      } else {
        translateY.value = withTiming(0, { duration: theme.motion.standard });
      }
    });

  const sheetStyle = useAnimatedStyle(() => ({ transform: [{ translateY: translateY.value }] }));

  if (!visible) return null;

  return (
    <Modal visible transparent statusBarTranslucent animationType="slide" onRequestClose={onClose}>
      <GestureHandlerRootView style={styles.host}>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Tutup"
          style={[styles.scrim, { backgroundColor: theme.edge.scrim }]}
          onPress={onClose}
        />
        <GestureDetector gesture={pan}>
          <Animated.View style={sheetStyle}>
            <AmMaterial
              role="surface"
              radius="2xl"
              style={{
                paddingHorizontal: theme.pagePadding,
                paddingTop: theme.space[3],
                paddingBottom: insets.bottom + theme.space[5],
                gap: theme.space[4],
              }}
            >
              <View
                accessibilityElementsHidden
                importantForAccessibility="no"
                style={[styles.grabber, { backgroundColor: theme.color.borderStrong }]}
              />
              <View style={styles.header}>
                <Text style={[theme.type.h3, { color: theme.color.textPrimary }]}>{title}</Text>
                <Pressable
                  accessibilityRole="button"
                  accessibilityLabel="Tutup"
                  onPress={onClose}
                  hitSlop={12}
                  style={{ minWidth: theme.touchTargetMin, minHeight: theme.touchTargetMin, alignItems: "flex-end", justifyContent: "center" }}
                >
                  <Text style={[theme.type.label, { color: theme.color.accentText }]}>Tutup</Text>
                </Pressable>
              </View>
              {children}
            </AmMaterial>
          </Animated.View>
        </GestureDetector>
      </GestureHandlerRootView>
    </Modal>
  );
}

const styles = StyleSheet.create({
  host: { flex: 1, justifyContent: "flex-end" },
  scrim: StyleSheet.absoluteFillObject,
  grabber: { alignSelf: "center", width: 36, height: 4, borderRadius: 2 },
  header: { flexDirection: "row", alignItems: "center", justifyContent: "space-between" },
});
```

The scrim colour comes from `theme.edge.scrim`, defined in Task 1. It is dark in **both** themes on purpose: a scrim darkens whatever is behind the sheet, and what is behind it is content, not the theme. A light scrim in light mode would leave the sheet floating on an unchanged page and destroy the modality.

`animationType="slide"` handles the entry; the pan gesture handles the exit past the threshold and `withTiming` handles the snap back. Do not add a second entry animation on top of the `Modal`'s — two animations on the same transform is how a sheet ends up arriving twice.

- [ ] **Step 6: write the barrel and run the gate**

Create `apps/mobile/src/components/display/index.ts`:

```ts
export { AmCard } from "./AmCard";
export type { AmCardProps } from "./AmCard";
export { AmChip } from "./AmChip";
export type { AmChipProps } from "./AmChip";
export { AmBadge } from "./AmBadge";
export type { AmBadgeProps, AmBadgeTone } from "./AmBadge";
export { AmAvatar } from "./AmAvatar";
export type { AmAvatarProps } from "./AmAvatar";
export { AmBottomSheet } from "./AmBottomSheet";
export type { AmBottomSheetProps } from "./AmBottomSheet";
```

```bash
bun run format
make ds-check   # the scrim token
make mb-check
```

Expected: both `EXIT=0`.

### Acceptance criteria

1. `AmCard` renders through `AmMaterial` and adds only padding and radius; `role="working"` produces a solid card with §46's border, `role="surface"` produces the glass edge.
2. `AmChip` has a ≥ 44 pt effective hit area (32 pt visual plus symmetric `hitSlop`) and its selected state is orange with an `onAccent` label, matching AM-15 AC4's list of legitimate orange uses.
3. `AmBadge` never uses a saturated fill; it draws a neutral `working` fill with a semantic border and `semanticText` label, and carries a glyph as well as a colour so §61 is satisfied without the caller doing anything.
4. `AmAvatar` falls back to at most two initials on a neutral fill when `uri` is absent — no stock photograph, no generic person silhouette.
5. `AmBottomSheet` closes both by dragging down past 96 pt and by the "Tutup" button, and tapping the scrim also closes it. It uses `Modal` purely as a transparent host; no `Alert`, `ActionSheetIOS`, or platform picker appears anywhere in `apps/mobile/src/components/`.
6. The sheet's material is `surface`, its corner radius is `2xl` (28), and its bottom padding respects the safe-area inset.
7. `AmBottomSheetProps` matches the frozen signature in Task 5 Step 4 exactly.
8. `grep -nE "#[0-9A-Fa-f]{6}|rgba\(" apps/mobile/src/components/display/` returns nothing — the scrim moved to the tokens.
9. `make ds-check` and `make mb-check` are both `EXIT=0`.

**Block G and Block Q apply to this task.**

---
## Task 7: State primitives — `AmEmptyState`, `AmErrorState`, `AmSkeleton`, `AmToast` (AM-28)

The four components every list in the app will reach for. `AmEmptyState` always carries exactly one action — that is a product rule, not a default. Runs concurrently with Tasks 5 and 6.

**Files:**
- Create: `apps/mobile/src/components/state/AmEmptyState.tsx`
- Create: `apps/mobile/src/components/state/AmErrorState.tsx`
- Create: `apps/mobile/src/components/state/AmSkeleton.tsx`
- Create: `apps/mobile/src/components/state/AmToast.tsx`
- Create: `apps/mobile/src/components/state/index.ts`
- Modify: `apps/mobile/src/app/_layout.tsx` — add `ToastProvider` inside `AmGround`

**Interfaces:**
- Consumes: `useTheme` (Task 2), `AmMaterial` (Task 4), `AmButton` (Task 5 — same frozen-signature rule as Task 5 Step 4), `react-native-reanimated`.
- Produces: `<AmEmptyState title body actionLabel onAction>` · `<AmErrorState title body onRetry retryLabel?>` · `<AmSkeleton width? height? radius? role?>` · `<ToastProvider>` + `useToast(): (toast: { message: string; tone?: AmBadgeTone }) => void`.

**TDD: no** — verify by running. Presentational plus one timer-driven queue of at most one item. The single behavioural claim worth checking — a toast auto-dismisses and does not leak its timer — is verified by opening the catalogue and by the unmount cleanup being visible in the diff.

**Minimality check.** The toast queue is **one toast at a time**, replaced rather than stacked: a stack needs an ordering policy, an exit-animation choreography, and a maximum, none of which anything in this app has asked for. `AmSkeleton` pulses opacity rather than sweeping a gradient — a shimmer needs a moving mask, and the spec's own rule ("skeletons are drawn on the same material as the component they stand in for") is about not shimmering against a backdrop that swallows it. `AmErrorState` is not `AmEmptyState` with a different colour: §52 and §53 have genuinely different tone and structure, and collapsing them would force a `variant` prop that immediately branches on everything.

### Steps

- [ ] **Step 1: write `AmEmptyState`**

Create `apps/mobile/src/components/state/AmEmptyState.tsx`:

```tsx
import { StyleSheet, Text, View, type ViewStyle } from "react-native";

import { AmButton } from "@/components/input";
import { useTheme } from "@/theme";

export interface AmEmptyStateProps {
  readonly title: string;
  readonly body: string;
  /** Required, not optional. See the note below — this is a product rule. */
  readonly actionLabel: string;
  readonly onAction: () => void;
  readonly style?: ViewStyle;
}

/**
 * §52: an empty state encourages meaningful contribution.
 *
 * `actionLabel` and `onAction` are REQUIRED, and that is deliberate. AM-28's
 * definition of done says the empty state always carries one action rather
 * than a sentence, and the platform launches with no data at all — the
 * low-data state is designed as a primary experience, not a fallback. Making
 * the action optional would let the most-seen screen in the product ship as
 * a dead end.
 */
export function AmEmptyState({ title, body, actionLabel, onAction, style }: AmEmptyStateProps) {
  const theme = useTheme();
  return (
    <View style={[styles.container, { padding: theme.space[6], gap: theme.space[3] }, style]}>
      <Text style={[theme.type.h3, styles.centered, { color: theme.color.textPrimary }]}>
        {title}
      </Text>
      <Text style={[theme.type.body, styles.centered, { color: theme.color.textSecondary }]}>
        {body}
      </Text>
      <AmButton
        label={actionLabel}
        onPress={onAction}
        variant="accent"
        style={{ marginTop: theme.space[2] }}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  container: { alignItems: "center", justifyContent: "center" },
  centered: { textAlign: "center" },
});
```

- [ ] **Step 2: write `AmErrorState`**

Create `apps/mobile/src/components/state/AmErrorState.tsx`:

```tsx
import { StyleSheet, Text, View, type ViewStyle } from "react-native";

import { AmButton } from "@/components/input";
import { useTheme } from "@/theme";

export interface AmErrorStateProps {
  readonly title: string;
  readonly body: string;
  readonly onRetry: () => void;
  readonly retryLabel?: string;
  readonly style?: ViewStyle;
}

/**
 * §53: direct, useful, non-technical. "Something went wrong" is named in the
 * document as the bad version; the good version says what failed, reassures
 * about the data, and offers the retry.
 *
 * The tone marker is `semanticText.danger`, never the raw `semantic.danger`,
 * which is 3.79:1 as text on the dark surface.
 */
export function AmErrorState({
  title,
  body,
  onRetry,
  retryLabel = "Coba lagi",
  style,
}: AmErrorStateProps) {
  const theme = useTheme();
  return (
    <View style={[styles.container, { padding: theme.space[6], gap: theme.space[3] }, style]}>
      <Text style={[theme.type.h3, styles.centered, { color: theme.color.semanticText.danger }]}>
        {title}
      </Text>
      <Text style={[theme.type.body, styles.centered, { color: theme.color.textSecondary }]}>
        {body}
      </Text>
      <AmButton
        label={retryLabel}
        onPress={onRetry}
        variant="secondary"
        style={{ marginTop: theme.space[2] }}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  container: { alignItems: "center", justifyContent: "center" },
  centered: { textAlign: "center" },
});
```

- [ ] **Step 3: write `AmSkeleton`**

Create `apps/mobile/src/components/state/AmSkeleton.tsx`:

```tsx
import { useEffect } from "react";
import { type DimensionValue, type ViewStyle } from "react-native";
import Animated, {
  useAnimatedStyle,
  useSharedValue,
  withRepeat,
  withTiming,
} from "react-native-reanimated";

import { useTheme, type MaterialRole, type Theme } from "@/theme";

export interface AmSkeletonProps {
  readonly width?: DimensionValue;
  readonly height?: number;
  readonly radius?: keyof Theme["radius"];
  /** The material of the component this is standing in for. */
  readonly role?: Extract<MaterialRole, "surface" | "working">;
  readonly style?: ViewStyle;
}

/**
 * §51's loading placeholder.
 *
 * Drawn on the SAME material as the component it replaces, so a loading card
 * does not shimmer against a backdrop that swallows it. Opacity pulse rather
 * than a sweeping gradient: a shimmer needs a moving mask, and the anti-goals
 * rule out animated glass and per-item effects on long lists.
 */
export function AmSkeleton({
  width = "100%",
  height = 16,
  radius = "sm",
  role = "surface",
  style,
}: AmSkeletonProps) {
  const theme = useTheme();
  const pulse = useSharedValue(0.45);

  useEffect(() => {
    pulse.value = withRepeat(withTiming(0.9, { duration: 700 }), -1, true);
  }, [pulse]);

  const animated = useAnimatedStyle(() => ({ opacity: pulse.value }));

  return (
    <Animated.View
      accessibilityElementsHidden
      importantForAccessibility="no-hide-descendants"
      style={[
        {
          width,
          height,
          borderRadius: theme.radius[radius],
          backgroundColor: theme.color.surfaceSubtle,
        },
        // Kept for the caller's benefit: the role decides which surface this
        // is meant to sit on, and a `working` skeleton must not read lighter
        // than the solid card it replaces.
        role === "working" ? { backgroundColor: theme.color.surfaceSubtle } : null,
        animated,
        style,
      ]}
    />
  );
}
```

- [ ] **Step 4: write `AmToast` and its provider**

Create `apps/mobile/src/components/state/AmToast.tsx`:

```tsx
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { StyleSheet, Text, View } from "react-native";
import Animated, { FadeInDown, FadeOutDown } from "react-native-reanimated";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmMaterial } from "@/components/material";
import type { AmBadgeTone } from "@/components/display";
import { useTheme } from "@/theme";

export interface AmToastMessage {
  readonly message: string;
  readonly tone?: AmBadgeTone;
}

type ShowToast = (toast: AmToastMessage) => void;

const ToastContext = createContext<ShowToast | null>(null);

const VISIBLE_MS = 3200;

export function useToast(): ShowToast {
  const show = useContext(ToastContext);
  if (!show) throw new Error("useToast must be used inside <ToastProvider>");
  return show;
}

export interface ToastProviderProps {
  readonly children: ReactNode;
}

/**
 * One toast at a time, replaced rather than stacked.
 *
 * A stack needs an ordering policy, an exit choreography, and a maximum —
 * none of which anything in this app has asked for. When two things happen
 * at once the second is the one worth reading.
 *
 * The toast is `working`: solid. It is a message read to make a decision, it
 * appears over arbitrary content, and its tone is carried by a border and a
 * `semanticText` colour rather than by a saturated fill (§61).
 */
export function ToastProvider({ children }: ToastProviderProps) {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const [toast, setToast] = useState<AmToastMessage | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const show = useCallback<ShowToast>((next) => {
    if (timer.current) clearTimeout(timer.current);
    setToast(next);
    timer.current = setTimeout(() => setToast(null), VISIBLE_MS);
  }, []);

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );

  const value = useMemo(() => show, [show]);
  const tone = toast?.tone ?? "neutral";
  const accentColor =
    tone === "neutral" ? theme.color.borderStrong : theme.color.semantic[tone];
  const textColor =
    tone === "neutral" ? theme.color.textPrimary : theme.color.semanticText[tone];

  return (
    <ToastContext.Provider value={value}>
      {children}
      {toast ? (
        <Animated.View
          entering={FadeInDown.duration(theme.motion.standard)}
          exiting={FadeOutDown.duration(theme.motion.micro)}
          pointerEvents="none"
          style={[
            styles.host,
            { bottom: insets.bottom + theme.space[6], paddingHorizontal: theme.pagePadding },
          ]}
        >
          <AmMaterial
            role="working"
            radius="md"
            style={{
              paddingVertical: theme.space[3],
              paddingHorizontal: theme.space[4],
              borderLeftWidth: 3,
              borderLeftColor: accentColor,
            }}
          >
            <Text
              accessibilityLiveRegion="polite"
              accessibilityRole="alert"
              style={[theme.type.body, { color: textColor }]}
            >
              {toast.message}
            </Text>
          </AmMaterial>
        </Animated.View>
      ) : null}
    </ToastContext.Provider>
  );
}

const styles = StyleSheet.create({
  host: { position: "absolute", left: 0, right: 0 },
});
```

- [ ] **Step 5: mount the provider**

In `apps/mobile/src/app/_layout.tsx`, place `ToastProvider` inside `AmGround` so the toast renders above the ground but below nothing else:

```tsx
    <ThemeProvider>
      <AmGround>
        <ToastProvider>
          <Stack
            screenOptions={{ headerShown: false, contentStyle: { backgroundColor: "transparent" } }}
          />
        </ToastProvider>
      </AmGround>
    </ThemeProvider>
```

- [ ] **Step 6: write the barrel and run the gate**

Create `apps/mobile/src/components/state/index.ts`:

```ts
export { AmEmptyState } from "./AmEmptyState";
export type { AmEmptyStateProps } from "./AmEmptyState";
export { AmErrorState } from "./AmErrorState";
export type { AmErrorStateProps } from "./AmErrorState";
export { AmSkeleton } from "./AmSkeleton";
export type { AmSkeletonProps } from "./AmSkeleton";
export { ToastProvider, useToast } from "./AmToast";
export type { AmToastMessage, ToastProviderProps } from "./AmToast";
```

There is deliberately no standalone `AmToast` export. The toast is not a component a screen renders — it is a provider plus a hook, because a toast that a screen renders is a toast that unmounts with the screen that triggered it. The file is named `AmToast.tsx` for §69's naming convention; the exported surface is `ToastProvider` and `useToast`.

```bash
bun run format
make mb-check
```

Expected: `EXIT=0` once Tasks 5 and 6 have landed. Until then the only permitted failures are `Cannot find module '@/components/input'` and `'@/components/display'`.

### Acceptance criteria

1. `AmEmptyState`'s `actionLabel` and `onAction` are **required** props — omitting either is a type error. This is checked by attempting to render one without an action and seeing `tsc` fail.
2. `AmErrorState` follows §53's tone: it names what failed and offers a retry, and its heading uses `semanticText.danger`, not `semantic.danger`.
3. `AmSkeleton` pulses opacity between 0.45 and 0.9, is hidden from the accessibility tree, and takes a `role` so it can be drawn on the same material as the component it replaces.
4. `useToast()` outside `ToastProvider` throws a named error. Calling it twice in quick succession **replaces** the visible toast rather than stacking, and the pending timer is cleared both on replacement and on unmount — no leaked timer.
5. The toast renders on `working` (solid) with a 3 pt semantic left border, is `pointerEvents="none"` so it never swallows a tap, and carries `accessibilityLiveRegion="polite"`.
6. `grep -nE "#[0-9A-Fa-f]{6}|fontSize: [0-9]" apps/mobile/src/components/state/` returns nothing.
7. Every user-facing string in these four components is Bahasa Indonesia; the default retry label is `"Coba lagi"`.
8. `make mb-check` is `EXIT=0`.

**Block G and Block Q apply to this task.**

---

## Task 8: The component catalogue and the accessibility verification (AM-29)

One internal screen showing every primitive in every state, with the two switches that make the ticket's verification possible: a theme toggle (AM-15 AC2 wants both themes checked, and toggling the device is not a check anyone repeats) and a **force-no-blur** switch, which is the Android < 31 reality on demand. This is where the contrast table, the touch-target measurements, and the large-text behaviour are recorded.

**Files:**
- Create: `apps/mobile/src/app/catalog.tsx`
- Modify: `apps/mobile/src/app/index.tsx` — one link to the catalogue
- Modify: `apps/mobile/src/app/_layout.tsx` — mount `CapabilityControlContext`
- Create: `docs/superpowers/plans/2026-08-19-am-15-contrast-check.md` — the recorded results

**Interfaces:**
- Consumes: every primitive from Tasks 4–7, plus `useThemeControl`, `CapabilityControlContext`, and `contrastRatio` from `@anakmobil/tokens/derive`.
- Produces: the route `/catalog`, and the recorded verification evidence.

**TDD: no** — verify by running. The deliverable *is* the verification; a test of the catalogue would be a test of the components it displays, which Task 1 already covers where it can be tested and the eye covers where it cannot.

**Minimality check.** The catalogue is a plain scrolling route, not a navigator, not a tabbed gallery, and not a props playground. It ships as a real route rather than a `__DEV__`-gated one because the app is pre-launch, has exactly two screens, and gating it would need a mechanism that does not exist yet. **It does not become navigation**: no `AmAppBar`, no tab bar — one `chrome` sample exists so the role is exercised and nothing more.

### Steps

- [ ] **Step 1: mount the capability control**

In `apps/mobile/src/app/_layout.tsx`, hold the `forceTint` state and publish it, so the catalogue's switch reaches every primitive. Add `useMemo` and `useState` to the existing `react` import and `CapabilityControlContext` to the existing `@/theme` import; everything else in the file already arrived in Tasks 2, 3, 4, and 7.

```tsx
export default function RootLayout() {
  const fontsReady = useAppFonts();
  const [forceTint, setForceTint] = useState(false);
  const capability = useMemo(() => ({ forceTint, setForceTint }), [forceTint]);

  useEffect(() => {
    if (fontsReady) void SplashScreen.hideAsync();
  }, [fontsReady]);

  if (!fontsReady) return null;

  return (
    <ThemeProvider>
      <CapabilityControlContext.Provider value={capability}>
        <AmGround>
          <ToastProvider>
            <Stack
              screenOptions={{ headerShown: false, contentStyle: { backgroundColor: "transparent" } }}
            />
          </ToastProvider>
        </AmGround>
      </CapabilityControlContext.Provider>
    </ThemeProvider>
  );
}
```

- [ ] **Step 2: write the catalogue**

Create `apps/mobile/src/app/catalog.tsx`. It is long because it is a catalogue; every section is a flat list of rendered states.

```tsx
import { contrastRatio } from "@anakmobil/tokens/derive";
import { useState } from "react";
import { ScrollView, StyleSheet, Switch, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmAvatar, AmBadge, AmCard, AmChip } from "@/components/display";
import { AmButton, AmSelect, AmTextField } from "@/components/input";
import { AmMaterial } from "@/components/material";
import { AmEmptyState, AmErrorState, AmSkeleton, useToast } from "@/components/state";
import {
  numeric,
  useCapabilityControl,
  useMaterialCapability,
  useTheme,
  useThemeControl,
} from "@/theme";

type Transmission = "manual" | "matic" | "cvt" | "dct";

const TRANSMISSIONS: readonly { value: Transmission; label: string }[] = [
  { value: "manual", label: "Manual" },
  { value: "matic", label: "Automatic" },
  { value: "cvt", label: "CVT" },
  { value: "dct", label: "DCT" },
];

function Section({ title, children }: { readonly title: string; readonly children: React.ReactNode }) {
  const theme = useTheme();
  return (
    <View style={{ gap: theme.space[3] }}>
      <Text style={[theme.type.h3, { color: theme.color.textPrimary }]}>{title}</Text>
      {children}
    </View>
  );
}

export default function Catalog() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const { resolved, setScheme } = useThemeControl();
  const { forceTint, setForceTint } = useCapabilityControl();
  const capability = useMaterialCapability();
  const toast = useToast();

  const [text, setText] = useState("");
  const [transmission, setTransmission] = useState<Transmission | null>(null);
  const [chip, setChip] = useState<string>("Daily");

  // The contrast contract, computed live against the tokens actually loaded.
  // Task 1's test asserts the same pairs in CI; showing them here is what
  // makes AM-15 AC2 checkable by eye on a real device.
  const pairs: readonly { label: string; fg: string; bg: string }[] = [
    { label: "primary / working", fg: theme.color.textPrimary, bg: theme.material.working.solid },
    { label: "secondary / working", fg: theme.color.textSecondary, bg: theme.material.working.solid },
    { label: "tertiary / working", fg: theme.color.textTertiary, bg: theme.material.working.solid },
    { label: "primary / surface", fg: theme.color.textPrimary, bg: theme.material.surface.solid },
    { label: "secondary / surface", fg: theme.color.textSecondary, bg: theme.material.surface.solid },
    { label: "primary / chrome", fg: theme.color.textPrimary, bg: theme.material.chrome.solid },
    { label: "onAccent / accent", fg: theme.color.onAccent, bg: theme.color.accent },
  ];

  return (
    <ScrollView
      contentContainerStyle={{
        padding: theme.pagePadding,
        paddingTop: insets.top + theme.space[4],
        paddingBottom: insets.bottom + theme.space[10],
        gap: theme.space[8],
      }}
    >
      <Section title="Katalog Komponen">
        <AmCard role="working">
          <View style={{ gap: theme.space[3] }}>
            <View style={styles.row}>
              <Text style={[theme.type.body, { color: theme.color.textPrimary }]}>Tema gelap</Text>
              <Switch
                accessibilityLabel="Tema gelap"
                value={resolved === "dark"}
                onValueChange={(on) => setScheme(on ? "dark" : "light")}
              />
            </View>
            <View style={styles.row}>
              <Text style={[theme.type.body, { color: theme.color.textPrimary }]}>
                Paksa tanpa blur
              </Text>
              <Switch
                accessibilityLabel="Paksa tanpa blur"
                value={forceTint}
                onValueChange={setForceTint}
              />
            </View>
            <Text style={[theme.type.caption, { color: theme.color.textTertiary }]}>
              Material aktif: {capability}
            </Text>
          </View>
        </AmCard>
      </Section>

      <Section title="Material">
        <AmMaterial role="chrome" style={{ padding: theme.space[4] }}>
          <Text style={[theme.type.label, { color: theme.color.textPrimary }]}>
            chrome — hanya teks primer
          </Text>
        </AmMaterial>
        <AmMaterial role="surface" style={{ padding: theme.space[4], gap: theme.space[2] }}>
          <Text style={[theme.type.body, { color: theme.color.textPrimary }]}>surface — primer</Text>
          <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
            surface — sekunder
          </Text>
        </AmMaterial>
        <AmMaterial role="working" style={{ padding: theme.space[4], gap: theme.space[2] }}>
          <Text style={[theme.type.body, { color: theme.color.textPrimary }]}>working — primer</Text>
          <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
            working — sekunder
          </Text>
          <Text style={[theme.type.body, { color: theme.color.textTertiary }]}>
            working — tersier
          </Text>
        </AmMaterial>
      </Section>

      <Section title="Kontras">
        <AmCard role="working">
          <View style={{ gap: theme.space[2] }}>
            {pairs.map((pair) => {
              const ratio = contrastRatio(pair.fg, pair.bg);
              return (
                <View key={pair.label} style={styles.row}>
                  <Text style={[theme.type.caption, { color: theme.color.textSecondary }]}>
                    {pair.label}
                  </Text>
                  <Text
                    style={[
                      theme.type.caption,
                      numeric,
                      {
                        color:
                          ratio >= 4.5
                            ? theme.color.semanticText.success
                            : theme.color.semanticText.danger,
                      },
                    ]}
                  >
                    {ratio.toFixed(2)}:1
                  </Text>
                </View>
              );
            })}
          </View>
        </AmCard>
      </Section>

      <Section title="Tipografi">
        <AmCard role="working">
          <View style={{ gap: theme.space[2] }}>
            <Text style={[theme.type.display, { color: theme.color.textPrimary }]}>Display</Text>
            <Text style={[theme.type.h1, { color: theme.color.textPrimary }]}>H1</Text>
            <Text style={[theme.type.h2, { color: theme.color.textPrimary }]}>H2</Text>
            <Text style={[theme.type.h3, { color: theme.color.textPrimary }]}>H3</Text>
            <Text style={[theme.type.title, { color: theme.color.textPrimary }]}>Title</Text>
            <Text style={[theme.type["body-lg"], { color: theme.color.textPrimary }]}>Body Large</Text>
            <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>Body</Text>
            <Text style={[theme.type.label, { color: theme.color.textSecondary }]}>Label</Text>
            <Text style={[theme.type.caption, { color: theme.color.textTertiary }]}>Caption</Text>
            <Text style={[theme.type.micro, { color: theme.color.textTertiary }]}>Micro</Text>
            <Text style={[theme.type.title, numeric, { color: theme.color.textPrimary }]}>
              18×8.5 ET40
            </Text>
            <Text style={[theme.type.title, numeric, { color: theme.color.textPrimary }]}>
              225/40 R18
            </Text>
            <Text style={[theme.type.title, numeric, { color: theme.color.textPrimary }]}>
              146,120 KM
            </Text>
            <Text style={[theme.type.title, numeric, { color: theme.color.textPrimary }]}>
              Rp 14.500.000
            </Text>
          </View>
        </AmCard>
      </Section>

      <Section title="Tombol">
        <View style={{ gap: theme.space[3] }}>
          <AmButton label="Primary" onPress={() => toast({ message: "Primary ditekan" })} />
          <AmButton label="Accent" variant="accent" onPress={() => toast({ message: "Tersimpan", tone: "success" })} />
          <AmButton label="Secondary" variant="secondary" onPress={() => {}} />
          <AmButton label="Ghost" variant="ghost" onPress={() => {}} />
          <AmButton label="Destructive" variant="destructive" onPress={() => toast({ message: "Dihapus", tone: "danger" })} />
          <AmButton label="Disabled" disabled onPress={() => {}} />
          <AmButton label="Loading" loading onPress={() => {}} />
          <AmButton label="Small" size="sm" onPress={() => {}} />
          <AmButton label="Large" size="lg" onPress={() => {}} />
        </View>
      </Section>

      <Section title="Masukan">
        <AmTextField label="Plat nomor" value={text} onChangeText={setText} placeholder="B 1234 XYZ" />
        <AmTextField label="Dengan petunjuk" value="" onChangeText={() => {}} hint="Isi sesuai STNK" />
        <AmTextField label="Dengan kesalahan" value="B" onChangeText={() => {}} error="Plat nomor belum lengkap" />
        <AmTextField label="Nonaktif" value="Tidak bisa diubah" onChangeText={() => {}} disabled />
        <AmSelect
          label="Transmisi"
          value={transmission}
          options={TRANSMISSIONS}
          onChange={setTransmission}
        />
      </Section>

      <Section title="Tampilan">
        <AmCard role="surface">
          <Text style={[theme.type.body, { color: theme.color.textPrimary }]}>Kartu surface</Text>
        </AmCard>
        <AmCard role="working">
          <Text style={[theme.type.body, { color: theme.color.textPrimary }]}>Kartu working</Text>
        </AmCard>
        <View style={[styles.wrap, { gap: theme.space[2] }]}>
          {["Daily", "Track", "Stance", "Touring", "Show"].map((label) => (
            <AmChip key={label} label={label} selected={chip === label} onPress={() => setChip(label)} />
          ))}
        </View>
        <View style={[styles.wrap, { gap: theme.space[2] }]}>
          <AmBadge label="Verified" tone="success" icon="✓" />
          <AmBadge label="Perlu dicek" tone="warning" icon="!" />
          <AmBadge label="Tidak cocok" tone="danger" icon="×" />
          <AmBadge label="Informasi" tone="info" icon="i" />
          <AmBadge label="Netral" />
        </View>
        <View style={[styles.wrap, { gap: theme.space[2] }]}>
          <AmAvatar name="Oksa Satya" />
          <AmAvatar name="Budi" size={56} />
        </View>
      </Section>

      <Section title="Keadaan">
        <AmCard role="working">
          <View style={{ gap: theme.space[2] }}>
            <AmSkeleton height={20} width="60%" />
            <AmSkeleton height={14} />
            <AmSkeleton height={14} width="80%" />
          </View>
        </AmCard>
        <AmCard role="working">
          <AmEmptyState
            title="Belum ada modifikasi"
            body="Mulai bangun garasi digital kamu."
            actionLabel="Tambah modifikasi pertama"
            onAction={() => toast({ message: "Aksi empty state" })}
          />
        </AmCard>
        <AmCard role="working">
          <AmErrorState
            title="Garasi gagal dimuat"
            body="Data kamu aman. Coba beberapa saat lagi."
            onRetry={() => toast({ message: "Mencoba lagi", tone: "info" })}
          />
        </AmCard>
        <AmButton label="Tampilkan toast" variant="secondary" onPress={() => toast({ message: "Contoh pemberitahuan" })} />
      </Section>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", justifyContent: "space-between" },
  wrap: { flexDirection: "row", flexWrap: "wrap" },
});
```

- [ ] **Step 3: link the catalogue from the healthcheck screen**

In `apps/mobile/src/app/index.tsx`, add one link below the existing status block. Leave the rest of the screen alone — it is AM-14's deliverable and its inline styles are deliberate.

```tsx
import { Link } from "expo-router";
...
      <Link href="/catalog" style={styles.row}>
        Buka katalog komponen
      </Link>
```

- [ ] **Step 4: run the gate**

```bash
bun run format
make mb-check
```

Expected: `EXIT=0`.

- [ ] **Step 5: the visual review — light and dark, blur on and blur off**

```bash
make dev m=ios
```

Open `/catalog` and work through it four times: **light + glass**, **light + forced tint**, **dark + glass**, **dark + forced tint**. Capture a screenshot of each pass. For each, confirm:

- Every contrast row in the Kontras section reads green (≥ 4.5), and the numbers match Task 1's recorded table.
- No text disappears into its background in either theme (AM-15 AC2's second clause).
- With **Paksa tanpa blur** on, the material still reads as a deliberate material — a tinted panel with a top highlight and a bottom shadow — and not as a flat rectangle that lost an effect. **This is the Android < 31 review the spec requires**, and if it looks broken the fix is the edge, not the blur.
- Orange appears only on: the accent button, the selected chip, the selected option in the sheet, and the link. Nowhere else (AM-15 AC4).
- No glow anywhere.

- [ ] **Step 6: the touch-target measurement**

For `AmButton` at `size="sm"`, `AmChip`, the sheet's close button, and every `AmSelect` option, confirm the effective hit area is ≥ 44 × 44. Measure by temporarily adding `onLayout={(e) => console.log(label, e.nativeEvent.layout)}` to each and reading the Metro log, then remove it. Record the measured heights.

- [ ] **Step 7: the large-text check**

On the iOS simulator, Settings → Accessibility → Display & Text Size → Larger Text, move the slider to maximum. Reopen `/catalog`. Confirm:

- No text is clipped or truncated except where `numberOfLines` is deliberate (button labels, chip labels).
- The spec-data rows in the Tipografi section stay readable — §61's "automotive spec tables must remain readable at large text sizes".
- No layout overlaps and nothing scrolls sideways.
- Nowhere in the diff is `allowFontScaling={false}`.

- [ ] **Step 8: the reduced-transparency check**

Settings → Accessibility → Display & Text Size → Reduce Transparency, on. Reopen `/catalog`. Confirm every surface switches to its `solid` colour live, the reported "Material aktif" reads `tint`, and nothing about the layout moves.

- [ ] **Step 9: record the results**

Write `docs/superpowers/plans/2026-08-19-am-15-contrast-check.md` containing: the four screenshots referenced by filename, the full contrast table as rendered (role × text role × ratio, both themes), the measured touch-target heights, the large-text observations, the reduced-transparency observation, and the capability the simulator reported. AM-29's definition of done is "pemeriksaan kontras serta target sentuh tercatat hasilnya" — recorded, not merely performed.

### Acceptance criteria

1. `/catalog` renders every primitive from Tasks 4–7 in every state named in AM-26, AM-27, and AM-28.
2. The theme switch flips the whole catalogue without a reload; the **Paksa tanpa blur** switch flips every surface to its `tint` rendering without a reload.
3. Every contrast row reads ≥ 4.5 in both themes, and the values match Task 1's table. A row below 4.5 renders in the danger colour and is a blocking finding, not a note.
4. The no-blur pass looks intentional — this is a judgement call and it is recorded with a screenshot so the owner can disagree with it.
5. Every interactive element measures ≥ 44 × 44 pt with no caller-supplied padding, and the measurements are written down.
6. At maximum system text size nothing is clipped, nothing overlaps, and the page does not scroll sideways.
7. Reduce Transparency switches every surface to solid, live.
8. Orange appears in exactly four places and never as a material or a glow.
9. `docs/superpowers/plans/2026-08-19-am-15-contrast-check.md` exists and contains real recorded numbers, not "verified".
10. `make mb-check` is `EXIT=0`.

**Block G and Block Q apply to this task.** §27 applies in full: this is a frontend task, so the visual review is mandatory and its screenshots are the evidence.

---

## Task 9: Device check on a real mid-range Android — OWNER

**OWNER-EXECUTED. The agent cannot reach a physical device, and this cannot be delegated or simulated.**

The spec's platform ladder makes claims that are unverified until this runs: that the tint-and-edge rendering reads as a deliberate material on a real screen rather than as a flat panel, that the gradient does not band on a cheap display, that the inset `boxShadow` renders on Android's New Architecture at all, and that scrolling the catalogue stays smooth. Every one of those is a claim about hardware.

**Files:** none. This task produces evidence, appended to `docs/superpowers/plans/2026-08-19-am-15-contrast-check.md`.

**Interfaces:** consumes the shipped app from Tasks 1–8.

**TDD: no** — this is a hardware observation.

### Steps

- [ ] **Step 1 (OWNER): build and install the dev client on a real Android phone**

```bash
make mb-run-dev p=android
```

Prefer a mid-range device on Android 11–13, which is the population the ladder is written for. If the device is Android 12+, it is above the SDK-31 line and gets a real blur *when a blur library is present* — this ticket ships none, so both sides of the line should look identical, and that is itself the thing to confirm.

- [ ] **Step 2 (OWNER): open `/catalog` and check the four things a simulator cannot answer**

1. **Does the ground band?** A three-stop gradient across a full screen on an 8-bit panel is where banding shows. If it bands, the fix is a fourth stop or a dither, not an image.
2. **Does the inset shadow render?** RN's `boxShadow` with `inset` is New-Architecture-only on Android. If the bottom inset is missing, the surfaces will look like flat rectangles with a top line — record it, because the fallback (a 1 pt bottom border in `borderStrong`) is a one-line change and belongs in the ledger, not in a later ticket.
3. **Does the no-blur material read as intentional?** This is the review the whole design was shaped around. On a real screen, in daylight if possible.
4. **Is scrolling smooth?** The catalogue is the longest screen the app has. Any jank here is a warning about lists.

- [ ] **Step 3 (OWNER): check the working surfaces outdoors**

Take the phone outside, in sun. The `working` role exists because service history, fitment results, and AI warnings are read at a workshop in direct Indonesian sun. Confirm the solid surfaces stay legible and that the glass `surface` role does not wash out next to them.

- [ ] **Step 4 (OWNER): append the findings**

Add an Android section to `docs/superpowers/plans/2026-08-19-am-15-contrast-check.md`: device model, Android version, the four answers above, the outdoor observation, and a screenshot. Any defect goes to the plan's ledger.

### Acceptance criteria

1. The catalogue has been opened on a physical mid-range Android device and the device model and OS version are recorded.
2. Banding, inset-shadow rendering, no-blur legibility, and scroll smoothness each have a recorded yes/no with a note.
3. The outdoor legibility of the `working` role is recorded.
4. **AM-15 is not called done before this task closes.** The platform ladder is a claim about hardware and stays unverified until hardware answers it.

---

## Task 10: Revise `docs/design.md`

The design document currently says cards are solid, "use borders before shadows", shadows are for floating elements only, and "avoid glossy dashboard UI". A pervasive glass material reverses three of those, and the spec's second paragraph says so explicitly rather than pretending continuity. This task makes the document tell the truth — and carries four corrections the spec could not have known about, because they were found by computing its own arithmetic.

Runs **concurrently with Tasks 4–8**, from the moment Task 1's numbers exist. It touches only `docs/design.md`, which no other task opens.

**Files:**
- Modify: `docs/design.md` — §7, §8, §9, §11, §15, §40, §41, §42, §46, §47, §50, §66, §67, §72, §74, §76, plus a new Material System section
- Modify: `docs/superpowers/specs/2026-08-19-am-15-glass-material-design.md` — the scrim table only

**Interfaces:** consumes Task 1's final token values and the recorded contrast table. Produces no code.

**TDD: no** — documentation.

**Minimality check.** The document is revised, not rewritten. Each edit is the smallest change that makes a false sentence true, plus one new section because the material genuinely has no home in the existing structure. No section is reorganised, renumbered, or reworded for style.

### Steps

- [ ] **Step 1: the eleven revisions the spec listed**

| Section | Edit |
|---|---|
| **§15 Elevation** | Retitle to **Elevation & Material**. State that "use borders before shadows" is **superseded for mobile `chrome` and `surface`** and **survives intact for `working`**, and record why: a translucent surface gets its form from its edge, and a solid data surface does not need one. Point to the new Material System section. Remove "avoid glossy dashboard UI" only if it now reads as a contradiction — if it can stand as a warning against *gloss* rather than against *glass*, keep it and say so. |
| **§40 Dark Mode** | Add the dark glass recipe: `chrome` tint `#0E1217` at 80%, `surface` tint `#151A20` at 92%, `working` solid `#151A20`, with the composited colours and the note that the composite is the contract. |
| **§41 Light Mode** | Add the light recipe: `chrome` tint `#FBFCFD` at 80%, `surface` tint `#FCFDFD` at 92%, `working` solid `#FFFFFF`. Record why the tint is off-white rather than §41's subtle grey: at 92% that grey gives secondary text only 4.40:1 over a dark backdrop. Keep "avoid pure-white-everywhere" and explain that the edge, not the fill, is what separates a surface from the page. |
| **§46 Cards** | The default card becomes `surface`; the `working` exception is named with its list (service history, fitment results, forms, AI evidence, AI warnings, confidence badges, the eight §73 components). |
| **§47 Image Treatment** | Unchanged in substance. Add a note that the ground is a **tint, not a filtered photo**, and record the reason: a blurred, scrimmed photo violates both "preserve vehicle color" and "avoid excessive filters", and costs ~46 MiB per decoded 4000×3000 image. |
| **§50 Motion** | Add "no glow on glass"; reaffirm the existing ban on constant glowing orange effects. |
| **§66 Token Reference** | Add the material, edge, and ground tokens; update `--am-text-tertiary` to `#616A74`. |
| **§67 Dark Theme Tokens** | Add the dark material, edge, and ground tokens; update `--am-text-tertiary` to `#8E98A2`. |
| **§72 Anti-Patterns** | New entry, **"Glassmorphism Tempelan"**: a four-sided white border, uniform transparency across roles, glass on warnings or data, blur as identity, decorative refraction blobs. |
| **§74 Recommended Initial Visual Style** | Add the ground and the material. Scope the 85 / 10 / 5 ratio to **UI tokens, imagery excluded** — a red car makes the pixel reading false on its own. |
| **§76 Final Direction** | One line for the material direction. |

- [ ] **Step 2: the new Material System section**

Add a section after §15 (or at the end, matching the document's own numbering habit) covering, in this order: the three roles and where each lives · the contrast contract in one paragraph — *the binding token is the composited colour that passes AA, never an opacity value* · the full matrix table from this plan's "The numbers this plan is built on" · the edge, with the four-sided-border ban stated by name · the ground and its boundary (it takes a tint it is given; extraction is the garage epic's) · the platform ladder as written in the spec, with the note that this repository ships no blur library and why.

- [ ] **Step 3: the four corrections the spec could not have known about**

These were found by computing the spec's own arithmetic against the document's own palette. Each is a defect in `docs/design.md` that predates the glass material.

| Section | Correction |
|---|---|
| **§7 Neutral Palette** | `--text-tertiary` `#8A939D` → `#616A74` and `--text-dark-tertiary` `#737D87` → `#8E98A2`, with the measured ratios and one line saying the previous values failed AA (2.80 and 3.87 at worst). |
| **§8 Semantic Colors** | State that the four semantic values are **fills, borders, and icons**, and add the per-theme `semanticText` table for when they carry words. Record why: success is 3.99:1 as text on the dark surface, warning 2.81:1 on white. |
| **§9 AI Confidence Colors** | Point at `semanticText` for the label colours, and reaffirm §61 — a confidence badge carries a label and a shape as well as a colour, and is always solid, never glass. |
| **§42 Buttons** | The strongest brand CTA keeps `#ED491C` and its **label changes from `#FFFFFF` to `#0F141A`**. White on that orange is 3.77:1; graphite-950 is 4.91:1. Record the measurement. |

Also add one line to **§11 Type Scale**: the mobile scale's 650 weight renders at 600 in the app, because React Native's `fontWeight` has no 650 and the shipped Inter cuts are static. The desktop scale keeps 650 and 750, where the web loads the variable cut.

- [ ] **Step 4: correct the spec's scrim table**

**Already done — verify, do not redo.** The spec was corrected at source on 2026-08-19, before this plan was merged: the three scrim percentages now read **55.4% / 73.1% / 93.3%**, with a paragraph recording that the originals were `1 − L`, the linear luminance ceiling read as an sRGB alpha, and that compositing back gives exactly 4.50:1.

Your step is to confirm it, not to repeat it: `grep -n "55.4\|73.1\|93.3" docs/superpowers/specs/2026-08-19-am-15-glass-material-design.md` should hit, and `grep -n "83.3\|94.1\|99.4"` should not.

One conclusion in the spec did change with the numbers, and the corrected text already reflects it: **it is no longer true that every text-bearing surface is nearly opaque.** Primary text needs only 55.4%, which is genuinely translucent — it is the *quiet* text that forces opacity. What survives untouched is the rule that matters: secondary and tertiary text may never sit on a material whose backdrop is unknown, because 93.3% is opaque in all but name. If any task's tokens were derived from the belief that all surfaces must be near-opaque, they are more conservative than they need to be and may be re-derived.

- [ ] **Step 5: verify**

```bash
bun run format:check
```

Markdown is excluded from Prettier by `.prettierignore` — the specs and READMEs are wrapped by hand — so this only confirms nothing else drifted. Then re-read §15, §40, §41, and §46 end to end and confirm no sentence in them now contradicts another.

### Acceptance criteria

1. All eleven revisions from the spec's own table are applied, and each names its reason rather than only its outcome.
2. The new Material System section carries the three roles, the contrast contract, the full matrix, the edge with its four-sided-border ban, the ground's boundary, and the platform ladder.
3. §7's two tertiary values match `packages/tokens`, and §66/§67 match them too — a reader picking up either file gets the same number.
4. §8 distinguishes semantic fills from semantic text and carries the eight `semanticText` values with their measured ratios.
5. §42's orange CTA label is `#0F141A` with the 4.91:1 measurement recorded.
6. §11 records the 650 → 600 mobile substitution.
7. The spec's scrim table reads 55.4% / 73.1% / 93.3% and carries the one-sentence explanation. Nothing else in the spec is edited.
8. `grep -n "737D87\|8A939D" docs/design.md` returns nothing.

**Block G applies to this task.** Block Q does not — no code is written.

---

## Execution status

- [x] Task 1 — Colour maths, material tokens, contrast contract (`packages/tokens`) · **TDD: yes** — DONE 2026-08-19. Writer `opus`, red-first evidence recorded (module-resolution red, then the pre-repair tertiary red: `light tertiary on #F7F8FA = 2.93`, matrix enumerating 3.12/4.18). Controller re-ran gates: `make ds-check` EXIT=0 **28 pass**, `make fe-check` EXIT=0, `bun install --frozen-lockfile` EXIT=0, `bun.lock` unchanged. Four plan corrections found by the writer, applied to this file: (1) `mix()` test assertion was self-contradictory — `mix(base, other, 0)` must return `base`, the plan asserted the other; (2) `material.test.mjs` must composite against `ground.stops[1]` (the middle stop = each theme's `background`), not `stops[0]` — every `solid` in the plan's matrix matches `stops[1]` exactly; (3) `dark.semanticText.danger` is `#EC6363`, not `#E45B5B` (4.26 on `surfaceSubtle` fails the plan's own test); (4) `git diff --stat packages/tokens/dist/` is structurally always empty — `dist/` is gitignored — verify repaired values by grepping the CSS files instead. Landing renders light tertiary darker (`#8A939D` → `#616A74`); goes in the commit message per Block G item 15.
- [x] Task 2 — Mobile theme layer and capability resolver — DONE 2026-08-19. Writer `sonnet`. Controller re-ran gates: `make mb-check` EXIT=0, `bun install --frozen-lockfile` EXIT=0 "(no changes)", symlink `apps/mobile/node_modules/@anakmobil/tokens -> ../../../../packages/tokens` verified. **Metro-resolution evidence deviates from the plan's Step 4:** no interactive Metro/simulator run (controller owns the runtime); instead `bun x expo export --platform ios` with the probe import in place exited 0 (`iOS Bundled 8889ms … (1104 modules)`), proving Metro resolves `@anakmobil/tokens` AND its `./derive` subpath through the exports map. The literal `tokens probe #14191F 21` console line was therefore never printed; the runtime confirmation folds into Task 8's simulator pass. Probe fully removed. `expo-glass-effect` predicate signatures verified against the installed `.d.ts` — no drift. No `metro.config.js` needed.
- [x] Task 3 — Typography: Inter, the mobile scale, tabular figures — DONE 2026-08-19. Writer `sonnet`. Controller re-ran gates: `make mb-check` EXIT=0, `bun install --frozen-lockfile` EXIT=0, `bun.lock` byte-identical across repeat. Font package shape exactly as planned (4 cuts imported). Plan correction: `bun add --filter` does not exist → `bun add --cwd apps/mobile` (Step 1 edited in place). Ledger finding #4 (typed `source` binding in `buildTypeScale`) folded in mid-task and re-gated. AC6's simulator half (renders in Inter) deferred to the controller's Task 8 visual pass. Review dispatched separately; reviewer must confirm `h3`/`title` → 600 (ledger #6).
- [x] Task 4 — `AmGround` and `AmMaterial` — DONE 2026-08-19. Writer `sonnet`. Controller re-ran `make mb-check` EXIT=0. No GlassView API drift (`glassEffectStyle`/`tintColor`/`colorScheme` verified against the installed `.d.ts`). `working` cannot reach `GlassView` by construction (`solid = coverage === 1` short-circuits the glass branch). `borderWidth` grep: only `borderTopWidth` (glass edge) + the `solid && edge` §46 branch — no four-sided border. Runtime checks (both capability rungs, Reduce Transparency live toggle) deferred to the controller's Task 8 pass. Review dispatched on `opus` (the material choke point).
- [x] Task 5 — Input primitives (AM-26) · *parallel with 6, 7, 10* — DONE 2026-08-19 (code-complete; full gate deferred to the post-fan-out controller run). Writer `sonnet`, transcription verbatim, zero deviations. Workspace check failed ONLY with `TS2307: Cannot find module '@/components/display'` at `AmSelect.tsx(4,31)` — the permitted class while T6 was mid-write. AC8 grep: one hit, a comment documenting the contrast math. Visual/44pt halves of the ACs defer to the controller's Task 8 pass.
- [x] Task 6 — Display primitives (AM-27) · *parallel with 5, 7, 10* — DONE 2026-08-19 (code-complete; full gate = controller's post-fan-out `make check`). Writer `sonnet`. Four recorded deviations, all forced by real files: (1) AmCard passes a merged object into AmMaterial (the landed `style?: ViewStyle` rejected the plan's array — independently confirmed as T4 review finding S1; the controller widened `AmMaterialProps.style` to `StyleProp<ViewStyle>` immediately after, so both forms now type-check); (2) `expo-image` style union forced `as ImageStyle[]` in AmAvatar; (3) `StyleSheet.absoluteFillObject` not declared in RN 0.86.2's d.ts → `absoluteFill`; (4) two scoped `eslint-disable-next-line react-hooks/immutability` in AmBottomSheet gesture callbacks — the bundled react-hooks v6 compiler rule doesn't model Reanimated's SharedValue mutation API; no Reanimated eslint plugin installed by design. Frozen AmBottomSheetProps emitted exactly.
- [x] Task 7 — State primitives (AM-28) · *parallel with 5, 6, 10* — DONE 2026-08-19 (code-complete; full gate deferred to the post-fan-out controller run). Writer `sonnet`. One deviation: dropped the plan's unused `View` import in `AmToast.tsx` (eslint no-unused-vars). Cross-group edges consumed exactly as frozen and type-checked clean once sibling barrels landed. Workspace check red ONLY on sibling `display/` files (their writer's scope). `ToastProvider` mounted inside `AmGround` in `_layout.tsx`.
- [x] Task 8 — Component catalogue and accessibility verification (AM-29) — DONE 2026-08-20. Code half: writer `sonnet` (ledger H1 folded in — `useMaterialTextColor` exercised by the Material section). Visual half: CONTROLLER, on iPhone 17 / iOS 26.5, run AFTER the fix pass. All four combos captured (light/dark × liquid-glass/tint), contrast tables green in both themes and byte-identical to Task 1's numbers, sheet gesture-dismiss verified, toast verified, large-text (accessibility-XXXL) reflow verified incl. the spec-data rows, touch targets recorded. Reduce Transparency: rendering path proven via the forceTint switch; the OS-listener toggle cannot be driven from simctl → deferred to Task 9 owner device. **One correctness defect found by opening the page and fixed by the controller** (ledger #29 — the navigation-container background covered AmGround). Full evidence: `2026-08-19-am-15-contrast-check.md` + `am-15-checks/*.png`.
- [ ] Task 9 — Real mid-range Android device check (**OWNER**)
- [x] Task 10 — `docs/design.md` revision · *parallel with 4–8* — DONE 2026-08-19. Writer `sonnet`, docs only (263 insertions / 11 deletions in `docs/design.md`; the spec verified, not edited). Controller re-ran `bun run format:check` EXIT=0 and `grep -c "737D87\|8A939D" docs/design.md` → 0. New `# 77. Material System` placed at the END (the plan's own numbering-habit option — inserting after §15 would force renumbering, which the minimality rule forbids). AC7 nuance: `grep "83.3"` hits ONCE in the spec, on the explanation sentence the plan itself requires to quote the wrong historical figures — the scrim TABLE carries only 55.4/73.1/93.3; the plan's "should not hit" phrasing was imprecise, content is correct. Review deferred: folds into Task 8's review per the run-shape verdict.

## Review findings ledger

*(Every finding arrives here the moment it is reported, with: task, severity, file and line, the concrete failure scenario, and the smallest fix. Severity vocabulary: `structural` — a token value, a public component signature, or a material recipe, raised and fixed immediately because later tasks build on it · `correctness` · `test-integrity` · `hygiene`. Everything except `structural` is worked in one fix pass after Task 8.)*

1. **T1 · test-integrity (low)** — `packages/tokens/src/tokens.js` ground docstring (~line 284) claims "material.test.mjs pins that relationship" (middle stop = theme's own `background`), but no assertion ties `ground.stops[1].color` to the theme's `background` — only to the recorded solids. Failure: a later edit changes `light.background` alone; all 28 tests stay green while the ground diverges from the material contract's compositing base. Smallest fix: in `material.test.mjs`'s first test, `assert.equal(groundBase, THEMES[themeName].background)`. *(Reviewer: fable, 2026-08-19.)*
2. **T1 · hygiene (trivial)** — `onGraphite` is exported but excluded from the hex sweep in `test/tokens.test.mjs` (only `onAccent` asserted). A plan gap, not a writer slip. Fix: add `onGraphite` to the sweep. *(Reviewer: fable, 2026-08-19.)*
3. **T2 · correctness (low)** — `apps/mobile/src/theme/capability.ts:42-56`: the initial `isReduceTransparencyEnabled()` promise and the `reduceTransparencyChanged` listener write the same state with no ordering guard; a promise resolving after an event fires overwrites the newer value → glass renders while Reduce Transparency is on until the user toggles again. Fix: `let settled = false;` set in the listener; `if (alive && !settled)` in `.then`/`.catch`. *(Reviewer: opus, 2026-08-19.)*
4. **T2 · test-integrity (low)** — `apps/mobile/src/theme/theme.ts:37-41`: `Object.fromEntries(...) as Record<TypeName, TextStyle>` casts from `any` — tsc verifies NO key coverage (proved with a deliberately missing key compiling EXIT=0). Fix folded into Task 3's rewrite of `buildTypeScale`: bind `const source: Readonly<Record<TypeName, string>> = typeMobile;` and map over `source`. *(Reviewer: opus, 2026-08-19 — instruction sent to the T3 writer in flight.)*
5. **T2 · hygiene (low)** — `capability.ts:31`: `useCapabilityControl()` returns a fresh object per call when context is absent; hoist a module-level `NO_CONTROL` constant. Also: Task 8's toggle must render inside `CapabilityControlContext.Provider` or the no-op default makes the switch silently dead — the plan's Task 8 Step 1 already mounts the provider in `_layout.tsx`; T8's reviewer confirms. *(Reviewer: opus, 2026-08-19.)*
6. **T2 · hygiene (trivial, tracked deferral)** — `theme.ts:39` emits `fontWeight: "650"` for `h3`/`title`, invalid in RN; plan-sanctioned until Task 3's 650→600 mapping lands. T3's reviewer MUST confirm `h3` and `title` resolve to 600/`Inter_600SemiBold`. *(Reviewer: opus, 2026-08-19.)*
7. **T2 → T4 note (not a finding)** — `capability.ts:62` calls both `expo-glass-effect` predicates on every render of every consumer; process-constant values. Only if a profile shows it: lazily-initialised module constant (NOT top-level — native modules may not be ready at module eval). *(Reviewer: opus, 2026-08-19.)*
8. **T3 · correctness (medium)** — `apps/mobile/src/theme/fonts.ts:1-7`: importing from the `@expo-google-fonts/inter` barrel bundles ALL 18 cuts (6,104 KB) though only 4 are registered (1,348 KB) — Metro tree-shaking is opt-in and not enabled anywhere in this repo → ~4.75 MB dead font binaries per platform binary. Exactly what the plan's minimality check set out to prevent; invisible to tsc/eslint. Fix: deep imports (`@expo-google-fonts/inter/400Regular` etc., each subpath has its own index.js + .d.ts) + `useFonts` from `expo-font` directly. Verify: `bunx expo export --platform ios` then count `.ttf` in dist/assets — 4 after, 18 before. *(Reviewer: opus, 2026-08-19.)*
9. **T3 · hygiene (trivial)** — `_layout.tsx:9,18`: `void promise` without rejection handler → LogBox unhandled-rejection warning if either splash call rejects; Expo's own example is equally bare. If touched in the fix pass: `.catch(() => {})`. Also `hideAsync()` is the backwards-compat name; current API is sync `hide()`. *(Reviewer: opus, 2026-08-19.)*
10. **T3 · confirmations** — ledger #4 landed (typed `source` binding enforces TypeName coverage, one direction); ledger #6 CONFIRMED: `h3`/`title` (650) → cut 600/`Inter_600SemiBold`, emitted `fontWeight` `"600"`, `"650"` unreachable. Both closed. *(Reviewer: opus, 2026-08-19.)*
11. **T4 · structural (S1) — FIXED IMMEDIATELY by the controller** — `AmMaterialProps.style` was `ViewStyle`; T6's AmCard passes a style array → TS2559. Widened to `StyleProp<ViewStyle>` in `AmMaterial.tsx` the moment the review landed (RN's own components use `StyleProp<ViewStyle>`; AmMaterial was narrower than everything it wraps). T6's writer had independently worked around it with a merged-object spread — semantics identical, both forms now compile. *(Reviewer: opus, 2026-08-19.)*
12. **T4 · correctness (low, → T9 device pass)** — the AA guarantee is machine-proven on the tint path only; the `GlassView` branch renders a native adaptive effect that approximates but is not defined as the composite. T9 must read `surface` (92%) and `chrome` (80%) against a bright backdrop on an iOS 26 device/simulator, and confirm the inset `boxShadow` edge actually renders over the native glass view. *(Reviewer: opus.)*
13. **T4 · correctness (low)** — `AmGround` throws at the app root on a malformed `tint` (`channels()` throws on non-`#RRGGBB`), in production. Nothing supplies a tint in this ticket; guard by documenting `#RRGGBB` required on the `tint` prop doc comment so the garage epic reads the contract at the prop. *(Reviewer: opus.)*
14. **T4 · hygiene (H1, folded into T8's brief)** — `useMaterialTextColor` has zero call sites; AC5 has no runtime exercise. Fix: T8's catalogue Material section calls it for its three sample labels. *(Reviewer: opus.)*
15. **T4 · hygiene (H2, plan-text contradiction)** — the plan's creative-direction section says "No third stop" / lists ">2 stops" as a tell, while AC1 and the tokens mandate a THREE-stop gradient (the middle stop is the AA backdrop constant). The code follows AC1/tokens, which is right; recorded here so no future reviewer re-raises it against `AmGround`. *(Reviewer: opus.)*
16. **T4 · hygiene (H3)** — `AmGroundProps.style` has no caller and is outside the task's Interfaces contract; dead flexibility from the plan's own code block. Decide in the fix pass: delete or keep. *(Reviewer: opus.)*
17. **T6 · note for its reviewer** — verify the `as ImageStyle[]` cast in AmAvatar and the two scoped `react-hooks/immutability` disables in AmBottomSheet are the narrow, justified forms the writer reported. *(Controller.)*
18. **T5 · correctness (medium)** — `AmButton.tsx`: `minHeight: Math.max(HEIGHT[size], touchTargetMin)` makes `size="sm"` dead code — sm and md render identically at 44pt; AC1's "36pt visual in a 44pt target" is not met. Fix (preserves the frozen props): `minHeight: HEIGHT[size]` + `hitSlop={Math.max(0, (touchTargetMin - HEIGHT[size]) / 2)}` on the Pressable — Block Q's own sanctioned route. Do NOT delete `sm` from the union (T7/T8 depend on it). *(Reviewer: opus, 2026-08-19.)*
19. **T5 · correctness (medium)** — `AmSelect.tsx:92,100`: dark-theme selected option label (`accentText` = `#ED491C`) on pressed `surfaceSubtle` `#202730` = **4.00:1**, below AA (unpressed 4.69 passes). Escaped the token matrix because `accentText × surfaceSubtle` is neither a text role nor a material role. Fix (also closes #20): selected row label `textPrimary` + trailing check glyph tinted `accentText` (glyph needs only 3:1 under 1.4.11). *(Reviewer: opus.)*
20. **T5 · correctness (medium, recipe-level)** — AmSelect communicates selection by colour alone to sighted users (WCAG 1.4.1 / §61); `accessibilityState.selected` serves screen readers only. AC6's wording ("colour and accessibilityState") is itself the gap — fix the code per #19 AND correct AC6's wording in this plan. *(Reviewer: opus.)*
21. **T5 · correctness (low)** — `AmTextField.tsx:62`: `accessibilityHint={hint}` never announces `error`; VoiceOver user hears the field, not the failure. Fix: `accessibilityHint={error ?? hint}` (+ optional `accessibilityLiveRegion="polite"` on the error Text). *(Reviewer: opus.)*
22. **T5 · test-integrity (trivial, recipe)** — AC8's grep as written returns the `#ED491C` COMMENT in AmButton.tsx and can never pass verbatim; exempt comments in the AC wording. *(Reviewer: opus.)*
23. **T5 · hygiene (batch)** — (a) `numberOfLines={1}` on AmButton non-loading branch only — inconsistent with the wrapping loading branch; drop it on AmButton, keep deliberately on the AmSelect trigger; (b) `minHeight: 52` literal duplicated in AmTextField/AmSelect — future `layout.controlHeight` token, follow-up not in-task; (c) caller `style` can override the 44pt floor (spread order) — move minHeight/minWidth after `style` if the floor is meant inviolable; (d) no `radiogroup` container around AmSelect options; (e) disabled opacity 0.45 (button) vs 0.5 (field/select) — one state, two values, from the plan's own code. *(Reviewer: opus.)*
24. **T7 · correctness (minor, a11y)** — `AmToast.tsx:99-100`: toast never announced on iOS (`accessibilityLiveRegion` is Android-only; RN's `accessibilityRole="alert"` triggers no VoiceOver announcement). Fix inside `show`: `if (Platform.OS === "ios") AccessibilityInfo.announceForAccessibility(next.message);` — guarded, or Android double-announces. *(Reviewer: opus, 2026-08-19.)*
25. **T7 · hygiene (batch)** — (a) `AmSkeleton` `role` prop is INERT — both ternary branches set the same `surfaceSubtle`; the plan's own snippet is the source (recipe correction, §29): delete the dead branch, do NOT wire `surfaceRaised` (invisible on white); (b) `useMemo(() => show, [show])` no-op — `value={show}`; (c) titles in Empty/ErrorState should carry `accessibilityRole="header"`; (d) fired timer leaves stale `timer.current` — optional null-out; (e) replacing a visible toast doesn't replay the entering animation — acceptable, noted for the catalogue pass; (f) plan's Step 4 snippet imports unused `View` — already dropped by the writer, correct the plan text if ever re-run. *(Reviewer: opus.)*
26. **T6 · correctness (C1)** — `AmBottomSheet.tsx:57-59`: the `withTiming(600, …)` completion callback ignores `finished`, so an exit animation cancelled by the reopen-reset effect still calls `onClose` — a reopened sheet closes itself within a ~280ms window. Fix: `(finished) => { if (finished) runOnJS(onClose)(); }`. Do NOT also reset `translateY` in that callback (runOnJS is async; the sheet would snap up before unmount). *(Reviewer: opus, 2026-08-19.)*
27. **T6 · hygiene (batch)** — (a) `AmBadge.tsx:36` dead ternary `accessibilityLabel={icon ? \`${label}\` : label}` — plan-inherited (plan's own Task 6 code); fix to `accessibilityLabel={label}`; (b) that label is inert — the View needs `accessible`; (c) sheet scrim is a second focusable "Tutup" ahead of the title — `accessibilityElementsHidden` + `importantForAccessibility="no"` on the scrim; (d) AmCard's merged-object spread can revert to the plan's array form now that `AmMaterialProps.style` is `StyleProp<ViewStyle>`; (e) second eslint-disable lacks its own reason line; (f) after drag-dismiss `translateY` stays 600 until the next open's effect — Modal slide-in covers it; known quantity for T9. `as ImageStyle[]` cast: VERIFIED honest, keep. Both eslint-disables VERIFIED genuine and minimal (`--report-unused-disable-directives` clean; stripping them reproduces exactly 2 rule hits). *(Reviewer: opus.)*
28. **T8-visual · correctness (HIGH — found by §27's open-the-page rule, fixed immediately by the controller)** — Expo Router's navigation container paints its theme's `colors.background` (`rgb(242,242,242)`) behind every screen; `contentStyle: transparent` clears only the screen layer. `AmGround` was therefore NEVER visible, and in dark theme the app sat on a light page with near-invisible headings — AM-15 AC2's second clause, failing while every static gate was green. Fix: wrap the Stack in **expo-router's own** `ThemeProvider` (`DarkTheme`/`DefaultTheme` re-exported from `expo-router`; SDK 56+ dropped react-navigation — `@react-navigation/native` was tried, produced the router's own incompatibility error, and was removed again) with `colors.background: "transparent"`; plus `key={theme.name}-{tint}` on AmGround's View so the experimental gradient rebuilds on theme change. Verified after: dark ground from cold start, live toggle follows, headings readable. Evidence: `am-15-checks/dark-tint-STALE-GROUND-BUG.png` (before) vs `dark-glass-top.png` (after). *(Controller, 2026-08-20.)*
29. **Fix pass record** — all 27 ledger fixes applied by a `sonnet` writer, controller re-ran `make ds-check` (28 pass) / `make mb-check` / `make fe-check` all EXIT=0. Font bundle verified 18 → **4** `.ttf` entries via the export's `metadata.json` (Expo hashes asset filenames — the plan's `find -name "*.ttf"` counts 0 by construction; corrected here). Items #1/#2 landed as assertions inside existing `test()` blocks, so the suite count stays 28. *(2026-08-20.)*
30. **Final review · correctness (HIGH, F1) — FIXED + re-verified** — the controller's first ground fix put `key={theme.name}` on the View wrapping `{children}`, which unmounts the ENTIRE app subtree on every theme change (typed text, select value, chip selection, scroll position all discarded; two-tap reproducible). Fix per the reviewer's smallest-fix: the key moved to an absolute, `pointerEvents="none"` gradient-only layer inside an unkeyed outer View carrying the flat fallback; children keep their fibers. Re-verified on-simulator: system appearance flipped dark→light live — full re-theme, ground repainted, AND the selected chip + scroll position survived (`am-15-checks/light-after-live-flip-state-preserved.png`). *(Final reviewer: opus · fixed by controller, 2026-08-20.)*
31. **Final review · correctness (HIGH, F2) — FIXED + re-verified** — this branch's dark ground made AM-14's unthemed healthcheck screen invisible in dark (RN default `#000000` text = 1.12:1). The reviewer correctly overruled the contrast-check doc's "not AM-15's" disposition: a ticket that breaks a shipped screen owns the break. Fix: `useTheme()` in `index.tsx` — `textPrimary` rows, `semanticText.success/danger` status, `accentText` link; hardcoded `#137333`/`#c5221f` removed. Layout/copy untouched. Dark render verified readable. *(Final reviewer: opus · fixed by controller, 2026-08-20.)*
32. **Final review · hygiene (F3, F4) — both closed in the same pass** — F3: `docs/design.md` §67 `--am-surface-subtle` was `#1B2128` (surfaceRaised's value) → `#202730`, the rest of the dark block sweep-verified clean; F4: `AmSkeleton.role`'s doc comment now states it is caller documentation and does not change the rendered fill. *(Final reviewer: opus.)*
33. **Final review · fidelity + honesty lenses** — fix pass 27/27 zero-drift (double-checked independently), `bun.lock` carries no `@react-navigation/native` residue, Tidak boleh ada sweep clean (zero hex literals outside doc comments across components/theme/catalog), contrast-check doc judged honest except the F2 attribution (corrected). *(Final reviewer: opus.)*
34. **T10 · hygiene (pre-existing, out of AM-15 scope)** — `docs/design.md` §67 documents `--am-border-strong: #39434E` while `packages/tokens/src/tokens.js` ships `borderStrong: "#3A434E"` (dark). One-character discrepancy predating this ticket; left alone per the minimal-edit rule. Decide in the fix pass whether to correct the doc line (one character) or defer to its own ticket. *(Writer T10, 2026-08-19.)*

## Execution mode

**Mapped once, up front, then run as a ready-queue.** The dependency map is drawn in the run-shape verdict above and is not re-derived per task. T1 → T2 → T3 → T4 is a genuine serial chain — every link consumes the previous link's exported names — and saying so is a finding, not a habit: each of those four writes into a file set the next one reads.

The fan-out is real and worth taking. **T5, T6, T7, and T10 dispatch together** the moment T4 lands: their file sets are disjoint, none imports another, and the one cross-group edge (`AmSelect` needs `AmBottomSheet`) is handled by freezing that signature in both briefs rather than by serialising the pair. Four writers in flight, inside the 3–5 band. T8 waits on all three primitive groups; T9 is the owner's.

**Tiers.** T1 is `opus` — it is the only `TDD: yes` task, it changes two published token values, and every later contrast claim rests on its arithmetic. T2, T4, and T8 are `sonnet`: substantial but fully specified. T3, T5, T6, T7, and T10 are `sonnet` too — the plan carries their code, but each involves enough judgement (a font package whose exact shape must be read at install time, a gesture, a document with sixteen sections to reconcile) that `haiku` would spend more turns than it saves. **No task is written inline by the controller:** T3 is the only one that comes close to the four small-and-serial tests, and it fails the "≤2 files" test.

**Reviewers.** `opus` on T1 (token values, a public contract, and the one testable claim in the ticket) and on T4 (the material choke point — a defect there reproduces in every primitive). `sonnet` elsewhere, always with an explicit `model` override and never the same model as that task's writer. T10 is a documentation diff and folds into T8's review. Each reviewer brief carries Block G, Block Q, the `Tidak boleh ada` list, and these lenses:

- **`audit-coverage` and `security-coverage`: N/A, stated explicitly.** Nothing here mutates business state, moves money, alters access, or emits a sensitive output. A reviewer answers "N/A" out loud rather than leaving it silent.
- **`algorithmic-complexity`: N/A**, for the reason given in this plan's own section.
- **`frontend-senior` and `react-doctor`** on every task that writes a component (4–8): re-render cost, the accessibility tree, and whether a `StyleSheet.create` was skipped in favour of an inline object rebuilt every render.
- **The `Tidak boleh ada` scan.** The single highest-value check on this ticket: did a primitive quietly become feature-specific, did a blur or gradient library appear, did glass reach a warning or a badge, did a token value get written as a literal because it was faster.

**Every task that writes UI code says, in its brief: invoke `frontend-design` first.** And it says the next sentence too, because it is what stops a wasted round: **the font and palette gate does not apply here.** This surface has a committed design system — `docs/design.md` plus the spec plus `packages/tokens` — Inter is settled, the AnakMobil palette is settled, and the material recipe is settled in this plan's matrix. `impeccable` refines within that system; it does not re-open it.

**Gates are re-run by the controller, never taken on a writer's word.** `make ds-check` after any `packages/tokens` change, `make mb-check` after any `apps/mobile` change, both after a task that touches both, exit codes read rather than output skimmed. `make check` before the fix pass closes.

**Finishing, in order:** consolidate the ledger → fix pass → `make check` green → push and watch **both** CI jobs to green (`mobile.yml` for `apps/mobile/**`, `frontend.yml` for `packages/tokens/**` — a token change reddens the second, not the first, and watching only the obvious one is how this ticket would ship a red branch) → the ticket's Artifact → move AM-15 and its five subtasks → show the owner the diff → commit.

**Commits at the end, one logical change each**, dispatched to `haiku`. The natural split is six: the tokens and the contrast contract; the mobile theme and typography; the material; the three primitive groups (one commit each is defensible, or one commit if they landed together); the catalogue and its recorded verification; the `docs/design.md` revision. The token commit's message must say that `apps/landing` renders differently as a result, because a reviewer who does not know that will read it as an unexplained visual change.

---

## Docs consulted

Read from the installed packages and the repository at plan time rather than from memory:

- **`expo-glass-effect` 57.0.1** — `apps/mobile/node_modules/expo-glass-effect/build/index.d.ts` and `src/`. `GlassView` off iOS is literally `<View {...props} />`; `isLiquidGlassAvailable()` and `isGlassEffectAPIAvailable()` both `return false` off iOS. `isLiquidGlassAvailable`'s own docstring points at `AccessibilityInfo.isReduceTransparencyEnabled()` for the accessibility case, and `isGlassEffectAPIAvailable`'s cites expo/expo#40911 — iOS 26 betas that ship the design without the API and crash on `GlassView`. Both predicates are therefore checked, not one.
- **React Native 0.86.2** — `node_modules/react-native/Libraries/StyleSheet/StyleSheetTypes.d.ts`. `experimental_backgroundImage` accepts a string (line 520); `boxShadow` accepts a string or `BoxShadowValue[]` and `BoxShadowValue.inset` is `boolean` (lines 343, 516); `fontVariant` includes `'tabular-nums'`; `fontWeight`'s union is `'100'..'900' | 100..900` and **has no 650**, which is what forces the 650 → 600 mapping.
- **`packages/tokens`** — `src/tokens.js`, `src/tokens.d.ts`, `scripts/build.mjs`, `test/tokens.test.mjs`, `package.json`. `bun test` runs the `node:test` suite unchanged (10 passing at plan time). The generator imports named exports explicitly, so new groups do not leak into the CSS unless they are added to it.
- **`apps/landing/package.json`** — the `"@anakmobil/tokens": "*"` precedent, and its resolved symlink `apps/landing/node_modules/@anakmobil/tokens -> ../../../../packages/tokens`. Verified at plan time that `apps/mobile` has **no** such link, which is why Task 2 Step 2 exists.
- **`@fontsource-variable/inter`** — checked and rejected for mobile: it ships 42 `.woff2` files and no TTF, and React Native cannot load woff2. Hence a separate font source for the app.
- **`Makefile`, `.github/workflows/mobile.yml`, `.github/workflows/frontend.yml`, `.prettierignore`** — the two gate chains, the two path filters, and the Prettier working-directory trap that made `fmt-check` a Make prerequisite.
- **Jira AM-15, AM-25 … AM-29** — the acceptance criteria and the five definitions of done, read live rather than paraphrased from the ticket titles.
- **`docs/design.md`** — §7, §8, §9, §11, §12, §13, §14, §15, §17, §29, §40–§48, §50–§53, §61, §66, §67, §69, §72, §73, §74, §76.

## Points where the repository forced a deviation from the spec

Recorded so a later reader can tell a considered departure from an oversight.

1. **The spec's scrim percentages are wrong and are corrected here.** 83.3 / 94.1 / 99.4 are `1 − L`; the real alphas are 55.4 / 73.1 / 93.3. **Every conclusion the spec drew from them survives** — this is a numerical correction, not a re-opened decision — and Task 10 fixes it at the source.
2. **`docs/design.md`'s own palette fails AA in nine places, before any glass.** Both tertiary text tokens, the white-on-orange CTA, and all four semantic colours used as text. AM-15 AC2 and AM-25's definition of done make repairing them part of this ticket, so Task 1 repairs them and Task 10 records them.
3. **`chrome` has no consumer in AM-15.** The app bar and the tab bar are `chrome` surfaces and neither is in this ticket's five subtasks. The role is defined because it is part of the token contract, and the catalogue renders one sample so it is exercised — but nothing navigational is built, and **that is why no blur library is installed**: `expo-blur` would be a dependency bought for a surface that does not exist yet. The app-shell story adds it when it has somewhere to put it.
4. **No grain on the ground.** The spec describes "a gradient plus fine grain". React Native has no asset-free noise short of a shader, and the anti-goals forbid an image asset. The gradient and the edge ship; the grain is deferred with a `ponytail:` note rather than faked with stacked radial gradients that would cost the Android performance the ladder exists to protect.
5. **`light.surface` is an off-white tint at 92%, not §41's subtle grey.** That grey at 92% gives secondary text 4.40:1 over a dark backdrop — a fail — and every darker light tint fails worse. This is the spec's own escape hatch being used as written.
6. **The mobile 650 weight renders at 600.** React Native's `fontWeight` union has no 650 and the shipped Inter cuts are static. The desktop scale is untouched.
7. **AM-26's "so iOS does not auto-zoom" is a web requirement.** A native React Native app has no auto-zoom — that is mobile Safari behaviour, and the wording came across from web guidance. The 16 px floor is implemented anyway, as a legibility rule. The number is identical; only the reason changes.
8. **AM-15's technical note "utamakan garis tepi sebelum bayangan" is partly superseded**, exactly as the spec says. It survives on `working` and is superseded on `chrome` and `surface`. Recorded rather than silently ignored, because the ticket and the spec disagree on the page and a reader deserves to know which won.
9. **The contrast maths lives in `packages/tokens`, not `apps/mobile`.** `apps/mobile` has no test runner and this ticket does not add one; `packages/tokens` already has one, its own CI job, and the values being asserted. Putting the one testable thing in the ticket where it can actually be tested is the whole reason the token architecture went this way.

---

## Creative direction for the implementing session

**The mandate, and its fence.** Be genuinely creative about *arrangement, edge, rhythm, density, and motion* — how the dock sits, how a service row breathes, where light lands on a card, what changes when a chip is selected. Do not be creative about the material roles (chrome / surface / working), the palette (graphite #1D232A, accent #ED491C, Inter), the 16px core radius, the 4px spacing grid, or the contrast contract (binding token = the *composited* colour that passes AA; percentages describe appearance and are never the promise). Those were argued to a conclusion and re-opening them costs more than it buys. Everything below is a starting value derived from shipped systems, not a preference — deviate where you have a measured reason, and write the reason next to the token.

### The one structural rule that makes the rest work

Build every glass object as **three stacked views inside one clipped container**, in this order:

1. `View` with `borderRadius` + `overflow: 'hidden'`, `backgroundColor` = the **opaque composited token**. This is the design.
2. *(optional)* `BlurView`, mounted here — above the fill, below the gradients — gated on `(Platform.OS === 'ios' || Platform.Version >= 31) && !reduceTransparency`. It never supplies colour. Removing it must change texture and nothing else.
3. `LinearGradient`, height 3, `rgba(255,255,255,α)` → transparent, pinned top.
4. `LinearGradient`, height 5, transparent → `rgba(0,0,0,β)`, pinned bottom.

Because the container clips on a radius, the top gradient follows the arc and fades out at the corners — which is what a real specular highlight does and what a uniform stroke conspicuously does not. The Android-without-blur path is then structurally identical to the iOS path minus one layer nobody can point to. **Build and screenshot the whole app with blur off first.** Turn it on and diff: if the reading order, the boundaries, or which text is legible changed, the blur was carrying meaning and the token is wrong.

Starting α/β, to be tuned against measured contrast, not by eye:

| | top highlight α | bottom shadow β |
|---|---|---|
| dark theme | `rgba(255,255,255,0.16)` | `rgba(0,0,0,0.24)` |
| light theme (tints dark) | `rgba(255,255,255,0.45)` | `rgba(29,35,42,0.14)` |

Light needs the stronger highlight because its surfaces sit *darker* than the ground; light-mode drop shadows are too weak to carry hierarchy, so the edge does the work there while dark can lean on luminance. AOSP ships two separately tuned states for exactly this reason (blur-on dim 0.1f, blur-off dim 0.4f — four times, not a rounding error). Ship each role twice: `chrome-blur` / `chrome-flat`, `surface-blur` / `surface-flat`, where the flat variant is a *higher-opacity composite with a stronger edge*, not the blur variant with the blur deleted. Default to flat; blur is the enhancement.

### The radius scale — derive it, don't reuse 16 everywhere

A single radius on every element is the single most recognisable generated-UI fingerprint. Keep 16 as the core and step outward on the 4-grid:

`AmBadge 8 · AmTextField 12 · AmButton 12 · AmCard 16 · dock 20 · AmBottomSheet 24 (top corners only) · AmChip full pill`

Spacing follows the same logic: tight inside a group (4/8), generous between groups (24/32). Never a uniform 16 gap down a screen. Service history and fitment tables get the tightest inner rhythm in the app — row cadence is their hierarchy.

### The dock (bottom navigation)

Float on **one axis only: lifted, not inset.** Every shipped 5-item bar does this; the narrow centred pill you see posted is a 3–4 item object.

- Height **56** = 8 top + 24 icon + 4 icon→label gap + 12 label line + 8 bottom.
- Side inset **8**, not 16. Radius 20.
- Bottom position: `bottom: Math.max(insets.bottom, 16)`, `marginBottom: 0`. The safe-area inset *absorbs* the float gap; it is never added to it. A 48dp three-button Android inset plus a fixed 16 puts the dock 64dp up and reads as broken.
- **Every scrollable screen** sets `contentContainerStyle.paddingBottom = 56 + Math.max(insets.bottom, 16) + 8`. This is the most common defect in every floating-dock implementation; without it the last service-history row is permanently under the dock.
- Mount the dock as a sibling **after** the FlatList/ScrollView in the tree, or iOS blur freezes on the first frame's backdrop.

**Why inset 8 and not 16 — the research disagreed with itself and this is the resolution.** At 360dp (the Android baseline that dominates Indonesia), 16px insets leave ~54px of label per item: "Komunitas" fits by about 6px and truncates the moment `fontScale` hits 1.15, which users set deliberately. At inset 8 the budget is ~62px and survives. Labels are mandatory and non-negotiable (Google's own M3 default hides labels at 4+ destinations — that is the honest counter-evidence, and it is wrong for an app whose destinations are not universal icons and whose first users have never seen them). So buy the label room from the horizontal axis, which was the cheap one.

**Active state carries three channels, none of them the label.** Icon outlined → filled; icon tint graphite → #ED491C; a flat capsule **48×32, radius 16** behind the icon only. Label stays graphite in both states. The capsule is a plain `View` filled with a resolved neutral step — **never a second BlurView** (glass cannot sample glass, and two RenderNode passes per frame on a 60Hz Android buys an effect nobody can see), never orange, never a glow. Colour alone would fail WCAG 1.4.1 and #ED491C against graphite is precisely the pair that collapses under deuteranopia; on the flat Android render the tint is the weakest of the three channels, which is why the capsule earns its cost.

**No minimize-on-scroll, no collapse, no scroll-driven translation.** iOS 26's version drops the labels this design mandates, and the RN port has open issues for it silently not firing under nested stacks. Static dock. If a scroll behaviour is wanted later, translate the whole dock down by its own height and back — one Reanimated value, labels intact.

### AmButton

Primary is a **working-role solid**: orange-9 (`#ED491C`) fill, height 48, radius 12, label at the on-accent token. Solid means pressable — that is the whole grammar, so nothing that is not pressable gets a solid orange fill anywhere in the app.

- **States are a step on a ladder, never a new effect.** Press = orange-10 (a resolved darker step). Build one `AmStateLayer` primitive — an absolutely-positioned `View`, `pointerEvents="none"`, inside the pressable's clipping radius, filled with the *content's own colour* at 8% hover / 10% pressed. Because it is the content colour it darkens light containers and lightens dark ones with one recipe. Fade 120ms.
- On **working-role solids only** those percentages may be literal, because the backdrop is known. On a chrome- or surface-role control the state layer must be a **resolved tint**, or press feedback changes strength depending on what the vehicle-tinted ground is doing behind it.
- **Disabled is a resolved per-theme token pair, never `opacity: 0.4`.** An alpha over a translucent material makes the final colour a function of the ground — the exact thing the binding token exists to prevent — and it fades the label by the same factor, which is how disabled starts looking like a failed render.
- **Loading**: keep the measured width (no reflow), swap the label for a spinner at the label's own colour, set `accessibilityState={{ busy: true }}`, stay focusable. Do not set `disabled`.
- Focus/selection ring on any chrome or surface backdrop is **two-tone**: the control keeps a 2px border in the theme-base tone, and a wrapper adds `padding: 2, borderWidth: 2, borderRadius: <inner + 4>` in the theme-*inverse* tone. 4px total, both tones present, one of which always clears 3:1 against an unknown backdrop. **Never orange alone** — the ground can be orange-ish once a red or orange vehicle drives the tint, and a single-tone ring is the failure case this technique was written to replace. No glow, ever: a blurred halo has no measurable contrast area, and RN Android renders elevation shadows black-only, so the state cue would simply be absent on much of the install base.

### AmCard

Surface role, 88–92% composited, radius 16. **Hold that number** — Apple would classify a card as content and forbid glass on it outright; 88–92% is what keeps ours honest, so do not let it drift down because a screenshot looked nicer at 80.

- Top-edge highlight + bottom inset shadow only. **No `borderWidth` on left, right, or bottom of any `Am*` component.** No `elevation`, no `shadowColor` on a surface that already carries the inset edge — pick edge *or* elevation, never both (hairline border + wide shadow is a named generated-UI signature). Watch for `elevation: 4` sneaking in as an Android-only fix: it draws a real four-sided shadow and silently re-adds the banned thing.
- **Anything inside a card is solid.** An `AmChip` on an `AmCard` is a tint of the card's already-composited colour, not a second translucent layer. Glass never samples glass; two translucent layers multiply and the AA-passing token stops being a constant.
- Weight cards by content, not by grid. The home screen's active vehicle is a full-bleed surface-role card; everything else is a list row. Three equal-weight "Garasi / Servis / Komunitas" tiles is the generated-landing-page composition wearing product clothes.
- Where a card scrolls under `AmAppBar`, put a **scroll edge effect** on the chrome, not on the list: a ~20px `LinearGradient` from the chrome's composited colour to transparent, mounted at the chrome's own edge. Hard style (fuller opacity) when the app bar carries a title or a pinned section header; soft for the dock. **One per screen.** This is what lets chrome stay genuinely translucent while a user scrolls a white service-invoice photo past it.

### AmChip and AmBadge

**AmChip** is a filter/selection control and changes three things at once, so it survives greyscale, colourblindness, and the flat render:

- unselected: transparent fill on its parent material + 1px border at step-7 + label at primary text;
- selected: resolved step-5 fill + border stepped to 8 + a **16px leading checkmark**.
- Orange is never the selected fill. Selected is a *state*; solid orange means *action*.

**AmBadge** — including the AI confidence badge — is **working role: solid, zero transparency**, minimum 11pt, and always carries an icon plus a Bahasa Indonesia label. Colour confirms the meaning; it never *is* the meaning. Semantic status colours stay clearly separate from #ED491C, which is an action colour.

**AmEmptyState** is a first-class screen, not a fallback: this platform launches with no data and says so. Design it at the same fidelity as the populated state — it is the first thing most users will see, and nothing is ever seeded to hide it.

### AmBottomSheet and AmAppBar

`AmBottomSheet` is the **strongest candidate in the set for genuine translucency** — it is transient and light-dismiss, which is the one duration budget every shipped system permits real translucency in. Radius 24, top corners only, with a **dimming layer beneath it** rather than a thicker material above.

When the sheet is open, **`AmAppBar` stops being glass for the duration** — one of the two goes solid. Glass on glass is a rendering fact, not taste, and on Android without blur three stacked translucent rectangles at similar values collapse into one ambiguous shape.

A row of chips or buttons sitting on `AmAppBar` renders as **one material with plain children drawn on top**, never N independent BlurViews. Apply the material to the control, not its inner views.

### The ground, and its hard limit

Two-stop linear ramp, one direction, lighter at the top so it agrees with the edge highlight's implied light source. The active vehicle's dominant colour contributes **hue only**: clamp saturation to ≤14%, pin luminance to a narrow band around #1D232A so the ramp's darkest and lightest points are known constants. No third stop, no radial halo, no mesh, no blobs.

An unclamped user-derived tint is an unbounded input: a bright yellow Jazz or a red Brio produces the saturated-glow look that reads as generated *and* moves the contrast floor, so the AA-passing token stops passing — for that user only, which is the worst possible failure shape. Derive the ground in code, then **assert every composited chrome/surface/working token against the ramp's lightest stop and fail the build if AA breaks.** Pin a hostile-colour test set: white, bright yellow, red, silver — the common colours on Indonesian roads, and precisely the ones that break a graphite tint.

### Anti-slop checklist — each entry checkable against a diff

| Tell | Check | Smaller alternative |
|---|---|---|
| Four-sided hairline stroke on a glass surface | `grep -n "borderWidth" src/components/Am*` — only the two-tone focus wrapper and AmChip may have one | top-edge gradient highlight + bottom inset shadow |
| Border *and* shadow on the same surface | any `elevation:` or `shadowColor:` on a component that already has the edge stack | pick one — here, always the edge |
| Coloured glow / accent shadow | `shadowColor` that is not ground graphite; any orange `shadowColor`, including a "subtle 4%" bloom on a focused field | 2px solid two-tone ring |
| Alpha as the token layer | `rgba(` literals in `src/components/**` | resolved opaque tokens; percentages live only in the derivation note |
| `opacity: 0.4` disabled | grep `opacity:` in components | pre-composited disabled token pair per theme |
| Nested glass | more than one `BlurView` in a subtree; `BlurView` supplying `backgroundColor` | one sampling container per plane; children solid |
| Blur treated as spec | `intensity={…}` cited in a design note or PR description | specify the composited colour; intensity is a per-platform detail that must *land on* it |
| One radius everywhere | grep radius values — the set must contain 8, 12, 16, 20, 24 and a pill | the derived scale above |
| Uniform 16 spacing down a screen | eyeball the styles: are inner and outer gaps the same number? | 4/8 inside a group, 24/32 between |
| Purple leak | `#6366F1`, `#8B5CF6`, `#A855F7`, any violet gradient, gradient text | #ED491C solid, on pressables only |
| Extra gradient stops on the ground | more than two stops, any `RadialGradient`, any decorative blob | 2-stop linear, hue-clamped |
| Tertiary grey on unknown backdrop | `#737D87` or any secondary token used on a chrome/surface component | working-role solids only |
| Decoration imitating function | a chip with no `onPress`, a pulsing dot on static data, a blinking caret on non-editable text, a marquee, bounce/elastic easing | animate only on a real state or data transition |
| Sparkle/AI iconography outside the AI surface | grep the AI icon import list | the AI mark appears on the AI answer and its evidence, nowhere else |
| Generated-landing composition | icon tile above heading, three equal cards in a row, all-caps eyebrow label, numbered 1-2-3 steps, emoji as a section marker | weight by content; delete the eyebrow, the heading says it |
| Happy-path-only | any of the 8 components missing loading / empty / error / full | all four states, at 320dp, with real Bahasa strings |
| Seeded data | invented counts, sample vehicles, fabricated testimonials | the empty state, designed as a primary experience |

### Using Dribbble and Behance honestly

Use them as a parts bin, never as a target. A shot is a static image composited once over a curated photograph; it has never had to survive a 93.3% scrim requirement, an Android below SDK 31 with no blur, `fontScale` 1.15, or a service record read on a phone in direct sun in a workshop. The composition is not portable; the *mechanics* sometimes are.

Search phrases worth running: `floating tab bar`, `frosted navigation bar mobile`, `car maintenance app ui`, `vehicle garage app`, `service history mobile`, `dark mode automotive dashboard`, `glass card dark ui`. Also search the words this design avoids — `glowing pill nav`, `glassmorphism dashboard` — precisely to build the reject pile.

**Steal:** how an active tab is marked; how a numeric-heavy row is set (alignment, tabular figures, where the unit sits); density rhythm in a list; how an edge catches light; how an empty state is composed.
**Discard on sight:** the blurred-photo backdrop the whole shot depends on, the purple, glass on content cards, four-sided white strokes, nested glass, centre FAB notches, animated specular sweeps, and any layout that only has a populated state.

Discipline: for every shot you save, write one line — *mechanic stolen, and why the rest was rejected*. If you cannot name the mechanic, do not save the shot. Paste those lines into the PR description; they are the record that the reference was interrogated rather than copied.

### What the owner reviews, and when

The **component catalogue is the review surface** — a real route inside the app, not a separate tooling install. It ships before any product screen is built, immediately after the tokens land.

It shows all eight components (`AmButton`, `AmCard`, `AmChip`, `AmBadge`, `AmTextField`, `AmEmptyState`, `AmBottomSheet`, `AmAppBar`) plus the dock, each in **loading / empty / error / full**, and it carries **two toggles at the top: theme (light/dark) and blur (on/off)**. The blur toggle is not a debug affordance — it is how the owner reviews the reference rendering, and it later becomes the user-facing "kurangi transparansi" setting wired to `AccessibilityInfo.isReduceTransparencyEnabled()` on iOS and `isHighTextContrastEnabled()` on Android. Apple needed that escape hatch within six weeks of shipping Liquid Glass; retrofitting one is far more expensive than designing it in.

Screenshots to hand over, all with **blur off first, then on**: light and dark; 320dp and 360dp; `fontScale` 1.0 and 1.15; and the ground at four hostile vehicle colours (white, bright yellow, red, silver). Plus the measured contrast table: every composited token against the ramp's lightest stop. A grey-scale screenshot of any screen must still sort into three roles by eye — if it does not, the tiering has collapsed into one glass token, which is the primary generated-UI tell.

### Three ways this fails with every rule followed

**It reads as decoration rather than as a tool.** Someone under a car with dirty hands wants to log 2.500 km and a filter part number and get out. If the glass, the tint, and the edges make that take longer or read slower than a plain list would, the material has become the product. The test: cover the chrome and ask whether the working-role screens are *faster* to read than a boring app. If they are only equal, delete something.

**It is beautiful on an iPhone and broken on an Android without blur.** This is the likeliest failure because it is the one you will not see — the dev device has blur. The guard is that the flat rendering is the reference build, reviewed and signed off first; if the two states differ in anything beyond texture, the design only ever existed on one platform.

**It is unreadable in the sun at a workshop.** Translucency's effective contrast is a function of what is behind it, and outdoor glare eats the low-contrast end of every ramp. That is why secondary and tertiary text never touch a material with an unknown backdrop, why status never rides on colour alone, and why AI safety warnings and confidence badges are solid working-role surfaces with no exceptions. If a warning about a brake fault is hard to read in daylight, none of the rest of this matters.

### Reconciliation with `docs/design.md` §14 and `packages/tokens`

Two values in the radius ladder above do not currently exist as tokens. Neither is a real disagreement, but both must be settled deliberately rather than discovered at implementation time — an off-scale radius is exactly the kind of small invention that turns a token system into a suggestion.

1. **`AmBottomSheet 24` is not a step.** The scale runs `xs 6 · sm 8 · md 12 · lg 16 · xl 20 · 2xl 28 · pill`, so 24 falls between `xl` and `2xl`. §14 does mention 24 in prose ("Hero/car imagery: 20–24 px"), which is where the number came from. **Decide one:** use `2xl` (28) for the sheet's top corners and drop 24 from the brief, or add a real `3xl`-style step to `packages/tokens/src/tokens.js` and `tokens.d.ts` together — the package has a test asserting the two stay in sync, so a new step is two edits and a passing test, not one. Prefer the first unless 28 measurably reads too soft at sheet width.

2. **`AmBadge 8` versus the token comment.** `packages/tokens/src/tokens.js` documents `pill` as being "for chips and badges", while the brief gives badges `sm` (8) and reserves the pill for chips. The brief's split is the better call — a numeric badge at pill radius on a dense spec row reads as a bubble and competes with the chip it sits beside — but the comment in the token source then says something untrue and must be corrected in the same change. A stale comment in the token file is worse than no comment, because it is the first thing the next reader trusts.

Everywhere else the ladder sits exactly on the existing scale: badge `sm`, text field and button `md`, card `lg`, dock `xl`, chip `pill`. That is deliberate — the ladder's purpose is to stop a single radius flattening the hierarchy, not to invent new numbers.
