import { Image } from "expo-image";
import { useState } from "react";
import { StyleSheet, Text, View } from "react-native";

import { brandLogosAreBranded, brandLogoUrl } from "@/features/catalog/brandLogo";
import { useTheme } from "@/theme";

export interface AmBrandLogoProps {
  /** The brand's primary domain, from `brand_logo_domain` on the wire. */
  readonly domain: string | null | undefined;
  /** What the tile falls back to — the car's name, for its first letter. */
  readonly name: string;
  readonly size?: number;
}

/**
 * A brand mark, with a letter tile behind it.
 *
 * Three states, and only one of them is a picture: a car with no catalog match
 * has no brand, a build with no Brandfetch client id has no CDN, and a request
 * that fails has no image. All three land on the same initial, which is why
 * the tile is always drawn and the image sits on top of it rather than
 * replacing it — there is no frame in which this component is a hole.
 *
 * Decorative. The name is already beside it in every mount, and a screen
 * reader announcing "Toyota" twice is worse than not announcing the logo.
 */
export function AmBrandLogo({ domain, name, size = 32 }: AmBrandLogoProps) {
  const theme = useTheme();
  const [failed, setFailed] = useState(false);
  const url = failed ? null : brandLogoUrl(domain, size);
  const initial = (name.trim()[0] ?? "?").toUpperCase();

  // How big to DRAW the image inside the tile, which is not the same as how
  // big the tile is.
  //
  // Brandfetch serves the mark at whatever size is asked for, so it fills the
  // tile. The favicon fallback does not: every keyless source — Google,
  // gstatic, unavatar, icon.horse, DuckDuckGo — returns 48x48 and ignores the
  // size parameter, so drawing one at 34pt on a 3x screen upscales it 2.1x and
  // it reads as a smear. Held to 20pt it is only 1.25x over native, which is
  // the difference between "small logo" and "broken logo". The tile keeps its
  // size either way, so nothing around it moves.
  const inner = brandLogosAreBranded() ? size : Math.min(size, 20);

  return (
    <View
      accessibilityElementsHidden
      importantForAccessibility="no"
      style={[
        styles.tile,
        {
          width: size,
          height: size,
          borderRadius: theme.radius.sm,
          backgroundColor: theme.color.surfaceSubtle,
          borderColor: theme.color.border,
        },
      ]}
    >
      {url ? null : (
        <Text style={[theme.type.label, { color: theme.color.textSecondary }]}>{initial}</Text>
      )}
      {url ? (
        <Image
          source={{ uri: url }}
          contentFit="contain"
          onError={() => setFailed(true)}
          // `cachePolicy` is the default already; naming it is the point — a
          // logo is immutable per brand, and re-fetching one on every render of
          // a list would be a request per row per scroll.
          cachePolicy="memory-disk"
          style={{ width: inner, height: inner }}
        />
      ) : null}
    </View>
  );
}

const styles = StyleSheet.create({
  tile: { alignItems: "center", justifyContent: "center", borderWidth: 1, overflow: "hidden" },
});
