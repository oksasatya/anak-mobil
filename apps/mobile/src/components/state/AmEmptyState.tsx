import Ionicons from "@expo/vector-icons/Ionicons";
import { StyleSheet, Text, View, type ViewStyle } from "react-native";

import { AmButton } from "@/components/input";
import { useTheme } from "@/theme";

export interface AmEmptyStateProps {
  readonly title: string;
  readonly body: string;
  /** Required, not optional. See the note below — this is a product rule. */
  readonly actionLabel: string;
  readonly onAction: () => void;
  /**
   * An Ionicons glyph, drawn in a soft disc above the title.
   *
   * The redesign specifies an illustration here. Illustration assets do not
   * exist yet, and a stretched placeholder box reads as a broken image — so
   * the disc carries the screen's own subject as a line glyph until real
   * artwork lands. It is decorative: the title and body already say
   * everything, so it is hidden from the screen reader.
   */
  readonly icon?: keyof typeof Ionicons.glyphMap;
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
export function AmEmptyState({
  title,
  body,
  actionLabel,
  onAction,
  icon,
  style,
}: AmEmptyStateProps) {
  const theme = useTheme();
  return (
    <View style={[styles.container, { padding: theme.space[6], gap: theme.space[3] }, style]}>
      {icon ? (
        <View
          accessibilityElementsHidden
          importantForAccessibility="no"
          style={[
            styles.disc,
            {
              backgroundColor: theme.color.surfaceSubtle,
              borderColor: theme.color.border,
              marginBottom: theme.space[1],
            },
          ]}
        >
          <Ionicons name={icon} size={38} color={theme.color.textTertiary} />
        </View>
      ) : null}
      <Text
        accessibilityRole="header"
        style={[theme.type.h3, styles.centered, { color: theme.color.textPrimary }]}
      >
        {title}
      </Text>
      <Text
        style={[
          theme.type.body,
          styles.centered,
          styles.measure,
          { color: theme.color.textSecondary },
        ]}
      >
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
  // The body is a paragraph, not a label: past ~310pt it runs to the screen
  // edge on a large phone and stops scanning as one block.
  measure: { maxWidth: 310 },
  disc: {
    width: 88,
    height: 88,
    borderRadius: 44,
    borderWidth: 1,
    alignItems: "center",
    justifyContent: "center",
  },
});
