import { useFonts } from "expo-font";

// Deep imports, not the package barrel: the barrel re-exports all 14 static
// cuts (~4.8 MB) though only these four (~1.4 MB) are ever registered, and
// Metro does not tree-shake unused named exports out of a require() barrel.
import { PlusJakartaSans_400Regular } from "@expo-google-fonts/plus-jakarta-sans/400Regular";
import { PlusJakartaSans_500Medium } from "@expo-google-fonts/plus-jakarta-sans/500Medium";
import { PlusJakartaSans_600SemiBold } from "@expo-google-fonts/plus-jakarta-sans/600SemiBold";
import { PlusJakartaSans_700Bold } from "@expo-google-fonts/plus-jakarta-sans/700Bold";

/**
 * The four Plus Jakarta Sans cuts the mobile scale actually uses.
 *
 * Plus Jakarta Sans (Tokotype) replaced Inter as the UI typeface in the AM-50
 * redesign. It was commissioned for Jakarta's city identity, so it carries
 * local character while keeping the neutral geometric forms dense spec data
 * needs — and its lining figures line up under `fontVariant: tabular-nums`
 * exactly as Inter's did, which is what `numeric` in ./typography.ts relies on.
 *
 * docs/design.md §11 lists weights 400, 500, 600, 650, and 700 for mobile.
 * 650 has no static cut and React Native's `fontWeight` does not accept it
 * (the type union is '100'..'900' and 100..900 — 650 is not a member), so
 * the two 650 steps, H3 and Title, render at 600. The substitution is the
 * same one Inter needed and is recorded in docs/design.md §11 rather than
 * left silent. Plus Jakarta Sans ships no variable cut through
 * `@expo-google-fonts` at all — only the 14 static faces — so this is not a
 * choice that a different import would undo.
 */
export const FONT_FAMILY = {
  400: "PlusJakartaSans_400Regular",
  500: "PlusJakartaSans_500Medium",
  600: "PlusJakartaSans_600SemiBold",
  700: "PlusJakartaSans_700Bold",
} as const;

export type FontWeightKey = keyof typeof FONT_FAMILY;

/**
 * Map a design weight onto a cut that exists. 650 -> 600 is the only
 * substitution and it is the one documented above.
 */
export function resolveWeight(weight: number): FontWeightKey {
  if (weight >= 700) return 700;
  if (weight >= 600) return 600;
  if (weight >= 500) return 500;
  return 400;
}

/** `true` once the fonts are ready — or have failed, in which case the system font stands in. */
export function useAppFonts(): boolean {
  const [loaded, error] = useFonts({
    PlusJakartaSans_400Regular,
    PlusJakartaSans_500Medium,
    PlusJakartaSans_600SemiBold,
    PlusJakartaSans_700Bold,
  });
  // A font that will not load must not hold the app at a blank splash screen
  // forever. The system font is a worse look, not a broken one.
  return loaded || error !== null;
}
