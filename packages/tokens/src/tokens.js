/**
 * AnakMobil design tokens — the single source of truth.
 *
 * Every value here comes from `docs/design.md`. Nothing is invented, and
 * nothing is duplicated: the landing page, the backoffice, and the mobile
 * app all read from this file, directly or through a generated artifact.
 *
 * Plain JavaScript rather than TypeScript on purpose. Node imports it
 * natively so the generator needs no build step and no dependencies, and
 * `tokens.d.ts` gives TypeScript consumers full types anyway.
 *
 * Regenerate the CSS artifacts after editing:
 *
 *     cd packages/tokens && node scripts/build.mjs
 */

/**
 * Graphite. The product's ground, and about 85% of any screen.
 *
 * `800` is the brand colour proper — the one on the logo and on dark
 * surfaces. The lighter steps are for borders and raised layers in dark
 * mode, not for text.
 */
export const brand = {
  950: "#0F141A",
  900: "#151B22",
  800: "#1D232A",
  700: "#2B323B",
  600: "#3C4550",
};

/**
 * AnakMobil Orange. Roughly 10% of a screen — no more.
 *
 * Reserved for the primary action, the selected state, an AI highlight,
 * and genuinely important markers. It is NOT a status colour: success,
 * warning, danger, and info have their own values below, and using orange
 * for any of them makes both meanings unreadable.
 */
export const accent = {
  700: "#C93413",
  600: "#DC3E17",
  500: "#ED491C",
  400: "#F45C32",
  300: "#FF805E",
};

/**
 * Status colours. The remaining ~5%.
 *
 * `success` carries a specific product rule: it means *verified*. An AI
 * answer that is merely high-confidence must not wear it, because green
 * reads as "checked" and that would make the confidence badge a lie.
 */
export const semantic = {
  success: "#168A52",
  warning: "#D58A00",
  danger: "#D63B3B",
  info: "#2678D9",
};

/** Light theme surfaces, text, and borders. */
export const light = {
  background: "#F7F8FA",
  surface: "#FFFFFF",
  surfaceSubtle: "#F1F3F5",
  surfaceRaised: "#FFFFFF",
  textPrimary: "#171C22",
  textSecondary: "#5D6670",
  textTertiary: "#8A939D",
  border: "#E3E6E9",
  borderStrong: "#CDD2D7",
};

/** Dark theme surfaces, text, and borders. */
export const dark = {
  background: "#0E1217",
  surface: "#151A20",
  surfaceSubtle: "#202730",
  surfaceRaised: "#1B2128",
  textPrimary: "#F5F7F8",
  textSecondary: "#AAB2BA",
  textTertiary: "#737D87",
  border: "#29313A",
  borderStrong: "#3A434E",
};

/**
 * Spacing scale, in pixels. Base unit is 4.
 *
 * Page padding is 16 on phones and 20 on larger phones; the desktop
 * container caps at 1280.
 */
export const spacing = {
  1: 4,
  2: 8,
  3: 12,
  4: 16,
  5: 20,
  6: 24,
  8: 32,
  10: 40,
  12: 48,
  16: 64,
  20: 80,
  24: 96,
};

/**
 * Corner radii, in pixels.
 *
 * Cards use `lg` (16). Buttons use `md` (12). Vehicle imagery uses `xl`
 * (20) through `2xl` (28). `pill` is for chips and badges.
 */
export const radius = {
  xs: 6,
  sm: 8,
  md: 12,
  lg: 16,
  xl: 20,
  "2xl": 28,
  pill: 999,
};

/**
 * Typography.
 *
 * Inter, chosen for legibility of dense spec data on a phone. Numeric
 * values — `18×8.5 ET40`, `225/40 R18`, `146,120 KM` — should be set with
 * tabular figures so columns of them line up.
 */
export const typography = {
  fontFamily: [
    "Inter",
    "ui-sans-serif",
    "system-ui",
    "-apple-system",
    "BlinkMacSystemFont",
    '"Segoe UI"',
    "sans-serif",
  ].join(", "),
  numericFeature: '"tnum" 1, "lnum" 1',
};

/**
 * Composite type tokens — the `font` shorthand minus the family, which the
 * generator appends as `var(--font-sans)`. Used as `font: var(--type-hero);`.
 *
 * Two scales, per docs/design.md §11–12. Weights 650 and 750 are why the
 * font must be the variable cut — no static Inter provides them.
 */
export const type = {
  hero: "750 56px/64px",
  h1: "700 44px/52px",
  h2: "700 36px/44px",
  h3: "650 28px/36px",
  title: "650 20px/28px",
  body: "400 16px/25px",
  small: "400 14px/21px",
};

/** Mobile type scale — docs/design.md §12. Emitted as `--type-m-*`. */
export const typeMobile = {
  display: "700 32px/38px",
  h1: "700 28px/34px",
  h2: "700 24px/30px",
  h3: "650 20px/26px",
  title: "650 18px/24px",
  "body-lg": "400 16px/24px",
  body: "400 14px/21px",
  label: "600 13px/18px",
  caption: "400 12px/17px",
  micro: "500 11px/14px",
};

/**
 * Elevation — borders before shadows (docs/design.md §15). Default surfaces
 * carry a 1px border and no shadow; `soft` is reserved for genuinely floating
 * things (a dropdown, a bottom sheet, a floating waitlist card) and is
 * theme-dependent — heavier in dark so it reads on a near-black ground.
 */
export const elevation = {
  none: "none",
  soft: {
    light: "0 8px 24px rgba(15,20,26,0.10)",
    dark: "0 12px 32px rgba(0,0,0,0.45)",
  },
};

/** Motion — docs/design.md §50. Short and functional; no racing effects. */
export const motion = {
  durationMicro: "150ms",
  durationStandard: "210ms",
  durationSheet: "280ms",
  easeOut: "cubic-bezier(0.2,0,0,1)",
};

/** Layout constants that are not spacing steps. */
export const layout = {
  pagePaddingMobile: 16,
  pagePaddingMobileLarge: 20,
  containerMaxWidth: 1280,
  /** Minimum touch target, per the accessibility rules in AM-273. */
  touchTargetMin: 44,
};

export const tokens = {
  brand,
  accent,
  semantic,
  light,
  dark,
  spacing,
  radius,
  typography,
  type,
  typeMobile,
  elevation,
  motion,
  layout,
};

export default tokens;
