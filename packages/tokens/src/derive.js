/**
 * Values you compute from the tokens rather than store.
 *
 * Plain JavaScript with a hand-written `.d.ts`, matching `tokens.js` — Node
 * and Bun import it with no build step, and React Native imports it through
 * Metro unchanged.
 *
 * The contrast maths is the WCAG 2.x definition, not an approximation. It is
 * here rather than in a dependency because it is nine lines and because the
 * material system's whole contract is asserted against it: a library that
 * rounds differently would move a surface across the AA boundary silently.
 */

const HEX = /^#[0-9A-Fa-f]{6}$/;

/** Split #RRGGBB into three 0-255 channels. */
function channels(hex) {
  if (!HEX.test(hex)) throw new Error(`not a six-digit hex colour: "${hex}"`);
  return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
}

/** Reassemble three 0-255 channels into an uppercase #RRGGBB. */
function toHex(rgb) {
  return (
    "#" +
    rgb
      .map((v) =>
        Math.round(Math.min(255, Math.max(0, v)))
          .toString(16)
          .padStart(2, "0")
          .toUpperCase(),
      )
      .join("")
  );
}

/** The sRGB inverse transfer function, per channel. */
function linearize(value) {
  const s = value / 255;
  return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}

/**
 * WCAG relative luminance, 0 (black) to 1 (white).
 *
 * This is NOT the same scale as an opacity, and conflating the two is how a
 * scrim table ends up quoting 99.4% where the real alpha is 93.3%.
 */
export function relativeLuminance(hex) {
  const [r, g, b] = channels(hex).map(linearize);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG contrast ratio, 1 to 21. Symmetric in its arguments. */
export function contrastRatio(a, b) {
  const [high, low] = [relativeLuminance(a), relativeLuminance(b)].sort((x, y) => y - x);
  return (high + 0.05) / (low + 0.05);
}

/** AA is 4.5:1 for body text, 3:1 for large text (>=18.66px bold or >=24px). */
export function meetsAA(foreground, background, large = false) {
  return contrastRatio(foreground, background) >= (large ? 3 : 4.5);
}

/**
 * The colour a `tint` at `coverage` actually ends up being over `backdrop`.
 *
 * This is what the material system binds to. The coverage is a rendering
 * input; the returned colour is the contract.
 */
export function composite(tint, coverage, backdrop) {
  const t = channels(tint);
  const b = channels(backdrop);
  return toHex(t.map((c, i) => c * coverage + b[i] * (1 - coverage)));
}

/** `composite` with the arguments named for blending one colour into another. */
export function mix(base, other, weight) {
  return composite(other, weight, base);
}

/** An `rgba(r, g, b, a)` string, which React Native accepts anywhere a colour goes. */
export function withAlpha(hex, alpha) {
  const [r, g, b] = channels(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

const TYPE_SHORTHAND = /^(\d{3})\s+(\d+)px\/(\d+)px$/;

/**
 * Split a `"700 32px/38px"` type token into the three numbers a React Native
 * TextStyle wants. Throws rather than returning NaN: a NaN fontSize renders
 * nothing at all, which is far harder to trace than a build-time throw.
 */
export function parseTypeShorthand(value) {
  const match = TYPE_SHORTHAND.exec(value);
  if (!match) throw new Error(`unrecognised type shorthand: "${value}"`);
  return {
    fontWeight: Number(match[1]),
    fontSize: Number(match[2]),
    lineHeight: Number(match[3]),
  };
}
