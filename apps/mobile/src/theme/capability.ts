import { isGlassEffectAPIAvailable, isLiquidGlassAvailable } from "expo-glass-effect";
import { createContext, useContext, useEffect, useState } from "react";
import { AccessibilityInfo } from "react-native";

/**
 * Which rung of the platform ladder this device is on.
 *
 *   'liquid-glass'  iOS 26+, native Liquid Glass through expo-glass-effect
 *   'tint'          everything else: tint + edge, no blur
 *
 * There is no third rung, and that is deliberate. expo-blur on Android
 * defaults to blurMethod: 'none', which renders a semi-transparent tint and
 * is not a blur; a real blur needs Android 12+ and costs a dependency for a
 * surface this ticket does not build. So 'tint' IS the design, not a
 * degradation — the ground is a gradient and the edge does the shaping, both
 * of which render identically with or without blur.
 */
export type MaterialCapability = "liquid-glass" | "tint";

interface CapabilityControl {
  /** The catalogue's "force no blur" switch — the Android < 31 reality on demand. */
  readonly forceTint: boolean;
  readonly setForceTint: (force: boolean) => void;
}

export const CapabilityControlContext = createContext<CapabilityControl | null>(null);

// Hoisted so a caller with no provider gets the same referentially-stable
// object on every render, instead of a fresh one that invalidates memoisation.
const NO_CONTROL: CapabilityControl = { forceTint: false, setForceTint: () => {} };

export function useCapabilityControl(): CapabilityControl {
  // Deliberately tolerant: outside the catalogue there is no control, and a
  // primitive must not crash because nobody offered it a switch.
  return useContext(CapabilityControlContext) ?? NO_CONTROL;
}

export function useMaterialCapability(): MaterialCapability {
  const { forceTint } = useCapabilityControl();
  const [reduceTransparency, setReduceTransparency] = useState(false);

  useEffect(() => {
    let alive = true;
    // The listener may fire before the initial promise resolves; once it
    // has, the promise's resolution is stale and must not overwrite it.
    let settled = false;
    // iOS-only setting; resolves false on Android, which is the right answer
    // there because Android has no system-wide transparency switch.
    AccessibilityInfo.isReduceTransparencyEnabled()
      .then((value) => {
        if (alive && !settled) setReduceTransparency(value);
      })
      .catch(() => {
        if (alive && !settled) setReduceTransparency(false);
      });
    const sub = AccessibilityInfo.addEventListener("reduceTransparencyChanged", (value) => {
      settled = true;
      setReduceTransparency(value);
    });
    return () => {
      alive = false;
      sub.remove();
    };
  }, []);

  if (forceTint || reduceTransparency) return "tint";
  // Both predicates: isLiquidGlassAvailable() reports the design is active,
  // isGlassEffectAPIAvailable() guards the iOS 26 betas that ship the design
  // without the API and crash on GlassView (expo/expo#40911).
  return isLiquidGlassAvailable() && isGlassEffectAPIAvailable() ? "liquid-glass" : "tint";
}
