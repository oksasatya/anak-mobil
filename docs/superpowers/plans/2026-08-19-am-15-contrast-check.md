# AM-15 — Recorded verification results (AM-29)

Recorded 2026-08-19/20 by the controller session executing
`2026-08-19-am-15-glass-material-plan.md`, on the iOS Simulator. The Android
half of this document is Task 9's and is appended by the OWNER after the
physical-device check; AM-15 is not called done before that section exists.

Simulator: **iPhone 17, iOS 26.5** (UDID 597C2C27-…). Capability reported by
the app: **`liquid-glass`** (both `expo-glass-effect` predicates true), with
the catalogue's *Paksa tanpa blur* switch exercising the `tint` rung live.

Screenshots (in `am-15-checks/`, captured at 2.286 px/pt):

| File | State |
|---|---|
| `light-glass-top.png` | light theme · liquid-glass · toggles + Material + Kontras |
| `light-glass-tampilan.png` | light · cards, chips, badges, avatars, skeleton, empty state |
| `light-buttons-inputs.png` | light · button variants incl. Small(36pt)/Large(52pt), four text-field states |
| `light-sheet.png` | light · `AmBottomSheet` open (scrim, grabber, "Tutup", options) |
| `light-toast.png` | light · toast "Contoh pemberitahuan" on solid `working` |
| `light-tint-top.png` | light · **forced tint** — "Material aktif: tint" |
| `dark-glass-top.png` | dark · liquid-glass — dark ground, all contrast rows green |
| `dark-tint-top.png` | dark · forced tint |
| `dark-largetext-specdata.png` | dark · accessibility-XXXL type — spec data reflowing, no clipping |
| `dark-tint-STALE-GROUND-BUG.png` | evidence of the defect found and fixed during this pass (see below) |

## Contrast table as rendered (catalogue "Kontras" section, live `contrastRatio`)

All rows rendered **green (≥ 4.5:1)** in both themes, and every value matches
Task 1's computed table exactly.

| Pair | Light | Dark |
|---|---|---|
| primary / working | 17.13:1 | 16.28:1 |
| secondary / working | 5.83:1 | 8.15:1 |
| tertiary / working | 5.49:1 | 5.97:1 |
| primary / surface | 16.81:1 | 16.44:1 |
| secondary / surface | 5.72:1 | 8.23:1 |
| primary / chrome | 16.54:1 | 17.49:1 |
| onAccent / accent | 4.91:1 | 4.91:1 |

## Touch targets

Method: rendered heights measured from the 2.286 px/pt screenshots,
cross-checked against the code-enforced minimums (`minHeight`/`hitSlop` on the
pressable itself — no caller padding anywhere in the catalogue).

| Control | Visual | Effective hit area |
|---|---|---|
| `AmButton` sm | ≈36 pt (84 px) | 36 + 2×4 hitSlop = **44 pt** |
| `AmButton` md | 44 pt | **44 pt** |
| `AmButton` lg | ≈52 pt (118 px) | **52 pt** |
| `AmChip` | 32 pt | 32 + 2×6 hitSlop = **44 pt** |
| `AmSelect` trigger | 52 pt | **52 pt** |
| Sheet option rows | ≥44 pt (`minHeight: touchTargetMin`) | **≥44 pt** |
| Sheet "Tutup" button | 44×44 (`minWidth`/`minHeight`) | **44 pt** |

## Interaction checks (all on-simulator, light theme)

- Toast appears on `working` (solid) with neutral left border, bottom-anchored
  above the safe area, and auto-dismisses (~3.2 s). Replaces, never stacks.
- `AmSelect` opens `AmBottomSheet` (never a native picker); selecting closes
  the sheet; reopening shows the selected row with **`textPrimary` label + a
  trailing orange ✓ glyph** (the fix-pass form — selection is not colour-alone).
