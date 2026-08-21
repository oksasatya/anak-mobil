import { useState } from "react";
import {
  StyleSheet,
  Text,
  TextInput,
  View,
  type KeyboardTypeOptions,
  type TextInputProps,
  type ViewStyle,
} from "react-native";

import { useTheme } from "@/theme";

export interface AmTextFieldProps {
  readonly label: string;
  readonly value: string;
  readonly onChangeText: (value: string) => void;
  readonly placeholder?: string;
  readonly hint?: string;
  readonly error?: string;
  readonly disabled?: boolean;
  readonly secureTextEntry?: boolean;
  readonly keyboardType?: KeyboardTypeOptions;
  readonly autoCapitalize?: TextInputProps["autoCapitalize"];
  readonly autoCorrect?: boolean;
  /** Password-manager and keyboard hints. "email" | "username" | "new-password" | "current-password". */
  readonly autoComplete?: TextInputProps["autoComplete"];
  readonly textContentType?: TextInputProps["textContentType"];
  readonly maxLength?: number;
  readonly style?: ViewStyle;
}

/**
 * docs/design.md §44: height 48-52, radius 12, neutral border, and the label
 * is ALWAYS visible — structured automotive data must never rely on a
 * placeholder that vanishes the moment someone types.
 *
 * The input text is `body-lg` (16px). AM-26's definition of done frames this
 * as stopping iOS auto-zoom; a native React Native app has no auto-zoom (that
 * is mobile Safari behaviour), so here 16 is a legibility floor rather than a
 * zoom guard. The number is the same either way.
 */
export function AmTextField({
  label,
  value,
  onChangeText,
  placeholder,
  hint,
  error,
  disabled = false,
  secureTextEntry,
  keyboardType,
  autoCapitalize,
  autoCorrect,
  autoComplete,
  textContentType,
  maxLength,
  style,
}: AmTextFieldProps) {
  const theme = useTheme();
  const [focused, setFocused] = useState(false);

  const borderColor = error
    ? theme.color.semantic.danger
    : focused
      ? theme.color.accent
      : theme.color.border;

  return (
    <View style={[{ gap: theme.space[2] }, style]}>
      <Text style={[theme.type.label, { color: theme.color.textSecondary }]}>{label}</Text>
      <TextInput
        accessibilityLabel={label}
        accessibilityHint={error ?? hint}
        editable={!disabled}
        value={value}
        onChangeText={onChangeText}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        placeholder={placeholder}
        placeholderTextColor={theme.color.textTertiary}
        secureTextEntry={secureTextEntry}
        keyboardType={keyboardType}
        autoCapitalize={autoCapitalize}
        autoCorrect={autoCorrect}
        autoComplete={autoComplete}
        textContentType={textContentType}
        maxLength={maxLength}
        style={[
          theme.type["body-lg"],
          styles.input,
          {
            minHeight: 52,
            paddingHorizontal: theme.space[4],
            borderRadius: theme.radius.md,
            borderWidth: 1,
            borderColor,
            // A form is `working`: solid, read to make a decision.
            backgroundColor: theme.material.working.solid,
            color: theme.color.textPrimary,
            opacity: disabled ? 0.45 : 1,
          },
        ]}
      />
      {error ? (
        <Text
          accessibilityLiveRegion="polite"
          style={[theme.type.caption, { color: theme.color.semanticText.danger }]}
        >
          {error}
        </Text>
      ) : hint ? (
        <Text style={[theme.type.caption, { color: theme.color.textTertiary }]}>{hint}</Text>
      ) : null}
    </View>
  );
}

const styles = StyleSheet.create({
  input: { textAlignVertical: "center" },
});
