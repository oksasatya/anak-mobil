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

export declare const tokens: {
  readonly brand: typeof brand;
  readonly accent: typeof accent;
  readonly semantic: typeof semantic;
  readonly light: ThemeColors;
  readonly dark: ThemeColors;
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
