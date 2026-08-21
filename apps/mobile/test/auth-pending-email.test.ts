/**
 * The register -> login email handoff.
 *
 * Pure and react-native-free, so it is testable; the screens that use it are
 * not (no React renderer in this suite).
 */
import { expect, test } from "bun:test";

import { setPendingEmail, takePendingEmail } from "@/features/auth/pendingEmail";

test("with nothing pending, the login field seeds empty", () => {
  // Not undefined and not null — it is fed straight to a `readonly value:
  // string` prop, and React Native turns an undefined value into an
  // uncontrolled input that silently stops tracking state.
  expect(takePendingEmail()).toBe("");
});

test("a pending address is handed over exactly once", () => {
  setPendingEmail("budi@example.com");
  expect(takePendingEmail()).toBe("budi@example.com");

  // Read-once. Reaching login again by the welcome screen or a back gesture
  // must not resurrect an address from an abandoned attempt.
  expect(takePendingEmail()).toBe("");
});

test("a second handoff replaces the first rather than queueing", () => {
  setPendingEmail("satu@example.com");
  setPendingEmail("dua@example.com");
  expect(takePendingEmail()).toBe("dua@example.com");
  expect(takePendingEmail()).toBe("");
});
