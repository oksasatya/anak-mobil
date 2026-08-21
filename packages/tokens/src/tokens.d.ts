/**
 * Types for the design tokens. Hand-written so `tokens.js` can stay
 * plain JavaScript that Node imports without a build step.
 *
 * Keep in sync with `tokens.js`. There is a test that fails if a key is
 * added to one and not the other.
 */

export declare const brand: {
  readonly 950: string;
  readonly 900: string;
  readonly 800: string;
  readonly 700: string;
  readonly 600: string;
};

export declare const accent: {
  readonly 700: string;
  readonly 600: string;
  readonly 500: string;
  readonly 400: string;
  readonly 300: string;
};

export declare const semantic: {
  readonly success: string;
  readonly warning: string;
  readonly danger: string;
  readonly info: string;
};

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
}

export declare const light: ThemeColors;
export declare const dark: ThemeColors;

export declare const spacing: Readonly<
  Record<1 | 2 | 3 | 4 | 5 | 6 | 8 | 10 | 12 | 16 | 20 | 24, number>
>;

export declare const radius: {
  readonly xs: number;
  readonly sm: number;
  readonly md: number;
  readonly lg: number;
  readonly xl: number;
  readonly "2xl": number;
  readonly pill: number;
};

export declare const typography: {
  readonly fontFamily: string;
  readonly numericFeature: string;
};

export declare const type: {
  readonly hero: string;
  readonly h1: string;
  readonly h2: string;
  readonly h3: string;
  readonly title: string;
  readonly body: string;
  readonly small: string;
};

export declare const typeMobile: Readonly<
  Record<
    "display" | "h1" | "h2" | "h3" | "title" | "body-lg" | "body" | "label" | "caption" | "micro",
    string
  >
>;

export declare const elevation: {
  readonly none: string;
  readonly soft: { readonly light: string; readonly dark: string };
};

export declare const motion: {
  readonly durationMicro: string;
  readonly durationStandard: string;
  readonly durationSheet: string;
  readonly easeOut: string;
};

export declare const layout: {
  readonly pagePaddingMobile: number;
  readonly pagePaddingMobileLarge: number;
  readonly containerMaxWidth: number;
  readonly touchTargetMin: number;
};

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

export declare const tokens: {
  readonly brand: typeof brand;
  readonly accent: typeof accent;
  readonly semantic: typeof semantic;
  readonly onAccent: typeof onAccent;
  readonly onGraphite: typeof onGraphite;
  readonly semanticText: typeof semanticText;
  readonly light: ThemeColors;
  readonly dark: ThemeColors;
  readonly material: typeof material;
  readonly edge: typeof edge;
  readonly ground: typeof ground;
  readonly spacing: typeof spacing;
  readonly radius: typeof radius;
  readonly typography: typeof typography;
  readonly type: typeof type;
  readonly typeMobile: typeof typeMobile;
  readonly elevation: typeof elevation;
  readonly motion: typeof motion;
  readonly layout: typeof layout;
};

export default tokens;
