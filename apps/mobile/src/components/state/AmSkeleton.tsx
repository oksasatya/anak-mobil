import { useEffect } from "react";
import { type DimensionValue, type ViewStyle } from "react-native";
import Animated, {
  useAnimatedStyle,
  useSharedValue,
  withRepeat,
  withTiming,
} from "react-native-reanimated";

import { useTheme, type MaterialRole, type Theme } from "@/theme";

export interface AmSkeletonProps {
  readonly width?: DimensionValue;
  readonly height?: number;
  readonly radius?: keyof Theme["radius"];
  /**
   * The material of the component this is standing in for. Documentation for
   * the caller only: `surfaceSubtle` currently reads correctly on both roles
   * in both themes, so the value does not change the rendered fill.
   */
  readonly role?: Extract<MaterialRole, "surface" | "working">;
  readonly style?: ViewStyle;
}

/**
 * §51's loading placeholder.
 *
 * Drawn on the SAME material as the component it replaces, so a loading card
 * does not shimmer against a backdrop that swallows it. Opacity pulse rather
 * than a sweeping gradient: a shimmer needs a moving mask, and the anti-goals
 * rule out animated glass and per-item effects on long lists.
 */
export function AmSkeleton({ width = "100%", height = 16, radius = "sm", style }: AmSkeletonProps) {
  const theme = useTheme();
  const pulse = useSharedValue(0.45);

  useEffect(() => {
    pulse.value = withRepeat(withTiming(0.9, { duration: 700 }), -1, true);
  }, [pulse]);

  const animated = useAnimatedStyle(() => ({ opacity: pulse.value }));

  return (
    <Animated.View
      accessibilityElementsHidden
      importantForAccessibility="no-hide-descendants"
      style={[
        {
          width,
          height,
          borderRadius: theme.radius[radius],
          backgroundColor: theme.color.surfaceSubtle,
        },
        animated,
        style,
      ]}
    />
  );
}
