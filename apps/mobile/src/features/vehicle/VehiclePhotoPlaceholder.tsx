import { StyleSheet, Text } from "react-native";

import { AmMaterial } from "@/components/material";
import { useTheme } from "@/theme";

export interface VehiclePhotoPlaceholderProps {
  readonly caption?: string;
}

/**
 * docs/design.md §48: a car with no photo gets a neutral placeholder, never a
 * stock car that would imply the wrong model — and AM-113's technical note
 * says the same thing in the ticket's own words.
 *
 * There is no silhouette asset in packages/assets, so the neutral form is a
 * themed frame with a caption. That is genuinely neutral, adds no dependency,
 * and cannot mislead. A real silhouette is an owner-supplied file; when one
 * exists it drops in here and nothing else changes.
 *
 * The frame is `AmMaterial role="working"` with nothing added: `working` is
 * solid on every rung, and its default edge already draws the 1px border in
 * `theme.color.border` at `theme.radius.lg`. Re-declaring either in the style
 * below would be a second source of truth for the same edge.
 */
export function VehiclePhotoPlaceholder({
  caption = "Belum ada foto",
}: VehiclePhotoPlaceholderProps) {
  const theme = useTheme();
  return (
    <AmMaterial role="working" radius="lg" style={styles.frame}>
      <Text style={[theme.type.label, styles.centered, { color: theme.color.textSecondary }]}>
        {caption}
      </Text>
    </AmMaterial>
  );
}

const styles = StyleSheet.create({
  // 16:9, the shape a vehicle photo will occupy when one can be uploaded.
  frame: { aspectRatio: 16 / 9, alignItems: "center", justifyContent: "center" },
  centered: { textAlign: "center" },
});
