/// <reference types="bun-types" />
/**
 * Pure-function coverage for Task 2's `useCountdown.ts`.
 *
 * `secondsRemaining`, `normalizeSeconds`, and `formatCountdown` are native-free
 * arithmetic — no `mock.module` needed. `useCountdown` itself (the hook) is
 * NOT tested here: it is a `useState`/`useEffect` wrapper around
 * `secondsRemaining`, and this repo has no React renderer under `bun:test`
 * (see test/session.test.ts's own note on why). The arithmetic these tests
 * pin is exactly the part that is wrong at a boundary and invisible on a
 * simulator, which is the point of testing it at all.
 */
import { expect, test } from "bun:test";

import {
  DEFAULT_RETRY_SECONDS,
  formatCountdown,
  normalizeSeconds,
  secondsRemaining,
} from "@/features/auth/useCountdown";

test("secondsRemaining counts down toward a future deadline", () => {
  const now = 1_000_000;
  // Deleting the subtraction (returning `totalSeconds` unconditionally) still
  // passes this one test alone, but not alongside the "expiry in the past"
  // and "clock jumps backwards" cases below.
  expect(secondsRemaining(now + 30_000, now, 30)).toBe(30);
});

test("secondsRemaining is zero exactly at the deadline", () => {
  const now = 1_000_000;
  // Catches an off-by-one such as `Math.ceil(x) + 1` or a `>` vs `>=` slip in
  // the clamp that would report 1 second still remaining at zero.
  expect(secondsRemaining(now, now, 30)).toBe(0);
});

test("secondsRemaining clamps to zero once the deadline has passed", () => {
  const now = 1_000_000;
  // Catches a missing `Math.max(0, …)` guard, which would otherwise report a
  // negative number here — and a negative countdown displayed on screen, or
  // worse, a negative number treated as truthy re-enabling the button early
  // in an `if (remaining)` check elsewhere.
  expect(secondsRemaining(now - 5_000, now, 30)).toBe(0);
});

test("secondsRemaining rounds a fractional second UP, never down", () => {
  const now = 1_000_000;
  // 1.5s left must read "2", not "1" — pinning ceil over floor. A `floor` (or
  // `round`) implementation would return 1 here, and the button would then
  // sit disabled through a tick showing "0 detik" a moment before it is
  // actually allowed to re-enable — the exact bug the plan calls out.
  expect(secondsRemaining(now + 1_500, now, 10)).toBe(2);
});

test("secondsRemaining never exceeds the originally requested total, even if the device clock jumps backwards", () => {
  const now = 1_000_000;
  const deadline = now + 10_000; // a 10s countdown was started
  const clockJumpedBack = now - 5_000; // the device's clock later reads 5s earlier than "now" above
  // Without the `Math.min(raw, totalSeconds)` clamp, the raw subtraction
  // widens to `ceil((10_000 - -5_000) / 1000) = 15`, showing a 15-second wait
  // for a limiter that only asked for 10 — worse for the person than the
  // server actually required. Clamping to the requested total bounds the
  // damage to "at most what the 429 said", never more.
  expect(secondsRemaining(deadline, clockJumpedBack, 10)).toBe(10);
});

test("normalizeSeconds falls back to the default when retryAfterSeconds is missing", () => {
  // Catches a naive `seconds ?? 0` (or no fallback at all), which would start
  // a countdown that is already expired — the button re-enables instantly on
  // a 429 the server explicitly asked us to wait out.
  expect(normalizeSeconds(undefined)).toBe(DEFAULT_RETRY_SECONDS);
});

test("normalizeSeconds falls back to the default on a non-finite or non-positive value", () => {
  // Catches a bare `Number(seconds)` with no finiteness/positivity check,
  // which would otherwise render "NaN detik" or start a countdown of zero or
  // negative seconds that never blocks the button at all.
  expect(normalizeSeconds(Number.NaN)).toBe(DEFAULT_RETRY_SECONDS);
  expect(normalizeSeconds(0)).toBe(DEFAULT_RETRY_SECONDS);
  expect(normalizeSeconds(-5)).toBe(DEFAULT_RETRY_SECONDS);
});

test("normalizeSeconds passes a real positive value through unchanged", () => {
  // Catches an over-eager fallback that clamps every value to the default
  // regardless of what the server actually sent.
  expect(normalizeSeconds(45)).toBe(45);
});

test("formatCountdown renders minutes:seconds, zero-padded", () => {
  // Catches a missing `padStart`, which would render "1:5" instead of "1:05".
  expect(formatCountdown(95)).toBe("1:35");
  expect(formatCountdown(5)).toBe("0:05");
  expect(formatCountdown(60)).toBe("1:00");
  expect(formatCountdown(0)).toBe("0:00");
});
