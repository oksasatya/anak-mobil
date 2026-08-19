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
      hitSlop={Math.max(0, (theme.touchTargetMin - HEIGHT[size]) / 2)}
      style={({ pressed }) => [
        styles.base,
        {
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
        // After `style`, not before: a caller's style must never be able to
        // silently defeat the accessibility floor.
        { minHeight: HEIGHT[size], minWidth: theme.touchTargetMin },
      ]}
    >
      {loading ? (
        <View style={[styles.row, { gap: theme.space[2] }]}>
          <ActivityIndicator color={fill.label} size="small" />
          <Text style={[theme.type.label, { color: fill.label }]}>{label}</Text>
        </View>
      ) : (
        <Text style={[theme.type.label, { color: fill.label }]}>{label}</Text>
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
