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
  "display" | "h1" | "h2" | "h3" | "title" | "body-lg" | "body" | "label" | "caption" | "micro";

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
  readonly semantic: {
    readonly success: string;
    readonly warning: string;
    readonly danger: string;
    readonly info: string;
  };
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
