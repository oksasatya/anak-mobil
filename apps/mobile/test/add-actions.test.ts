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

import { ADD_ACTIONS, hasAddActions } from "@/features/shell/addActions";
import type { AddAction } from "@/features/shell/addActions";

test("an empty registry has no available action", () => {
  expect(hasAddActions([])).toBe(false);
});

test("one entry makes an action available", () => {
  const oneEntry: readonly AddAction[] = [
    { key: "katalog", label: "Katalog komponen", href: "/catalog" },
  ];
  expect(hasAddActions(oneEntry)).toBe(true);
});

test("entries added later still make an action available", () => {
  const grown: readonly AddAction[] = [
    { key: "modifikasi", label: "Modifikasi", href: "/catalog" },
    { key: "servis", label: "Servis", href: "/catalog" },
  ];
  expect(hasAddActions(grown)).toBe(true);
});

test("the shipped registry is empty — AC2 ships as a mechanism, not a button", () => {
  expect(ADD_ACTIONS).toEqual([]);
  expect(hasAddActions(ADD_ACTIONS)).toBe(false);
});

/**
 * The rule this task exists for, held against the COMPONENT and not only
 * against the predicate.
 *
 * `hasAddActions([]) === false` above pins the predicate. It does not pin the
 * thing that matters: that `AddButton` consults it. Change `AddButton.tsx` to
 * `return <AddButtonContent />` and every other test here stays green, tsc
 * passes, lint passes — and the shipped app grows a floating "Tambah" on all
 * five tabs that opens a sheet with a vehicle picker and ZERO actions. That is
 * exactly the dead end AC2's decision paragraph and `AmEmptyState`'s design
 * note forbid.
 *
 * Source-text assertion because this repository's runner has no React
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
