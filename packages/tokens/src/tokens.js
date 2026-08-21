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
  // Repaired 2026-08-19 (AM-15). The documented #8A939D reached only 3.12:1
  // on white and 2.80:1 on surfaceSubtle — below the large-text floor, let
  // alone AA. #616A74 gives 5.49 and 4.94. docs/design.md §7/§66 updated.
  textTertiary: "#616A74",
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
  // Repaired 2026-08-19 (AM-15). The documented #737D87 reached 4.18:1 on
  // surface and 3.87:1 on surfaceRaised. #8E98A2 gives 5.97 and 5.53.
  textTertiary: "#8E98A2",
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
    // #E45B5B clears dark.working at 4.95 but reaches only 4.26 on
    // surfaceSubtle, where a badge or a toast sits as often as on the base
    // surface. #EC6363 is the same hue two steps lighter: 4.68 on
    // surfaceSubtle, 5.44 on working, matching its siblings' margins.
    danger: "#EC6363",
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
 * The MIDDLE stop is the theme's own background, and it is the backdrop each
 * material role's `solid` is composited over: it is the colour most of the
 * screen actually is, where the ends are the gradient's lighter and darker
 * extremes. packages/tokens/test/material.test.mjs pins that relationship.
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

export default tokens;
