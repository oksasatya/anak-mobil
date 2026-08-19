import {
  accent,
  brand,
  edge,
  ground,
  layout,
  material,
  motion,
  onAccent,
  onGraphite,
  radius,
  semantic,
  semanticText,
  spacing,
  dark as darkColors,
  light as lightColors,
} from "@anakmobil/tokens";

import { buildTypeScale } from "./typography";
import type { Theme, ThemeName } from "./types";

/** `"210ms"` -> `210`. Reanimated and Animated both want a number. */
function ms(value: string): number {
  return Number.parseInt(value, 10);
}

const typeScale = buildTypeScale();

function build(name: ThemeName): Theme {
  const colors = name === "dark" ? darkColors : lightColors;
  return {
    name,
    color: {
      ...colors,
      // Orange as TEXT: #ED491C is 4.64 on the dark surface but only 3.77 on
      // white, so the light theme steps down to accent-700 (5.27).
      accent: accent[500],
      accentText: name === "dark" ? accent[500] : accent[700],
      onAccent,
      // §42's default primary button is graphite, not orange — orange is the
      // "strongest brand CTA", used selectively.
      graphite: brand[800],
      onGraphite,
      semantic,
      semanticText: semanticText[name],
    },
    material: material[name],
    edge: edge[name],
    ground: ground[name],
    space: spacing,
    radius,
    type: typeScale,
    motion: {
      micro: ms(motion.durationMicro),
      standard: ms(motion.durationStandard),
      sheet: ms(motion.durationSheet),
    },
    touchTargetMin: layout.touchTargetMin,
    pagePadding: layout.pagePaddingMobile,
  };
}

export const lightTheme = build("light");
export const darkTheme = build("dark");
export const themes: Record<ThemeName, Theme> = { light: lightTheme, dark: darkTheme };
