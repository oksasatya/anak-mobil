import assert from "node:assert/strict";
import test from "node:test";

import { composite, contrastRatio } from "../src/derive.js";
import { dark, ground, light, material, onAccent, semanticText } from "../src/tokens.js";

const THEMES = { light, dark };

/** The two backdrops a translucent surface can be worst-cased against. */
const EXTREMES = { white: "#FFFFFF", black: "#000000" };

test("every material role composites to the colour it claims", () => {
  for (const [themeName, roles] of Object.entries(material)) {
    // The MIDDLE stop, not the first: it is the theme's own background and
    // the colour most of the screen actually is. The ends are the gradient's
    // lighter and darker extremes, which no role composites over.
    const groundBase = ground[themeName].stops[1].color;
    assert.equal(
      groundBase,
      THEMES[themeName].background,
      `${themeName} ground's middle stop must equal the theme's own background`,
    );
    for (const [roleName, role] of Object.entries(roles)) {
      assert.equal(
        role.solid,
        composite(role.tint, role.coverage, groundBase),
        `${themeName}.${roleName}.solid disagrees with its own tint and coverage`,
      );
    }
  }
});

test("working roles are solid — zero transparency, no exceptions", () => {
  // The rule that keeps a service record, a fitment result, and an AI safety
  // warning legible in direct sun at a workshop.
  assert.equal(material.light.working.coverage, 1);
  assert.equal(material.dark.working.coverage, 1);
});

test("secondary and tertiary text never sit on chrome", () => {
  // chrome is the one role whose backdrop is genuinely unknown.
  assert.deepEqual(material.light.chrome.allowsText, ["primary"]);
  assert.deepEqual(material.dark.chrome.allowsText, ["primary"]);
});

test("tertiary text never sits on a translucent role", () => {
  for (const roles of Object.values(material)) {
    for (const [name, role] of Object.entries(roles)) {
      if (role.coverage < 1) {
        assert.ok(
          !role.allowsText.includes("tertiary"),
          `${name} is translucent but claims to carry tertiary text`,
        );
      }
    }
  }
});

test("every material x allowed-text pair clears AA against both extremes", () => {
  const failures = [];
  for (const [themeName, roles] of Object.entries(material)) {
    const theme = THEMES[themeName];
    const text = {
      primary: theme.textPrimary,
      secondary: theme.textSecondary,
      tertiary: theme.textTertiary,
    };
    for (const [roleName, role] of Object.entries(roles)) {
      const backdrops =
        role.coverage === 1
          ? { solid: role.solid }
          : {
              ...Object.fromEntries(
                Object.entries(EXTREMES).map(([k, bd]) => [
                  k,
                  composite(role.tint, role.coverage, bd),
                ]),
              ),
              ground: role.solid,
            };
      for (const textRole of role.allowsText) {
        for (const [backdropName, backdrop] of Object.entries(backdrops)) {
          const ratio = contrastRatio(text[textRole], backdrop);
          if (ratio < 4.5) {
            failures.push(
              `${themeName}.${roleName} ${textRole} over ${backdropName} (${backdrop}) = ${ratio.toFixed(2)}`,
            );
          }
        }
      }
    }
  }
  assert.deepEqual(failures, [], `pairs below 4.5:1:\n${failures.join("\n")}`);
});

test("semantic text colours clear AA on their theme's working surface", () => {
  for (const [themeName, roles] of Object.entries(material)) {
    for (const [name, color] of Object.entries(semanticText[themeName])) {
      const ratio = contrastRatio(color, roles.working.solid);
      assert.ok(ratio >= 4.5, `${themeName} semanticText.${name} = ${ratio.toFixed(2)}`);
    }
  }
});

test("semantic text colours also clear AA on the subtle surface", () => {
  // Badges and toasts sit on surfaceSubtle as often as on the base surface.
  for (const [themeName, theme] of Object.entries(THEMES)) {
    for (const [name, color] of Object.entries(semanticText[themeName])) {
      const ratio = contrastRatio(color, theme.surfaceSubtle);
      assert.ok(
        ratio >= 4.5,
        `${themeName} semanticText.${name} on surfaceSubtle = ${ratio.toFixed(2)}`,
      );
    }
  }
});

test("the accent label clears AA on the accent fill", () => {
  // White on #ED491C is 3.77 and fails; graphite-950 is why onAccent exists.
  assert.ok(contrastRatio(onAccent, "#ED491C") >= 4.5);
});

test("tertiary text clears AA on every solid surface of its own theme", () => {
  for (const [themeName, theme] of Object.entries(THEMES)) {
    const surfaces = [theme.background, theme.surface, theme.surfaceSubtle, theme.surfaceRaised];
    for (const surface of surfaces) {
      const ratio = contrastRatio(theme.textTertiary, surface);
      assert.ok(ratio >= 4.5, `${themeName} tertiary on ${surface} = ${ratio.toFixed(2)}`);
    }
  }
});