- Sheet dismisses by **drag past ~96 pt** (verified), by "Tutup", and by scrim tap.
- Chip selection flips live: selected = orange fill + `onAccent` label.
- Theme switch and *Paksa tanpa blur* switch both flip the whole catalogue
  **live, with no reload**; "Material aktif" reads `liquid-glass` / `tint`
  accordingly.

## Large text (AC on §61)

`content_size accessibility-extra-extra-extra-large`: every section reflows;
headings and labels wrap instead of clipping; the automotive spec rows
(`18×8.5 ET40` · `225/40 R18` · `146,120 KM` · `Rp 14.500.000`) remain fully
readable; nothing scrolls sideways; no `allowFontScaling={false}` exists in
the app (grep-verified).

## Reduced transparency

- The **rendering path** is verified: the catalogue's *Paksa tanpa blur*
  switch drives the identical `'tint'` branch of `useMaterialCapability()`,
  and every surface switches to its `solid` composite live (screenshots
  `*-tint-top.png`).
- The **OS-listener path** (`AccessibilityInfo.isReduceTransparencyEnabled` +
  `reduceTransparencyChanged`) could **not** be toggled from `simctl` (the
  `com.apple.Accessibility` plist poke does not register on iOS 26.5), so the
  live Settings-toggle check is **deferred to Task 9's owner device pass**
  (Settings → Accessibility → Display & Text Size → Reduce Transparency).
  Recorded honestly rather than claimed.

## Defect found by this pass, and its fix

**The navigation container's background covered `AmGround` entirely.**
Expo Router (SDK 56+ vendored navigation) paints its theme's
`colors.background` — `rgb(242, 242, 242)` — behind every screen;
`contentStyle: transparent` clears only the screen layer, not the container.
Symptom: in dark theme the whole app sat on a light grey page, and dark-theme
headings were nearly invisible — exactly AM-15 AC2's "element lost against the
background" clause. Found only by opening the screen; every static gate was
green.

Fix (in `apps/mobile/src/app/_layout.tsx`): wrap the `Stack` in expo-router's
**own** `ThemeProvider` (`DarkTheme`/`DefaultTheme` are re-exported by
`expo-router` — SDK 56+ dropped react-navigation, so `@react-navigation/native`
must NOT be added; it was tried and produced the router's own incompatibility
error, then removed) with `colors.background: "transparent"`, keeping
`contentStyle: transparent`. `AmGround` additionally re-keys on
`theme.name`/`tint` so the `experimental_backgroundImage` gradient is rebuilt
on theme change (the first frame resolves light before `useColorScheme`
settles).

After the fix: dark ground renders from cold start and follows the live theme
toggle; all dark-theme headings readable (screenshot `dark-glass-top.png`).

## Second and third defects, found by the final independent review

**The first ground fix keyed the wrong element.** `key={theme.name}` sat on
the View wrapping `{children}`, so every theme change unmounted the entire app
subtree — typed text, select value, chip selection, and scroll position were
discarded. Fixed: the key moved to an absolute `pointerEvents="none"`
gradient-only layer; children keep their state. Re-verified live: flipping the
system appearance while the catalogue was open re-themed everything, repainted
the ground, and preserved the selected chip and scroll position
(`light-after-live-flip-state-preserved.png`).

**The dark ground broke the healthcheck screen — and this branch owns the
break.** `index.tsx` (AM-14) relied on RN's default black text, which was
readable on the old light page and **1.12:1** — invisible, not merely
low-contrast — on the new dark ground. An earlier revision of this document
filed that as "not AM-15's"; the final reviewer correctly overruled the
attribution: the screen was readable before this branch. Fixed by theming the
screen's four text colours through `useTheme()` (`textPrimary`,
`semanticText.success`/`danger`, `accentText`); layout and copy stay AM-14's.
Dark render verified readable.

---

## Android device check (Task 9 — OWNER, to be appended)

Pending. Required: device model + OS version; banding yes/no; inset-shadow
rendering yes/no; no-blur legibility verdict; scroll smoothness; outdoor
`working`-role legibility; one screenshot.
