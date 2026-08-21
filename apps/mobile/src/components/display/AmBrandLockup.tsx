import { Image } from "expo-image";
import { useEffect, useState } from "react";
import { AccessibilityInfo, Animated, Easing, Text, View } from "react-native";

import { useTheme } from "@/theme";

export interface AmBrandLockupProps {
  /**
   * `launch` is the larger, centred form for the launch screen. `header` is the
   * smaller, left-aligned form the auth screens wear.
   */
  readonly variant?: "launch" | "header";
  /** Play the entrance. Only the launch screen wants it. */
  readonly animate?: boolean;
}

/**
 * The real brand mark, with the tile removed so it floats on whatever ground it
 * sits on.
 *
 * Derived from `@anakmobil/assets/img/favicon-{dark,light}.png`, whose artwork
 * is baked onto a rounded tile: the tile is cropped away past its outer rim,
 * then the mark is separated from it by luminance with a soft ramp so the
 * artwork's antialiased edges survive. Two files rather than one tinted file —
 * the mark is a light garage with an orange road on dark, and a graphite garage
 * with an orange road on light, so a flat tint would destroy it.
 *
 * These stay in the app rather than in `@anakmobil/assets` because they are
 * derived, not an official export. Replace them the moment the brand ships a
 * transparent mark of its own.
 */
const MARK_ON_DARK = require("../../../assets/images/brand-mark.png") as number;
const MARK_ON_LIGHT = require("../../../assets/images/brand-mark-light.png") as number;

const LAUNCH_MARK = 88;
const HEADER_MARK = 46;

/**
 * The mark and the wordmark, side by side.
 *
 * On launch the mark is already on screen — the native splash drew it — so the
 * entrance animates only the wordmark: it fades up and slides in from the
 * mark's edge, which is what makes the app look like it is *resolving* rather
 * than cutting from a splash to a form. The mark itself never moves, because
 * the wordmark occupies its layout slot from the first frame and only its
 * opacity and offset change.
 *
 * `useNativeDriver` on both, so the entrance runs on the UI thread and is not
 * competing with the session bootstrap that is happening at the same moment.
 * `prefers-reduced-motion` is honoured: it resolves instantly instead.
 */
export function AmBrandLockup({ variant = "header", animate = false }: AmBrandLockupProps) {
  const theme = useTheme();
  const size = variant === "launch" ? LAUNCH_MARK : HEADER_MARK;

  // `useState` with a lazy initialiser, not `useRef(...).current` — the React
  // Compiler is enabled for this app and rejects a ref read during render
  // ("Cannot access refs during render"). This creates the value exactly once
  // and is safe to read while rendering.
  const [progress] = useState(() => new Animated.Value(animate ? 0 : 1));
  const [reduceMotion, setReduceMotion] = useState(false);

  useEffect(() => {
    let alive = true;
    void AccessibilityInfo.isReduceMotionEnabled().then((on) => {
      if (alive) setReduceMotion(on);
    });
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    if (!animate) return;
    if (reduceMotion) {
      progress.setValue(1);
      return;
    }
    Animated.timing(progress, {
      toValue: 1,
      duration: 420,
      delay: 120,
      easing: Easing.out(Easing.cubic),
      useNativeDriver: true,
    }).start();
  }, [animate, reduceMotion, progress]);

  return (
    <View
      accessible
      accessibilityRole="image"
      accessibilityLabel="AnakMobil.id"
      style={{
        flexDirection: "row",
        alignItems: "center",
        justifyContent: variant === "launch" ? "center" : "flex-start",
        gap: theme.space[3],
      }}
    >
      <Image
        source={theme.name === "dark" ? MARK_ON_DARK : MARK_ON_LIGHT}
        contentFit="contain"
        style={{ width: size, height: size }}
      />
      <Animated.View
        style={{
          opacity: progress,
          transform: [
            { translateX: progress.interpolate({ inputRange: [0, 1], outputRange: [-14, 0] }) },
          ],
        }}
      >
        <Text
          accessibilityElementsHidden
          importantForAccessibility="no"
          // A cap, NOT `allowFontScaling={false}` — that is banned, and rightly:
          // large system text must reflow. But a wordmark is a brand mark rather
          // than body text, and at the largest accessibility size an uncapped
          // 32px display face wrapped to two lines and took two thirds of the
          // screen, pushing the form somebody actually came for below the fold.
          // 1.4 still grows it to ~45px for anyone who needs that; it just stops
          // the logo from becoming the interface.
          maxFontSizeMultiplier={1.4}
          style={[theme.type.display, { color: theme.color.textPrimary, letterSpacing: -0.5 }]}
        >
          AnakMobil
          <Text style={{ color: theme.color.accentText }}>.id</Text>
        </Text>
      </Animated.View>
    </View>
  );
}
