/**
 * The retry-after-partial-success path.
 *
 * This exists because the first attempt at the fix was dead code: it keyed on
 * `mutation.data`, and query-core's reducer does
 * `case "error": return { ...state, data: void 0, ... }`, so that value is
 * always `undefined` alongside an error. The pair rides the error instead.
 *
 * CORRECTED after Task 2's review. These used to be RESTATEMENTS of
 * `isSignInFailure`, `asApiError`, and `AuthTokens`, because `api.ts`
 * value-imports `@/shared` and crashes bun's transpiler on react-native's
 * internals. A restated contract is not a contract — renaming `AuthTokens`'
 * fields, typoing the `in` check, or dropping `asApiError`'s guard all left
 * every assertion here green. The definitions now live in
 * `features/auth/signInFailure.ts`, which imports nothing at runtime, so these
 * tests pin the real thing.
 */
import { expect, test } from "bun:test";

import {
  asApiError,
  isSignInFailure,
  type AuthTokens,
  type SignInFailure,
} from "@/features/auth/signInFailure";
import type { ApiError } from "@/shared/api/errors";

const TOKENS: AuthTokens = {
  access_token: "a",
  refresh_token: "r",
  token_type: "Bearer",
  expires_in: 3600,
};

/**
 * The one line of `finish()` that still cannot be imported (it awaits
 * `signIn`, which reaches react-native). Everything it composes IS imported.
 */
function rethrown(error: ApiError, tokens: AuthTokens): SignInFailure {
  return { ...asApiError(error), tokens };
}

test("an ordinary request failure is NOT a sign-in failure", () => {
  // The screens must still re-submit on this one — the POST never landed.
  expect(isSignInFailure({ kind: "offline", message: "Tidak ada koneksi." })).toBe(false);
  expect(isSignInFailure({ kind: "unauthorized", message: "Email atau password salah." })).toBe(
    false,
  );
  expect(
    isSignInFailure({ kind: "validation", message: "x", fields: { email: "sudah terdaftar" } }),
  ).toBe(false);
});

test("a failure after a successful POST carries the pair and is recognised", () => {
  const failure = rethrown({ kind: "offline", message: "Tidak ada koneksi." }, TOKENS);

  expect(isSignInFailure(failure)).toBe(true);
  if (!isSignInFailure(failure)) throw new Error("unreachable");
  // The screens hand this straight back to signIn, which takes the WIRE shape:
  // snake_case, with expires_in required. Renaming a field here breaks the
  // resume silently, because signIn would receive undefined tokens.
  expect(failure.tokens.access_token).toBe("a");
  expect(failure.tokens.refresh_token).toBe("r");
  expect(failure.tokens.expires_in).toBe(3600);
});

test("rethrowing preserves the original kind and message", () => {
  // The screens still render this error; losing `kind` would turn an offline
  // interruption into a generic server fault.
  const failure = rethrown({ kind: "offline", message: "Tidak ada koneksi." }, TOKENS);
  expect(failure.kind).toBe("offline");
  expect(failure.message).toBe("Tidak ada koneksi.");
});

test("the pair is the only thing that distinguishes the two cases", () => {
  // Both are `kind: "offline"` — there is no other signal. If this ever stops
  // being true the screens can simplify; until then the tokens ARE the signal.
  const plain: ApiError = { kind: "offline", message: "Tidak ada koneksi." };
  const after = rethrown(plain, TOKENS);
  expect(after.kind).toBe(plain.kind);
  expect(after.message).toBe(plain.message);
  expect(isSignInFailure(plain)).toBe(false);
  expect(isSignInFailure(after)).toBe(true);
});

test("a real Error keeps its message — a bare spread would silently drop it", () => {
  // `Error.prototype.message` is an OWN property but NON-ENUMERABLE, so
  // `{ ...new Error("boom") }` is `{}`. The first version of `finish()` spread
  // the rejection directly, and `signIn` has two steps that reject with real
  // Errors rather than taxonomy literals — clearActiveVehicle() through MMKV
  // and writeSession() through expo-secure-store. A Keystore failure therefore
  // reached the screen with `message === undefined` and rendered NOTHING: the
  // person tapped a button that visibly did not respond.
  const raw = new Error("Keystore unavailable");

  // The defect, pinned so it cannot come back unnoticed.
  expect(Object.keys({ ...raw })).toEqual([]);
  expect(({ ...raw } as { message?: string }).message).toBeUndefined();

  // The fix.
  const normalised = { ...asApiError(raw), tokens: TOKENS };
  expect(normalised.message).toBe("Keystore unavailable");
  expect(normalised.kind).toBe("server");
  expect(isSignInFailure(normalised)).toBe(true);
});

test("a non-Error rejection still gets a readable message", () => {
  // A string throw, a rejected null, anything at all: never undefined.
  const normalised = { ...asApiError("something odd"), tokens: TOKENS };
  expect(typeof normalised.message).toBe("string");
  expect(normalised.message.length).toBeGreaterThan(0);
});

test("a taxonomy error passes through untouched", () => {
  const offline: ApiError = { kind: "offline", message: "Tidak ada koneksi." };
  expect(asApiError(offline)).toBe(offline);
});
