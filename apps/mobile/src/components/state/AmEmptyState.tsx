import { StyleSheet, Text, View, type ViewStyle } from "react-native";

import { AmButton } from "@/components/input";
import { useTheme } from "@/theme";

export interface AmEmptyStateProps {
  readonly title: string;
  readonly body: string;
  /** Required, not optional. See the note below — this is a product rule. */
  readonly actionLabel: string;
  readonly onAction: () => void;
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
export function AmEmptyState({ title, body, actionLabel, onAction, style }: AmEmptyStateProps) {
  const theme = useTheme();
  return (
    <View style={[styles.container, { padding: theme.space[6], gap: theme.space[3] }, style]}>
      <Text
        accessibilityRole="header"
        style={[theme.type.h3, styles.centered, { color: theme.color.textPrimary }]}
      >
        {title}
      </Text>
      <Text style={[theme.type.body, styles.centered, { color: theme.color.textSecondary }]}>
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
});
