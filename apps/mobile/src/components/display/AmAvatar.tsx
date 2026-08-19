import { Image } from "expo-image";
import { StyleSheet, Text, View, type ImageStyle, type ViewStyle } from "react-native";

import { useTheme } from "@/theme";

export interface AmAvatarProps {
  readonly name: string;
  readonly uri?: string;
  readonly size?: number;
  readonly style?: ViewStyle;
}

/** Up to two letters, which is what reads at 40 pt. */
function initials(name: string): string {
  return name
    .trim()
    .split(/\s+/)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("");
}

/**
 * §47's 1:1 community avatar. Solid, never glass — an avatar over a variable
 * backdrop stops being a recognisable face.
 *
 * With no photo it falls back to initials on a neutral fill rather than a
 * stock silhouette of a person, matching §48's reasoning for vehicles: a
 * placeholder must not imply something that is not true.
 */
export function AmAvatar({ name, uri, size = 40, style }: AmAvatarProps) {
  const theme = useTheme();
  const shape: ViewStyle = {
    width: size,
    height: size,
    borderRadius: theme.radius.pill,
    borderWidth: 1,
    borderColor: theme.color.border,
    backgroundColor: theme.color.surfaceSubtle,
  };

  if (uri) {
    return (
      <Image
        source={{ uri }}
        accessibilityLabel={name}
        contentFit="cover"
        // RN's ImageStyle is `ViewStyle` minus a stricter `overflow` union;
        // `shape` and the caller-supplied `style` never set `overflow`, so
        // this cast is exact at runtime, not a type escape hatch.
        style={[shape, style] as ImageStyle[]}
      />
    );
  }

  return (
    <View
      accessibilityRole="image"
      accessibilityLabel={name}
      style={[styles.fallback, shape, style]}
    >
      <Text style={[theme.type.label, { color: theme.color.textSecondary }]}>{initials(name)}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  fallback: { alignItems: "center", justifyContent: "center" },
});
