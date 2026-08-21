/// <reference types="bun-types" />
/**
 * Pure-function coverage for Task 5's add-action registry.
 *
 * `AddButton` decides whether to render at all from the registry's length,
 * and a React component cannot be rendered by this runner (no renderer, no
 * @testing-library — see `garage-format.test.ts`'s header for the same
 * landmine). Pulling that one decision out into a pure, react-native-free
 * predicate is what lets AM-16 AC2's actual rule — an empty registry means
 * no button, ever — be held by a test rather than only checked by eye.
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { expect, test } from "bun:test";

import { ADD_ACTIONS, hasAddActions, isAddActionReady } from "@/features/shell/addActions";
import type { AddAction } from "@/features/shell/addActions";

test("an empty registry has no available action", () => {
  expect(hasAddActions([])).toBe(false);
});

test("one entry makes an action available", () => {
  const oneEntry: readonly AddAction[] = [
    {
      key: "katalog",
      label: "Katalog komponen",
      href: "/catalog",
      icon: "cube-outline",
      description: "Contoh entri",
    },
  ];
  expect(hasAddActions(oneEntry)).toBe(true);
});

test("entries added later still make an action available", () => {
  const grown: readonly AddAction[] = [
    {
      key: "modifikasi",
      label: "Modifikasi",
      href: "/catalog",
      icon: "construct-outline",
      description: "Part, setup, dan build",
    },
    {
      key: "servis",
      label: "Servis",
      href: "/catalog",
      icon: "clipboard-outline",
      description: "Catatan servis dan biaya",
    },
  ];
  expect(hasAddActions(grown)).toBe(true);
});

test("the shipped registry names all four actions AM-16 AC2 lists", () => {
  expect(ADD_ACTIONS.map((a) => a.key)).toEqual(["modifikasi", "servis", "problem", "foto"]);
  expect(hasAddActions(ADD_ACTIONS)).toBe(true);
});

/**
 * The rule that replaced "the registry is empty".
 *
 * The registry used to be empty so that no control could point at a form that
 * does not exist. The four rows are visible now, which means the same
 * protection has to come from somewhere else: a row is tappable if and only if
 * it has a destination. A future entry that forgets its `href` fails here
 * rather than shipping a row that swallows a tap and does nothing.
 */
test("a row is ready exactly when it has somewhere to go", () => {
  for (const action of ADD_ACTIONS) {
    expect(isAddActionReady(action)).toBe(action.href !== null);
  }
});

test("every shipped row carries a label, an icon, and a description", () => {
  for (const action of ADD_ACTIONS) {
    expect(action.label.length).toBeGreaterThan(0);
    expect(action.icon.length).toBeGreaterThan(0);
    expect(action.description.length).toBeGreaterThan(0);
  }
});

/**
 * Two rules held against the COMPONENT, not only against the predicates.
 *
 * The first: `AddButton` still consults `hasAddActions`. Delete that line and
 * every other test here stays green, tsc passes, lint passes — and emptying
 * the registry later would grow a floating "Tambah" on all five tabs opening
 * a sheet with a vehicle picker and ZERO rows, the dead end AC2 forbids.
 *
 * The second: the sheet routes through `isAddActionReady`. Without it a row
 * with `href: null` would render as an ordinary tappable row that goes
 * nowhere — the same lie by a different route.
 *
 * Source-text assertions because this repository's runner has no React
 * renderer — the same technique `session.test.ts` uses for the auth gate and
 * for AC1's per-tab stacks. Found in the Tasks 3-5 review.
 */
test("AddButton renders nothing while the registry is empty", () => {
  const source = readFileSync(
    fileURLToPath(new URL("../src/components/shell/AddButton.tsx", import.meta.url)),
    "utf8",
  );
  expect(source).toMatch(/if\s*\(!hasAddActions\(ADD_ACTIONS\)\)\s*return null;/);
});

test("a row with no destination is never drawn as a Pressable", () => {
  const source = readFileSync(
    fileURLToPath(new URL("../src/components/shell/AddButton.tsx", import.meta.url)),
    "utf8",
  );
  expect(source).toMatch(/isAddActionReady\(action\)/);
  // The unavailable branch returns a plain View and returns EARLY, so the
  // Pressable below it is unreachable for a row with no href.
  expect(source).toMatch(/if\s*\(!ready\)\s*\{[\s\S]*?<View/);
});
