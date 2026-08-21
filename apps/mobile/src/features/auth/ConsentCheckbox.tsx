import { Pressable, StyleSheet, Text, View } from "react-native";

import { useTheme } from "@/theme";

export interface ConsentCheckboxProps {
  readonly checked: boolean;
  readonly onChange: (checked: boolean) => void;
  readonly label: string;
}

/**
 * The design system has no checkbox — `components/display` ships Card, Chip,
 * Badge, Avatar, BottomSheet; `components/input` ships Button, TextField,
 * Select. React Native has no checkbox element either. Built from theme
 * tokens, following `AmSelect`'s precedent of using an ARIA-style role for a
 * control the platform does not provide.
 *
 * Promote to `AmCheckbox` when a second consumer appears — not before.
 */
export function ConsentCheckbox({ checked, onChange, label }: ConsentCheckboxProps) {
  const theme = useTheme();
  return (
    <Pressable
      accessibilityRole="checkbox"
      accessibilityState={{ checked }}
      accessibilityLabel={label}
      onPress={() => onChange(!checked)}
      // The whole row is the target, not the 24pt box — a legal consent that
      // takes three attempts to tick is a consent nobody read.
      style={({ pressed }) => [
        styles.row,
        {
          minHeight: theme.touchTargetMin,
          gap: theme.space[3],
          opacity: pressed ? 0.82 : 1,
        },
      ]}
    >
      <View
        style={[
          styles.box,
          {
            width: theme.space[6],
            height: theme.space[6],
            borderRadius: theme.radius.xs,
            borderWidth: 1,
            borderColor: checked ? theme.color.accent : theme.color.borderStrong,
            backgroundColor: checked ? theme.color.accent : "transparent",
          },
        ]}
      >
        {checked ? (
          <Text style={[theme.type.label, { color: theme.color.onAccent }]}>{"✓"}</Text>
        ) : null}
      </View>
      <Text style={[theme.type.body, styles.label, { color: theme.color.textSecondary }]}>
        {label}
      </Text>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center" },
  box: { alignItems: "center", justifyContent: "center" },
  label: { flex: 1 },
});
