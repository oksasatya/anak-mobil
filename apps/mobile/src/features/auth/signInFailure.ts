import type { ApiError } from "@/shared/api/errors";

/**
 * The auth failure machinery, in a file with NO runtime imports.
 *
 * Split out of `api.ts` after Task 2's review. `api.ts` value-imports
 * `@/shared`, which reaches `react-native-mmkv` and `expo-router` and crashes
 * bun's transpiler on react-native's Flow-typed internals — so a test could
 * not import these and restated them instead. A restated contract is not a
 * contract: renaming `AuthTokens`' fields, typoing `isSignInFailure`'s key, or
 * dropping `asApiError`'s guard all left every assertion green. `ApiError` is
 * a type-only import from a module with zero runtime imports, so this file is
 * safe to import from a test and the real symbols are now pinned.
 */

/**
 * The wire shape, snake_case, exactly as the frozen contract states it.
 *
 * `Me` is camelCase in the same contract, which means the client does not
 * blanket-camelise — so these keys stay as the server sends them rather than
 * being tidied into a shape the parser would not produce.
 */
export interface AuthTokens {
  readonly access_token: string;
  readonly refresh_token: string;
  readonly token_type: string;
  readonly expires_in: number;
}

/**
 * The token pair, carried on the error when the POST succeeded but finishing
 * the session did not.
 *
 * It has to ride the error because TanStack Query destroys the success value
 * on the way to the error state — `mutation.js`'s reducer does
 * `case "error": return { ...state, data: void 0, ... }` (verified in the
 * installed query-core@5.101.4). So `mutation.data` is unconditionally
 * `undefined` whenever `isError` is true, and a retry keyed on it is dead code
 * that never runs. The tokens live in memory only, exactly where
 * `mutation.data` would have held them; mutations are never dehydrated
 * (`queryClient` sets `shouldDehydrateMutation: () => false`), so nothing
 * token-shaped reaches disk.
 */
export interface SignInFailure extends ApiError {
  readonly tokens: AuthTokens;
}

export function isSignInFailure(error: ApiError): error is SignInFailure {
  return "tokens" in error;
}

/**
 * Narrow an unknown rejection to the taxonomy, without losing its message.
 *
 * Spreading a real `Error` silently drops it: `Error.prototype.message` is an
 * own property but **non-enumerable**, so `{ ...new Error("Keystore
 * unavailable") }` is `{}`. That is not hypothetical here — `signIn` has two
 * steps that reject with real `Error`s rather than taxonomy literals
 * (`clearActiveVehicle()` through MMKV, and `writeSession()` through
 * expo-secure-store); only `fetchMe()` rejects with an `ApiError` object. The
 * first version of `finish` spread the error directly, so a Keystore failure
 * reached a screen with `message === undefined` and rendered **nothing at
 * all** — the person tapped a button that visibly did not respond. Found in
 * Task 3's review.
 */
export function asApiError(error: unknown): ApiError {
  if (typeof error === "object" && error !== null && "kind" in error) {
    return error as ApiError;
  }
  return {
    kind: "server",
    message: error instanceof Error ? error.message : "Ada gangguan. Coba lagi sebentar lagi.",
  };
}
