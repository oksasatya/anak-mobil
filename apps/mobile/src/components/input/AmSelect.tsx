import { useState } from "react";
import { Pressable, StyleSheet, Text, View, type ViewStyle } from "react-native";

import { AmBottomSheet } from "@/components/display";
import { useTheme } from "@/theme";

export interface AmSelectOption<T extends string> {
  readonly value: T;
  readonly label: string;
}

export interface AmSelectProps<T extends string> {
  readonly label: string;
  readonly value: T | null;
  readonly options: readonly AmSelectOption<T>[];
  readonly onChange: (value: T) => void;
  readonly placeholder?: string;
  readonly disabled?: boolean;
  readonly style?: ViewStyle;
}

/**
 * A select opens a bottom sheet, never a native picker or dialog.
 *
 * §45 lists the bottom-sheet picker as the pattern for vehicle specs, and
 * AM-27's definition of done makes it a requirement: every picker and filter
 * in the app goes through AmBottomSheet.
 */
export function AmSelect<T extends string>({
  label,
  value,
  options,
  onChange,
  placeholder = "Pilih",
  disabled = false,
  style,
}: AmSelectProps<T>) {
  const theme = useTheme();
  const [open, setOpen] = useState(false);
  const selected = options.find((option) => option.value === value);

  return (
    <View style={[{ gap: theme.space[2] }, style]}>
      <Text style={[theme.type.label, { color: theme.color.textSecondary }]}>{label}</Text>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={label}
        accessibilityValue={{ text: selected?.label ?? placeholder }}
        accessibilityState={{ disabled, expanded: open }}
        disabled={disabled}
        onPress={() => setOpen(true)}
        style={({ pressed }) => [
          styles.trigger,
          {
            minHeight: 52,
            paddingHorizontal: theme.space[4],
            borderRadius: theme.radius.md,
            borderWidth: 1,
            borderColor: theme.color.border,
            backgroundColor: theme.material.working.solid,
            opacity: disabled ? 0.45 : pressed ? 0.82 : 1,
          },
        ]}
      >
        <Text
          style={[
            theme.type["body-lg"],
            { color: selected ? theme.color.textPrimary : theme.color.textTertiary },
          ]}
          // One-line summary of the current value; the sheet below shows the
          // full label, so truncation here is deliberate, not a bug.
          numberOfLines={1}
        >
          {selected?.label ?? placeholder}
        </Text>
      </Pressable>

      <AmBottomSheet visible={open} onClose={() => setOpen(false)} title={label}>
        <View accessibilityRole="radiogroup">
          {options.map((option) => {
            const isSelected = option.value === value;
            return (
              <Pressable
                key={option.value}
                accessibilityRole="radio"
                accessibilityState={{ selected: isSelected }}
                onPress={() => {
                  onChange(option.value);
                  setOpen(false);
                }}
                style={({ pressed }) => [
                  styles.option,
                  {
                    minHeight: theme.touchTargetMin,
                    paddingHorizontal: theme.space[4],
                    borderRadius: theme.radius.sm,
                    backgroundColor: pressed ? theme.color.surfaceSubtle : "transparent",
                  },
                ]}
              >
                <Text style={[theme.type["body-lg"], { color: theme.color.textPrimary }]}>
                  {option.label}
                </Text>
                {isSelected ? (
                  <Text style={[theme.type["body-lg"], { color: theme.color.accentText }]}>
                    {"✓"}
                  </Text>
                ) : null}
              </Pressable>
            );
          })}
        </View>
      </AmBottomSheet>
    </View>
  );
}

const styles = StyleSheet.create({
  trigger: { justifyContent: "center" },
  option: { flexDirection: "row", alignItems: "center", justifyContent: "space-between" },
});
