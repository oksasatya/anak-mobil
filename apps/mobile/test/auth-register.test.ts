/// <reference types="bun-types" />
/**
 * Pure-function coverage for Task 3's password strength meter and username
 * availability state machine.
 *
 * Imports go straight to `./passwordScore` and `./availability` — the
 * react-native-free siblings of `PasswordStrength.tsx` and
 * `useUsernameAvailability.ts` — rather than the component/hook modules
 * themselves. Importing either of THOSE would drag in `react-native` (a
 * Flow-typed parse error this test runner cannot handle) or, via `./api` ->
 * `@/shared`, `react-native-mmkv` and `expo-router`'s `Redirect`, both of
 * which crash outside a device. See `test/session.test.ts`'s header comment
 * for the same landmine documented against Task 1's files.
 *
 * `passwordScore.ts`, not `passwordStrength.ts`: on this repo's default
 * case-insensitive filesystem, a name differing from `PasswordStrength.tsx`
 * only by case resolved to the WRONG file and imported react-native anyway —
 * discovered by this test crashing until the file was renamed.
 */
import { expect, test } from "bun:test";

import {
  deriveAvailability,
  disablesSubmit,
  outcomeOfResult,
  OUTCOME_ON_FAILURE,
} from "@/features/auth/availability";
import { strengthOf } from "@/features/auth/passwordScore";

// --- strengthOf: band boundaries -------------------------------------------
// Bands: 0 -> empty, 1 -> <8 (MIN_PASSWORD), 2 -> <12, 3 -> <16, 4 -> >=16.

test("strengthOf scores an empty password 0", () => {
  expect(strengthOf("")).toBe(0);
});

test("strengthOf scores one character below MIN_PASSWORD (7) as 1", () => {
  expect(strengthOf("a".repeat(7))).toBe(1);
});

test("strengthOf scores exactly MIN_PASSWORD (8) as 2, not 1", () => {
  expect(strengthOf("a".repeat(8))).toBe(2);
});

test("strengthOf scores 11 characters as 2", () => {
  expect(strengthOf("a".repeat(11))).toBe(2);
});

test("strengthOf scores exactly 12 characters as 3, not 2", () => {
  expect(strengthOf("a".repeat(12))).toBe(3);
});

test("strengthOf scores 15 characters as 3", () => {
  expect(strengthOf("a".repeat(15))).toBe(3);
});

test("strengthOf scores exactly 16 characters as 4, not 3", () => {
  expect(strengthOf("a".repeat(16))).toBe(4);
});

test("strengthOf scores well past 16 characters as 4", () => {
  expect(strengthOf("a".repeat(40))).toBe(4);
});

// --- strengthOf: characters, not UTF-16 code units --------------------------
// Mirrors the backend's own `length_is_counted_in_characters_not_bytes`.
//
// CORRECTED after Task 3's review. Two accented tests used to sit here and
// NEITHER COULD FAIL: the source held "é" as U+00E9 precomposed — one code
// point — so `[...s].length` and `s.length` both give 8, and deleting the
// spread from `strengthOf` left them green. They duplicated the plain
// `"a".repeat(8)` case. A decomposed "e\u0301" would not have helped either:
// 2 code points AND 2 UTF-16 units, so still no disagreement. The emoji is the
// only case where the two operators actually diverge, so it is the only one
// that pins the property.
//
// The comments also described a byte count, which JavaScript has no operator
// for — `.length` counts UTF-16 code units. That framing came from the brief
// and had propagated into `passwordScore.ts` as well; both are corrected.

test("strengthOf counts an emoji password by character, not UTF-16 code unit", () => {
  // "🚗" is one character but a surrogate PAIR in JS, so `.length` on 8 of
  // them is 16 -> band 4, while character count is 8 -> band 2. This is the
  // one input where a regression to `.length` changes the answer.
  expect(strengthOf("🚗".repeat(8))).toBe(2);
});

// --- deriveAvailability: every state the hook can render --------------------

test("deriveAvailability is idle when the check is disabled, regardless of any prior resolution", () => {
  expect(deriveAvailability(false, "oksa", null)).toBe("idle");
  expect(deriveAvailability(false, "oksa", { username: "oksa", outcome: "taken" })).toBe("idle");
});

test("deriveAvailability is checking when enabled with no resolution yet", () => {
  expect(deriveAvailability(true, "oksa", null)).toBe("checking");
});

test("deriveAvailability resolves a free name to available", () => {
  expect(deriveAvailability(true, "oksa", { username: "oksa", outcome: "available" })).toBe(
    "available",
  );
});

test("deriveAvailability resolves a taken name to taken", () => {
  expect(deriveAvailability(true, "oksa", { username: "oksa", outcome: "taken" })).toBe("taken");
});

test("an unknown outcome renders as unknown, never as taken", () => {
  // CORRECTED after Task 3's review. Three byte-identical tests used to sit
  // here, named for a 429, a 5xx, and offline — but all three called
  // `deriveAvailability` with an outcome ALREADY equal to "unknown", so they
  // only exercised a pass-through that two earlier tests already cover. The
  // mapping they were named for lives in the hook, which no test can import,
  // so `OUTCOME_ON_FAILURE` could have been changed to "taken" and all 22
  // tests would still have passed. The two tests below pin it properly.
  expect(deriveAvailability(true, "oksa", { username: "oksa", outcome: "unknown" })).toBe(
    "unknown",
  );
});

test("EVERY failure maps to unknown — a 429, a 5xx, and offline alike", () => {
  // The security-adjacent one. Indonesian carrier CGNAT puts many real users
  // behind one address, so a shared-address 429 on this HINT endpoint is
  // ordinary. Telling somebody a free name is taken because the hint failed
  // is worse than the hint saying nothing, and blocking submit on it stops
  // them registering at all. Change this constant to "taken" and this goes red.
  expect(OUTCOME_ON_FAILURE).toBe("unknown");
  expect(disablesSubmit(OUTCOME_ON_FAILURE)).toBe(false);
});

test("only a real {available: false} answer produces taken", () => {
  // The other half: the ONLY route to "taken". Invert this mapping and a free
  // name reads as taken while a taken one reads as free.
  expect(outcomeOfResult({ available: false })).toBe("taken");
  expect(outcomeOfResult({ available: true })).toBe("available");
});

test("deriveAvailability falls back to checking when the resolution belongs to a superseded username", () => {
  // This is what an aborted (cancelled-by-a-newer-keystroke) request looks
  // like once derived: the stale resolution is simply for a name that is no
  // longer on screen, so it must never be read as an answer for the current
  // one — a wrong-name "taken" would block the button on a name nobody typed.
  expect(deriveAvailability(true, "oksasatya", { username: "oksa", outcome: "taken" })).toBe(
    "checking",
  );
});

// --- disablesSubmit: only `taken` blocks the button -------------------------

test("disablesSubmit blocks only on taken", () => {
  expect(disablesSubmit("taken")).toBe(true);
});

test("disablesSubmit never blocks on unknown (a broken hint must not block registration)", () => {
  expect(disablesSubmit("unknown")).toBe(false);
});

test("disablesSubmit does not block on idle, checking, or available", () => {
  expect(disablesSubmit("idle")).toBe(false);
  expect(disablesSubmit("checking")).toBe(false);
  expect(disablesSubmit("available")).toBe(false);
});
