import Ionicons from "@expo/vector-icons/Ionicons";
import { StyleSheet, Text, View } from "react-native";

import { useTheme } from "@/theme";

export type FormNoticeTone = "danger" | "warning" | "info";

export interface FormNoticeProps {
  readonly tone: FormNoticeTone;
  readonly message: string;
}

const GLYPH: Record<FormNoticeTone, keyof typeof Ionicons.glyphMap> = {
  danger: "alert-circle",
  warning: "time",
  info: "information-circle",
};

/**
 * A one-line form-level message, under the fields and above the button.
 *
 * The glyph is decorative and hidden from the screen reader — the message
 * says everything, and a wordless icon announced beside it is noise. What the
 * glyph does carry is §53's rule that meaning never rides on colour alone: a
 * red line and an amber line are the same line to a person who cannot tell
 * them apart, and the shape distinguishes them.
 *
 * `semanticText`, never the raw `semantic` fill. `semantic.danger` is 3.79:1
 * as text on the dark surface and fails AA at this size.
 */
export function FormNotice({ tone, message }: FormNoticeProps) {
  const theme = useTheme();
  const color = theme.color.semanticText[tone];

  return (
    <View
      accessibilityLiveRegion="polite"
      accessibilityRole={tone === "danger" ? "alert" : undefined}
      style={[styles.row, { gap: theme.space[2] }]}
    >
      <Ionicons
        accessibilityElementsHidden
        importantForAccessibility="no"
        name={GLYPH[tone]}
        size={14}
        color={color}
        style={styles.glyph}
      />
      <Text style={[theme.type.caption, styles.text, { color }]}>{message}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "flex-start" },
  // The 14pt glyph sits on the caption's cap height rather than its baseline.
  glyph: { marginTop: 1 },
  text: { flex: 1 },
});
