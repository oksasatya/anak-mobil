import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { accent, brand, dark, layout, light, radius, semantic, spacing } from "../src/tokens.js";

const here = dirname(fileURLToPath(import.meta.url));
const read = (file) => readFile(resolve(here, "..", "dist", file), "utf8");

const HEX = /^#[0-9A-F]{6}$/;

test("every colour is a six-digit uppercase hex", () => {
  const groups = { brand, accent, semantic, light, dark };
  for (const [group, values] of Object.entries(groups)) {
    for (const [key, value] of Object.entries(values)) {
      assert.match(value, HEX, `${group}.${key} is "${value}"`);
    }
  }
});

test("the two themes define exactly the same keys", () => {
  // A key present in one theme but not the other renders as an unset
  // variable — text the same colour as its background, in one theme only.
  assert.deepEqual(Object.keys(light).sort(), Object.keys(dark).sort());
});

test("brand and accent match docs/design.md", () => {
  assert.equal(brand[800], "#1D232A", "Graphite");
  assert.equal(accent[500], "#ED491C", "AnakMobil Orange");
});

test("accent is not reused as a status colour", () => {
  // Orange means "primary action" or "AI". A status wearing it makes both
  // meanings unreadable, so the two palettes must not overlap.
  const accents = new Set(Object.values(accent));
  for (const [name, value] of Object.entries(semantic)) {
    assert.ok(!accents.has(value), `semantic.${name} reuses an accent value`);
  }
});

test("spacing is a multiple of the four-pixel base", () => {
  for (const [step, value] of Object.entries(spacing)) {
    assert.equal(value % 4, 0, `spacing.${step} is ${value}`);
  }
});

test("radius steps increase", () => {
  const ordered = [radius.xs, radius.sm, radius.md, radius.lg, radius.xl, radius["2xl"]];
  for (let i = 1; i < ordered.length; i += 1) {
    assert.ok(ordered[i] > ordered[i - 1], `radius step ${i} does not increase`);
  }
});

test("the touch target meets the accessibility floor", () => {
  // AM-273 AC1.
  assert.ok(layout.touchTargetMin >= 44);
});

test("generated CSS carries every theme key in both directions", async () => {
  const css = await read("tokens.css");
  assert.match(css, /GENERATED/);
  assert.match(css, /--color-surface-subtle:/, "camelCase keys become kebab-case");
  assert.match(css, /@media \(prefers-color-scheme: dark\)/);
  assert.match(css, /:root:not\(\[data-theme="light"\]\)/, "an explicit light choice must win");
  assert.match(css, /:root\[data-theme="dark"\]/, "an explicit dark choice must win");
});

test("generated CSS has no malformed variable names", async () => {
  for (const file of ["tokens.css", "theme.css"]) {
    const css = await read(file);
    assert.doesNotMatch(css, /---/, `${file} contains a triple dash`);
    assert.doesNotMatch(css, /--\s*:/, `${file} contains an empty variable name`);
  }
});

test("the Tailwind theme imports Tailwind and opens a theme block", async () => {
  const css = await read("theme.css");
  assert.match(css, /@import "tailwindcss";/);
  assert.match(css, /@theme \{/);
  assert.match(css, /--color-accent-500: #ED491C;/);
});
