/// <reference types="bun-types" />
/**
 * Pure-function coverage for Task 1's Indonesian formatters and error copy.
 *
 * Imports go straight to `@/features/garage/format` and
 * `@/features/shell/errorCopy` — both are deliberately react-native-free
 * (zero runtime imports; `errorCopy.ts`'s only import is `import type`,
 * erased at compile time) so they can be required by this test runner,
 * which has no renderer and cannot parse react-native's Flow-typed
 * internals. See `test/auth-register.test.ts`'s header comment for the
 * same landmine documented against Plan B's files.
 */
import { expect, test } from "bun:test";

import { formatKilometres, formatRupiah, formatShortDate } from "@/features/garage/format";
import { errorBody } from "@/features/shell/errorCopy";

// --- formatRupiah -------------------------------------------------------

test("formatRupiah formats a whole-rupiah decimal string with thousands dots", () => {
  expect(formatRupiah("4200000.00")).toBe("Rp 4.200.000");
});

test("formatRupiah drops fractional rupiah — sen are noise on a card", () => {
  // Pinned: `summary.rs` sends a scale-2 decimal, but rupiah has no
  // subunit in practice, so ".50" must not reach the screen.
  expect(formatRupiah("4200000.50")).toBe("Rp 4.200.000");
});

test("formatRupiah formats zero", () => {
  expect(formatRupiah("0")).toBe("Rp 0");
});

test("formatRupiah groups a value that needs more than one thousands separator", () => {
  expect(formatRupiah("185000000.50")).toBe("Rp 185.000.000");
});

test("formatRupiah formats a very large value", () => {
  expect(formatRupiah("999999999999.99")).toBe("Rp 999.999.999.999");
});

// --- formatKilometres -----------------------------------------------------

test("formatKilometres formats zero", () => {
  expect(formatKilometres(0)).toBe("0 km");
});

test("formatKilometres formats a three-digit value with no separator", () => {
  expect(formatKilometres(500)).toBe("500 km");
});

test("formatKilometres formats a seven-digit value with two separators", () => {
  expect(formatKilometres(1234567)).toBe("1.234.567 km");
});

// --- formatShortDate --------------------------------------------------

test("formatShortDate renders an ISO date at a month boundary", () => {
  expect(formatShortDate("2026-01-31")).toBe("31 Jan 2026");
});

test("formatShortDate renders an ISO date at a year boundary", () => {
  expect(formatShortDate("2025-12-31")).toBe("31 Des 2025");
  expect(formatShortDate("2026-01-01")).toBe("1 Jan 2026");
});

test("formatShortDate returns an unparseable value as-is rather than guessing", () => {
  expect(formatShortDate("bukan-tanggal")).toBe("bukan-tanggal");
});

// --- errorBody: every ApiErrorKind, plus a non-ApiError throw -------------

test("errorBody has its own message for offline", () => {
  expect(errorBody({ kind: "offline", message: "unused" })).toBe(
    "Tidak ada koneksi. Data kamu aman — coba lagi setelah online.",
  );
});

test("errorBody appends the reassurance to the server's own message for validation", () => {
  expect(errorBody({ kind: "validation", message: "Kata sandi terlalu pendek." })).toBe(
    "Kata sandi terlalu pendek. Data kamu aman.",
  );
});

test("errorBody appends the reassurance to the server's own message for rateLimited", () => {
  expect(errorBody({ kind: "rateLimited", message: "Terlalu banyak percobaan." })).toBe(
    "Terlalu banyak percobaan. Data kamu aman.",
  );
});

test("errorBody appends the reassurance to the server's own message for unauthorized", () => {
  expect(errorBody({ kind: "unauthorized", message: "Sesi kamu sudah berakhir." })).toBe(
    "Sesi kamu sudah berakhir. Data kamu aman.",
  );
});

test("errorBody appends the reassurance to the server's own message for server", () => {
  expect(errorBody({ kind: "server", message: "Ada gangguan di server." })).toBe(
    "Ada gangguan di server. Data kamu aman.",
  );
});

test("errorBody falls back to a generic message for a value that is not an ApiError", () => {
  expect(errorBody(new Error("boom"))).toBe(
    "Ada gangguan. Data kamu aman — coba beberapa saat lagi.",
  );
  expect(errorBody("a plain string")).toBe(
    "Ada gangguan. Data kamu aman — coba beberapa saat lagi.",
  );
  expect(errorBody(null)).toBe("Ada gangguan. Data kamu aman — coba beberapa saat lagi.");
  expect(errorBody(undefined)).toBe("Ada gangguan. Data kamu aman — coba beberapa saat lagi.");
});

// --- Paths that no assertion could kill, found by mutation in Task 1's review.
// Each of these was added because deleting the code it covers left the suite
// green. Verified by re-running that mutation after adding them.

test("formatRupiah keeps the minus sign OUTSIDE the Rp prefix", () => {
  // Delete the negative branch and this reads "Rp -4.200.000". Nothing else
  // in the suite passes a negative, so that mutation used to survive.
  expect(formatRupiah("-4200000.00")).toBe("-Rp 4.200.000");
});

test("formatShortDate refuses a date-TIME string instead of fabricating one", () => {
  // The old guard tested truthiness, so `day` was "12T00:00:00Z" — non-empty,
  // therefore accepted — and `Number(day)` was NaN. It rendered the literal
  // "NaN Agu 2026" to a person.
  expect(formatShortDate("2026-08-12T00:00:00Z")).toBe("2026-08-12T00:00:00Z");
});

test("formatShortDate accepts any 1-2 digit day — a documented limit, not an oversight", () => {
  // "99" is not a real day, and this renders it. The guard's job is to reject
  // input it cannot PARSE (a date-time string, a missing field), not to
  // validate a calendar — the server is the authority on what dates exist and
  // it serialises strictly YYYY-MM-DD. Pinned so the limit is a decision
  // somebody made rather than a surprise somebody finds.
  expect(formatShortDate("2026-08-99")).toBe("99 Agu 2026");
});

test("formatShortDate refuses a missing year or a missing day", () => {
  // These two kill the `!year` and `!day` halves of the guard, which the
  // original suite left individually dead — only `!name` was ever exercised.
  expect(formatShortDate("-08-12")).toBe("-08-12");
  expect(formatShortDate("2026-08-")).toBe("2026-08-");
});
