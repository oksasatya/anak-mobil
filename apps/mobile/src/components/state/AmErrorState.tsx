import Ionicons from "@expo/vector-icons/Ionicons";
import { StyleSheet, Text, View, type ViewStyle } from "react-native";

import { AmButton } from "@/components/input";
import { useTheme } from "@/theme";

export interface AmErrorStateProps {
  readonly title: string;
  readonly body: string;
  readonly onRetry: () => void;
  readonly retryLabel?: string;
  /**
   * An Ionicons glyph above the title, drawn in the danger tone. Decorative:
   * the title already names the failure, so it is hidden from the screen
   * reader rather than announced as a second, wordless error.
   */
  readonly icon?: keyof typeof Ionicons.glyphMap;
  readonly style?: ViewStyle;
}

/**
 * §53: direct, useful, non-technical. "Something went wrong" is named in the
 * document as the bad version; the good version says what failed, reassures
 * about the data, and offers the retry.
 *
 * The tone marker is `semanticText.danger`, never the raw `semantic.danger`,
 * which is 3.79:1 as text on the dark surface.
 */
export function AmErrorState({
  title,
  body,
  onRetry,
  retryLabel = "Coba lagi",
  icon,
  style,
}: AmErrorStateProps) {
  const theme = useTheme();
  return (
    <View style={[styles.container, { padding: theme.space[6], gap: theme.space[3] }, style]}>
      {icon ? (
        <Ionicons
          accessibilityElementsHidden
          importantForAccessibility="no"
          name={icon}
          size={30}
          color={theme.color.semanticText.danger}
        />
      ) : null}
      <Text
        accessibilityRole="header"
        style={[theme.type.h3, styles.centered, { color: theme.color.semanticText.danger }]}
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
        label={retryLabel}
        onPress={onRetry}
        variant="secondary"
        style={{ marginTop: theme.space[2] }}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  container: { alignItems: "center", justifyContent: "center" },
  centered: { textAlign: "center" },
  // Matches AmEmptyState: past ~300pt the paragraph runs to the screen edge.
  measure: { maxWidth: 300 },
});
