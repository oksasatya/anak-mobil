import assert from "node:assert/strict";
import test from "node:test";

import {
  composite,
  contrastRatio,
  meetsAA,
  mix,
  parseTypeShorthand,
  relativeLuminance,
  withAlpha,
} from "../src/derive.js";

test("relative luminance matches the WCAG reference values", () => {
  // The two anchors the whole formula is pinned to.
  assert.equal(relativeLuminance("#FFFFFF").toFixed(4), "1.0000");
  assert.equal(relativeLuminance("#000000").toFixed(4), "0.0000");
  // A mid grey sits below 0.5 because the transfer function is not linear —
  // getting this wrong is the classic way a contrast checker silently lies.
  assert.equal(relativeLuminance("#808080").toFixed(4), "0.2159");
});

test("contrast ratio is symmetric and hits the known extremes", () => {
  assert.equal(contrastRatio("#000000", "#FFFFFF").toFixed(2), "21.00");
  assert.equal(contrastRatio("#FFFFFF", "#000000").toFixed(2), "21.00");
  assert.equal(contrastRatio("#777777", "#777777").toFixed(2), "1.00");
});

test("meetsAA uses 4.5 for body text and 3.0 for large text", () => {
  // #767676 on white is the canonical 4.54 boundary case.
  assert.equal(meetsAA("#767676", "#FFFFFF"), true);
  assert.equal(meetsAA("#777777", "#FFFFFF"), false);
  assert.equal(meetsAA("#777777", "#FFFFFF", true), true);
});

test("composite blends a tint over a backdrop at a coverage", () => {
  assert.equal(composite("#000000", 0.5, "#FFFFFF"), "#808080");
  assert.equal(composite("#151A20", 1, "#FFFFFF"), "#151A20");
  assert.equal(composite("#151A20", 0, "#FFFFFF"), "#FFFFFF");
  // The number the dark surface role is built on.
  assert.equal(composite("#151A20", 0.92, "#FFFFFF"), "#282C32");
  assert.equal(composite("#151A20", 0.92, "#000000"), "#13181D");
});

test("mix is composite with the arguments named for a tint", () => {
  assert.equal(mix("#FFFFFF", "#000000", 0.5), "#808080");
  // The weight is how much of `other` reaches `base`, which is what makes a
  // tintStrength of 0 the neutral ground rather than an untinted accident.
  assert.equal(mix("#0E1217", "#ED491C", 0), "#0E1217");
  assert.equal(mix("#0E1217", "#ED491C", 1), "#ED491C");
});

test("withAlpha renders an rgba string a React Native style accepts", () => {
  assert.equal(withAlpha("#151A20", 0.92), "rgba(21, 26, 32, 0.92)");
  assert.equal(withAlpha("#FFFFFF", 1), "rgba(255, 255, 255, 1)");
});

test("parseTypeShorthand splits the CSS font shorthand the tokens use", () => {
  assert.deepEqual(parseTypeShorthand("700 32px/38px"), {
    fontWeight: 700,
    fontSize: 32,
    lineHeight: 38,
  });
  assert.deepEqual(parseTypeShorthand("400 14px/21px"), {
    fontWeight: 400,
    fontSize: 14,
    lineHeight: 21,
  });
});

test("parseTypeShorthand rejects a malformed value instead of guessing", () => {
  // A silent NaN here would render invisible text, so it must throw.
  assert.throws(() => parseTypeShorthand("bold 16px"), /type shorthand/);
});
