import { StyleSheet, Text, View } from "react-native";

import { useTheme } from "@/theme";

import { strengthOf } from "./passwordScore";

export interface PasswordStrengthProps {
  readonly password: string;
}

// Re-exported for callers that import the score function alongside the
// component. The scoring logic itself lives in `./passwordScore` — a
// react-native-free file — so it stays unit-testable; see that file's header
// comment for why this component cannot host it directly (and why it isn't
// named as a case-variant of this file).
export { strengthOf } from "./passwordScore";

const LABEL: Record<0 | 1 | 2 | 3 | 4, string> = {
  0: "",
  1: "Terlalu pendek",
  2: "Cukup",
  3: "Bagus",
  4: "Kuat",
};

export function PasswordStrength({ password }: PasswordStrengthProps) {
  const theme = useTheme();
  const score = strengthOf(password);
  if (score === 0) return null;

  // A bar is a FILL, which is what `semantic` is for; the label is words,
  // which is what `semanticText` is for. Swapping them is the mistake the
  // theme's own comments warn about.
  const fill =
    score === 1
      ? theme.color.semantic.danger
      : score === 2
        ? theme.color.semantic.warning
        : theme.color.semantic.success;
  const words =
    score === 1
      ? theme.color.semanticText.danger
      : score === 2
        ? theme.color.semanticText.warning
        : theme.color.semanticText.success;

  return (
    <View style={{ gap: theme.space[2] }}>
      <View style={[styles.track, { gap: theme.space[1] }]}>
        {[1, 2, 3, 4].map((segment) => (
          <View
            key={segment}
            style={[
              styles.segment,
              {
                height: theme.space[1],
                borderRadius: theme.radius.xs,
                backgroundColor: segment <= score ? fill : theme.color.border,
              },
            ]}
          />
        ))}
      </View>
      <Text accessibilityLiveRegion="polite" style={[theme.type.caption, { color: words }]}>
        {LABEL[score]}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  track: { flexDirection: "row" },
  segment: { flex: 1 },
});
