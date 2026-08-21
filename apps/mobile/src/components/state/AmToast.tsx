import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { AccessibilityInfo, Platform, StyleSheet, Text } from "react-native";
import Animated, { FadeInDown, FadeOutDown } from "react-native-reanimated";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmMaterial } from "@/components/material";
import type { AmBadgeTone } from "@/components/display";
import { useTheme } from "@/theme";

export interface AmToastMessage {
  readonly message: string;
  readonly tone?: AmBadgeTone;
}

type ShowToast = (toast: AmToastMessage) => void;

const ToastContext = createContext<ShowToast | null>(null);

const VISIBLE_MS = 3200;

export function useToast(): ShowToast {
  const show = useContext(ToastContext);
  if (!show) throw new Error("useToast must be used inside <ToastProvider>");
  return show;
}

export interface ToastProviderProps {
  readonly children: ReactNode;
}

/**
 * One toast at a time, replaced rather than stacked.
 *
 * A stack needs an ordering policy, an exit choreography, and a maximum —
 * none of which anything in this app has asked for. When two things happen
 * at once the second is the one worth reading.
 *
 * The toast is `working`: solid. It is a message read to make a decision, it
 * appears over arbitrary content, and its tone is carried by a border and a
 * `semanticText` colour rather than by a saturated fill (§61).
 */
export function ToastProvider({ children }: ToastProviderProps) {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const [toast, setToast] = useState<AmToastMessage | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const show = useCallback<ShowToast>((next) => {
    if (timer.current) clearTimeout(timer.current);
    setToast(next);
    // accessibilityLiveRegion only reaches TalkBack; VoiceOver needs an
    // explicit announcement or the toast is silent on iOS.
    if (Platform.OS === "ios") AccessibilityInfo.announceForAccessibility(next.message);
    timer.current = setTimeout(() => {
      setToast(null);
      timer.current = null;
    }, VISIBLE_MS);
  }, []);

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );

  const tone = toast?.tone ?? "neutral";
  const accentColor = tone === "neutral" ? theme.color.borderStrong : theme.color.semantic[tone];
  const textColor = tone === "neutral" ? theme.color.textPrimary : theme.color.semanticText[tone];

  return (
    <ToastContext.Provider value={show}>
      {children}
      {toast ? (
        <Animated.View
          entering={FadeInDown.duration(theme.motion.standard)}
          exiting={FadeOutDown.duration(theme.motion.micro)}
          pointerEvents="none"
          style={[
            styles.host,
            { bottom: insets.bottom + theme.space[6], paddingHorizontal: theme.pagePadding },
          ]}
        >
          <AmMaterial
            role="working"
            radius="md"
            style={{
              paddingVertical: theme.space[3],
              paddingHorizontal: theme.space[4],
              borderLeftWidth: 3,
              borderLeftColor: accentColor,
            }}
          >
            <Text
              accessibilityLiveRegion="polite"
              accessibilityRole="alert"
              style={[theme.type.body, { color: textColor }]}
            >
              {toast.message}
            </Text>
          </AmMaterial>
        </Animated.View>
      ) : null}
    </ToastContext.Provider>
  );
}

const styles = StyleSheet.create({
  host: { position: "absolute", left: 0, right: 0 },
});
