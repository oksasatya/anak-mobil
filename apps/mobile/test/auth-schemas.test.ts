/// <reference types="bun-types" />
/**
 * Pure-function coverage for Task 1's `schemas.ts` and `conflict.ts`.
 *
 * Both files are native-free (zod plus one type import), so nothing here
 * needs `mock.module` — the `@/shared` import is `import type`, erased before
 * either the schema or the conflict-resolution logic ever runs.
 */
import { expect, test } from "bun:test";
import { z } from "zod";

import { registerConflictOf } from "@/features/auth/conflict";
import {
  fieldErrorsOf,
  loginSchema,
  MIN_PASSWORD,
  registerSchema,
  USERNAME_PATTERN,
} from "@/features/auth/schemas";
import type { ApiError } from "@/shared";

// The boundary table from the plan (Task 1 Step 3), reproduced verbatim so a
// regression in the regex fails here rather than only being caught by eye on
// a simulator.
const USERNAME_CASES: ReadonlyArray<readonly [string, boolean]> = [
  ["ok", false], // 2 characters
  ["oks", true], // exactly 3
  ["a".repeat(30), true], // exactly 30
  ["a".repeat(31), false], // 31
  [".oksa", false], // leading dot
  ["_oksa", false], // leading underscore
  ["oksa.", false], // trailing dot
  ["oksa_", false], // trailing underscore
  ["ok..sa", false], // consecutive dots
  ["ok__sa", true], // consecutive underscores are not banned
  ["ok.sa", true], // single interior dot
  ["Oksa", false], // uppercase
  ["oks a", false], // space
  ["oksa-1", false], // hyphen
];

for (const [input, expected] of USERNAME_CASES) {
  test(`USERNAME_PATTERN ${expected ? "accepts" : "rejects"} ${JSON.stringify(input)}`, () => {
    expect(USERNAME_PATTERN.test(input)).toBe(expected);
  });
}

test("registerSchema rejects a password one character short of MIN_PASSWORD", () => {
  const result = registerSchema.safeParse({
    email: "budi@example.com",
    username: "budi123",
    password: "a".repeat(MIN_PASSWORD - 1),
    consent: true,
  });
  expect(result.success).toBe(false);
});

test("registerSchema accepts a password exactly MIN_PASSWORD characters", () => {
  const result = registerSchema.safeParse({
    email: "budi@example.com",
    username: "budi123",
    password: "a".repeat(MIN_PASSWORD),
    consent: true,
  });
  expect(result.success).toBe(true);
});

test("loginSchema requires a non-empty password with no length floor of its own", () => {
  expect(loginSchema.safeParse({ email: "budi@example.com", password: "" }).success).toBe(false);
  expect(loginSchema.safeParse({ email: "budi@example.com", password: "x" }).success).toBe(true);
});

test("fieldErrorsOf flattens a multi-field registerSchema failure", () => {
  const result = registerSchema.safeParse({
    email: "not-an-email",
    username: "no",
    password: "short",
    consent: false,
  });
  if (result.success) throw new Error("expected failure");

  const errors = fieldErrorsOf(result.error);
  expect(Object.keys(errors).sort()).toEqual(["consent", "email", "password", "username"]);

  // The dedup assertion lives HERE because this is the only fixture that
  // actually produces two issues on one path: "no" fails `.min(3)` AND the
  // regex. It replaces a test that could not fail — that one used
  // `z.string().min(3).max(5)` against "a", which raises exactly ONE issue
  // (`.max(5)` passes), so the `!(key in out)` guard was never reached and
  // deleting the guard left it green. Found in Task 1's review.
  expect(errors.username).toBe("Minimal 3 karakter.");
});

test("the username mirror canonicalises before validating, exactly as the server does", () => {
  // `username.rs` does `raw.trim().to_ascii_lowercase()` and only THEN
  // validates, so a client that validated the raw string rejected names the
  // server accepts. Pasting from a password manager is the ordinary route in.
  const base = { email: "budi@example.com", password: "a".repeat(MIN_PASSWORD), consent: true };

  const upper = registerSchema.safeParse({ ...base, username: "Oksa" });
  expect(upper.success).toBe(true);
  if (upper.success) expect(upper.data.username).toBe("oksa");

  const padded = registerSchema.safeParse({ ...base, username: "  Budi  " });
  expect(padded.success).toBe(true);
  if (padded.success) expect(padded.data.username).toBe("budi");

  // Canonicalising must not soften the rules it then applies.
  expect(registerSchema.safeParse({ ...base, username: "  ok  " }).success).toBe(false);
  expect(registerSchema.safeParse({ ...base, username: "Ok..Sa" }).success).toBe(false);
  expect(registerSchema.safeParse({ ...base, username: ".budi" }).success).toBe(false);
});

test("registerConflictOf reads a field-keyed validation error for username", () => {
  const error: ApiError = {
    kind: "validation",
    message: "Periksa kembali data kamu.",
    fields: { username: "Username ini sudah dipakai." },
  };
  expect(registerConflictOf(error)).toBe("username");
});

test("registerConflictOf reads a field-keyed validation error for email", () => {
  const error: ApiError = {
    kind: "validation",
    message: "Periksa kembali data kamu.",
    fields: { email: "Email ini sudah terdaftar." },
  };
  expect(registerConflictOf(error)).toBe("email");
});

test("registerConflictOf returns null for an error that named no field", () => {
  const error: ApiError = { kind: "server", message: "Ada gangguan di server." };
  expect(registerConflictOf(error)).toBe(null);
});
