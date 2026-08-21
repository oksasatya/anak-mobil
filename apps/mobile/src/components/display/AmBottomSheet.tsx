import Ionicons from "@expo/vector-icons/Ionicons";
import type { ReactNode } from "react";
import { useEffect } from "react";
import { Modal, Pressable, StyleSheet, Text, View } from "react-native";
import { Gesture, GestureDetector, GestureHandlerRootView } from "react-native-gesture-handler";
import Animated, {
  runOnJS,
  useAnimatedStyle,
  useSharedValue,
  withTiming,
} from "react-native-reanimated";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmMaterial } from "@/components/material";
import { useTheme } from "@/theme";

export interface AmBottomSheetProps {
  readonly visible: boolean;
  readonly onClose: () => void;
  readonly title: string;
  readonly children: ReactNode;
}

/** Past this many points of downward drag, releasing dismisses. */
const DISMISS_AT = 96;

/**
 * The sheet every picker and filter in the app goes through.
 *
 * `Modal` is used only as a transparent host for the overlay — the sheet
 * itself is our own view, so this is not the "native dialog" §45 and AM-27
 * rule out. Closable by dragging down and by the button, both required.
 *
 * Its material is `surface`: a sheet is a container, and containers are
 * glass. Anything inside it that is read to make a decision uses `working`.
 */
export function AmBottomSheet({ visible, onClose, title, children }: AmBottomSheetProps) {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const translateY = useSharedValue(0);

  useEffect(() => {
    if (visible) translateY.value = 0;
  }, [visible, translateY]);

  const pan = Gesture.Pan()
    .onChange((event) => {
      // Downward only — dragging up must not detach the sheet from the edge.
      // Reanimated's SharedValue.value is a mutable ref by design, not a
      // React Compiler violation; eslint-config-expo has no Reanimated-aware
      // exception for it, and this repo adds no new eslint dependency for one.
      // eslint-disable-next-line react-hooks/immutability
      translateY.value = Math.max(0, translateY.value + event.changeY);
    })
    .onEnd(() => {
      if (translateY.value > DISMISS_AT) {
        // Same SharedValue-is-a-mutable-ref justification as above, not a
        // repeat of the same violation.
        // eslint-disable-next-line react-hooks/immutability
        translateY.value = withTiming(600, { duration: theme.motion.sheet }, (finished) => {
          // `finished` is false when the reopen-reset effect cancels this
          // animation mid-flight; only a completed dismiss should close.
          if (finished) runOnJS(onClose)();
        });
      } else {
        translateY.value = withTiming(0, { duration: theme.motion.standard });
      }
    });

  const sheetStyle = useAnimatedStyle(() => ({ transform: [{ translateY: translateY.value }] }));

  if (!visible) return null;

  return (
    <Modal visible transparent statusBarTranslucent animationType="slide" onRequestClose={onClose}>
      <GestureHandlerRootView style={styles.host}>
        <Pressable
          accessibilityElementsHidden
          importantForAccessibility="no"
          style={[styles.scrim, { backgroundColor: theme.edge.scrim }]}
          onPress={onClose}
        />
        <GestureDetector gesture={pan}>
          <Animated.View style={sheetStyle}>
            <AmMaterial
              role="surface"
              radius="2xl"
              style={{
                paddingHorizontal: theme.pagePadding,
                paddingTop: theme.space[3],
                paddingBottom: insets.bottom + theme.space[5],
                gap: theme.space[4],
              }}
            >
              <View
                accessibilityElementsHidden
                importantForAccessibility="no"
                style={[styles.grabber, { backgroundColor: theme.color.borderStrong }]}
              />
              {/*
                A glyph in the CORNER, not the word "Tutup" beside the title.
                Two reasons it left the title's line: the title is the sheet's
                subject and a second element on that row competed with it, and
                a vehicle name is long — "Tambah ke Toyota Avanza 1.5 Veloz AT"
                ran into the control. Absolute, so the title keeps the full
                width and the corner stays put however many lines the title
                takes. The label stays on the accessibility node, so a screen
                reader still announces "Tutup"; `hitSlop` carries the 32pt disc
                to the 44pt touch floor.
              */}
              <Pressable
                accessibilityRole="button"
                accessibilityLabel="Tutup"
                onPress={onClose}
                hitSlop={6}
                style={({ pressed }) => [
                  styles.close,
                  {
                    top: theme.space[3],
                    right: theme.pagePadding,
                    borderRadius: theme.radius.pill,
                    backgroundColor: theme.color.surfaceSubtle,
                    opacity: pressed ? 0.7 : 1,
                  },
                ]}
              >
                <Ionicons name="close" size={18} color={theme.color.textSecondary} />
              </Pressable>
              <Text
                style={[
                  theme.type.h3,
                  // Clear of the corner control, so a long vehicle name wraps
                  // before it reaches the X rather than under it.
                  { color: theme.color.textPrimary, paddingRight: theme.space[10] },
                ]}
              >
                {title}
              </Text>
              {children}
            </AmMaterial>
          </Animated.View>
        </GestureDetector>
      </GestureHandlerRootView>
    </Modal>
  );
}

const styles = StyleSheet.create({
  host: { flex: 1, justifyContent: "flex-end" },
  scrim: StyleSheet.absoluteFill,
  grabber: { alignSelf: "center", width: 36, height: 4, borderRadius: 2 },
  close: {
    position: "absolute",
    zIndex: 1,
    width: 32,
    height: 32,
    alignItems: "center",
    justifyContent: "center",
  },
});
