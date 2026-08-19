import { Pressable, StyleSheet, Text, type ViewStyle } from "react-native";

import { useTheme } from "@/theme";

export interface AmChipProps {
  readonly label: string;
  readonly selected?: boolean;
  readonly onPress?: () => void;
  readonly disabled?: boolean;
  readonly style?: ViewStyle;
}

/**
 * §45's fast-selection control: Manual / Automatic / CVT / DCT, or
 * Daily / Track / Stance / Touring / Show.
 *
 * A chip is visually short but its hit area is not — §61 and AM-15 AC3 want
 * 44 pt, and hitSlop supplies the difference so a row of chips still looks
 * like a row of chips.
 */
export function AmChip({ label, selected = false, onPress, disabled = false, style }: AmChipProps) {
  const theme = useTheme();
  const pad = (theme.touchTargetMin - 32) / 2;

  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={{ selected, disabled }}
      accessibilityLabel={label}
      disabled={disabled || !onPress}
      onPress={onPress}
      hitSlop={{ top: pad, bottom: pad, left: 0, right: 0 }}
      style={({ pressed }) => [
        styles.chip,
        {
          minHeight: 32,
          paddingHorizontal: theme.space[3],
          borderRadius: theme.radius.pill,
          borderWidth: 1,
          // Selected is orange: §17 and AM-15 AC4 both name the selected
          // state as one of orange's four legitimate uses.
          borderColor: selected ? theme.color.accent : theme.color.border,
          backgroundColor: selected ? theme.color.accent : theme.color.surfaceSubtle,
          opacity: disabled ? 0.45 : pressed ? 0.82 : 1,
        },
        style,
      ]}
    >
      <Text
        style={[
          theme.type.label,
          { color: selected ? theme.color.onAccent : theme.color.textPrimary },
        ]}
        numberOfLines={1}
      >
        {label}
      </Text>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  chip: { alignItems: "center", justifyContent: "center" },
});
