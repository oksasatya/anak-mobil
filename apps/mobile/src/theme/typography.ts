import { typeMobile } from "@anakmobil/tokens";
import { parseTypeShorthand } from "@anakmobil/tokens/derive";
import type { TextStyle } from "react-native";

import { FONT_FAMILY, resolveWeight } from "./fonts";
import type { TypeName } from "./types";

/**
 * Automotive spec data — `18x8.5 ET40`, `225/40 R18`, `146,120 KM`,
 * `Rp 14.500.000` (docs/design.md §12). Tabular figures make a column of
 * these line up; without them a list of mileages is visually ragged and
 * genuinely harder to scan.
 *
 * Spread onto a Text style, never used alone: it carries no size or weight.
 */
export const numeric: TextStyle = { fontVariant: ["tabular-nums"] };

/**
 * The mobile type scale as React Native text styles.
 *
 * `fontFamily` carries the weight because that is how a static-cut font
 * family works on both platforms; `fontWeight` is set alongside it so that
 * the system font still renders at roughly the right weight if Inter fails
 * to load.
 */
export function buildTypeScale(): Record<TypeName, TextStyle> {
  // Bound through an explicit Record<TypeName, string> rather than mapped
  // straight off `typeMobile` — tsc otherwise checks the fromEntries() cast
  // below against a bare string index signature and never verifies that
  // typeMobile's actual keys cover every TypeName. This assignment is the
  // check: it fails to compile the moment the two key sets drift.
  const source: Readonly<Record<TypeName, string>> = typeMobile;
  const entries = Object.entries(source).map(([name, shorthand]) => {
    const { fontWeight, fontSize, lineHeight } = parseTypeShorthand(shorthand);
    const cut = resolveWeight(fontWeight);
    const style: TextStyle = {
      fontFamily: FONT_FAMILY[cut],
      fontWeight: String(cut) as TextStyle["fontWeight"],
      fontSize,
      lineHeight,
    };
    return [name, style];
  });
  return Object.fromEntries(entries) as Record<TypeName, TextStyle>;
}
