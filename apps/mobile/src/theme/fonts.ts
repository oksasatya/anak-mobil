import { useFonts } from "expo-font";

// Deep imports, not the package barrel: the barrel re-exports all 18 static
// cuts (~6.1 MB) though only these four (~1.35 MB) are ever registered, and
// Metro does not tree-shake unused named exports out of a require() barrel.
import { Inter_400Regular } from "@expo-google-fonts/inter/400Regular";
import { Inter_500Medium } from "@expo-google-fonts/inter/500Medium";
import { Inter_600SemiBold } from "@expo-google-fonts/inter/600SemiBold";
import { Inter_700Bold } from "@expo-google-fonts/inter/700Bold";

/**
 * The four Inter cuts the mobile scale actually uses.
 *
 * docs/design.md §11 lists weights 400, 500, 600, 650, and 700 for mobile.
 * 650 has no static cut and React Native's `fontWeight` does not accept it
 * (the type union is '100'..'900' and 100..900 — 650 is not a member), so
 * the two 650 steps, H3 and Title, render at 600. Recorded in
 * docs/design.md §11 rather than left as a silent substitution. The desktop
 * scale keeps 650 and 750 because the web ships the variable cut.
 */
export const FONT_FAMILY = {
  400: "Inter_400Regular",
  500: "Inter_500Medium",
  600: "Inter_600SemiBold",
  700: "Inter_700Bold",
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
    Inter_400Regular,
    Inter_500Medium,
    Inter_600SemiBold,
    Inter_700Bold,
  });
  // A font that will not load must not hold the app at a blank splash screen
  // forever. The system font is a worse look, not a broken one.
  return loaded || error !== null;
}
