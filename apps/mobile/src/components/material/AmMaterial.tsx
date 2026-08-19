import { withAlpha } from "@anakmobil/tokens/derive";
import { GlassView } from "expo-glass-effect";
import type { ReactNode } from "react";
import { View, type StyleProp, type ViewStyle } from "react-native";

import {
  useMaterialCapability,
  useTheme,
  type MaterialRole,
  type TextRole,
  type Theme,
} from "@/theme";

export interface AmMaterialProps {
  readonly role: MaterialRole;
  readonly children: ReactNode;
  readonly radius?: keyof Theme["radius"];
  /** The glass edge. Off for a flush surface inside another material. */
  readonly edge?: boolean;
  /** StyleProp, not bare ViewStyle: callers (AmCard) legitimately pass style arrays. */
  readonly style?: StyleProp<ViewStyle>;
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
