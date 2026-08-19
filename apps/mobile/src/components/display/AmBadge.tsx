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
