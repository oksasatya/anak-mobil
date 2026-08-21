/// <reference types="bun-types" />
/**
 * Pure-function coverage for Task 4's profile name/handle fallback.
 *
 * `@/features/profile/identity` is deliberately react-native-free so it can
 * be required by this runner, which has no renderer. See
 * `test/garage-format.test.ts`'s header comment for the same landmine.
 */
import { expect, test } from "bun:test";

import { resolveProfileIdentity } from "@/features/profile/identity";

test("resolveProfileIdentity uses the display name as the headline and shows the username as a handle", () => {
  expect(
    resolveProfileIdentity({
      displayName: "Budi Santoso",
      username: "budi92",
      email: "budi@example.test",
    }),
  ).toEqual({ name: "Budi Santoso", handle: "@budi92" });
});

test("resolveProfileIdentity falls back to the username as the headline, with no duplicate handle line", () => {
  expect(
    resolveProfileIdentity({ displayName: null, username: "budi92", email: "budi@example.test" }),
  ).toEqual({ name: "budi92", handle: null });
});

test("resolveProfileIdentity falls back to the email when neither name nor username is set", () => {
  expect(
    resolveProfileIdentity({ displayName: null, username: null, email: "budi@example.test" }),
  ).toEqual({ name: "budi@example.test", handle: null });
});

test("resolveProfileIdentity treats an empty-string username as absent rather than a stray @", () => {
  expect(
    resolveProfileIdentity({ displayName: null, username: "", email: "budi@example.test" }),
  ).toEqual({ name: "budi@example.test", handle: null });
});

test("resolveProfileIdentity treats a whitespace-only display name as absent rather than an empty headline", () => {
  expect(
    resolveProfileIdentity({ displayName: "   ", username: "budi92", email: "budi@example.test" }),
  ).toEqual({ name: "budi92", handle: null });
});
