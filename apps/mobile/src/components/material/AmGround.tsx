import { mix } from "@anakmobil/tokens/derive";
import type { ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { useTheme } from "@/theme";

export interface AmGroundProps {
  readonly children: ReactNode;
  /**
   * The dominant colour of the active vehicle, when there is one. A red car
   * gives a copper-tinted garage; a white car a cool one.
   *
   * MUST be a six-digit `#RRGGBB` hex string. `derive.js`'s `channels()`
   * throws on anything else, and that throw happens at the app root — so a
   * malformed value here is a startup crash, not a rendering glitch.
   *
   * WHERE THIS COMES FROM IS NOT BUILT HERE. Extraction is decided in the
   * garage epic, when vehicle photos actually exist. Absent, the ground falls
   * back to neutral graphite, which is the launch state.
   */
  readonly tint?: string;
}

/**
 * The bottom layer of the app: a graphite gradient, optionally tinted.
 *
 * Pure code — a gradient, no image asset, no blur, no photograph. That is
 * what makes it render identically on an iPhone and on an Android 10 phone,
 * and it is why the no-blur rendering of everything above it is a legitimate
 * variant rather than a degraded one.
 *
 * ponytail: the spec describes the ground as "a gradient plus fine grain".
 * The grain is deliberately not implemented — React Native has no asset-free
 * noise, and the anti-goals forbid an image asset. Add it with a shader if a
 * device check ever says the flat gradient bands.
 */
export function AmGround({ children, tint }: AmGroundProps) {
  const theme = useTheme();
  const { stops, tintStrength } = theme.ground;

  // Only the middle stop takes the vehicle colour. Tinting the ends washes
  // the whole screen and stops it reading as graphite.
  const middle = Math.floor(stops.length / 2);
  const gradient = stops
    .map((stop, index) => {
      const color = tint && index === middle ? mix(stop.color, tint, tintStrength) : stop.color;
      return `${color} ${Math.round(stop.at * 100)}%`;
    })
    .join(", ");

  return (
    // The flat fallback lives on the UNKEYED outer View (a plain colour change
    // repaints normally), so a platform that ignores the gradient shows
    // graphite rather than white, and nothing flashes mid-rebuild.
    <View style={[styles.fill, { backgroundColor: stops[middle].color }]}>
      <View
        // Remount the GRADIENT LAYER ONLY when the theme (or tint) changes:
        // RN 0.86's experimental_backgroundImage does not repaint when the
        // gradient string changes on an existing view. The key must never sit
        // on the view that wraps {children} — that would unmount the entire
        // app subtree and discard every screen's state on a theme switch.
        key={`${theme.name}-${tint ?? "neutral"}`}
        pointerEvents="none"
        style={[
          StyleSheet.absoluteFill,
          { experimental_backgroundImage: `linear-gradient(180deg, ${gradient})` },
        ]}
      />
      {children}
    </View>
  );
}

const styles = StyleSheet.create({
  fill: { flex: 1 },
});
