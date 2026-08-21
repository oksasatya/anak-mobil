import { useMutation, type UseMutationResult } from "@tanstack/react-query";

import { apiRequest, signIn, type ApiError } from "@/shared";

import { asApiError, isSignInFailure, type AuthTokens, type SignInFailure } from "./signInFailure";

// Re-exported so callers keep one import site. The definitions live in a
// react-native-free file so the test suite can import the REAL ones — see
// that file's header.
export { asApiError, isSignInFailure };
export type { AuthTokens, SignInFailure };

export interface RegisterInput {
  readonly email: string;
  readonly username: string;
  readonly password: string;
}

export interface LoginInput {
  readonly email: string;
  readonly password: string;
}

/**
 * Destructured rather than `body: input`, so the wire shape is provable by
 * reading this function.
 *
 * TypeScript's excess-property check fires only on fresh object literals, so
 * `mutate(parsed.data)` — where `parsed.data` is `registerSchema`'s output and
 * therefore carries `consent: true` — compiled and shipped a fourth field. The
 * server's `RegistrationRequest` is an explicit three-field DTO and serde
 * ignores unknown keys, so nothing was exploitable; the defect is that the
 * client's payload was not pinned to the frozen contract, and the next field
 * added to the form would have shipped silently. Corrected after Task 1's
 * review.
 */
export function registerAccount({ email, username, password }: RegisterInput): Promise<AuthTokens> {
  return apiRequest<AuthTokens>("/auth/register", {
    method: "POST",
    body: { email, username, password },
  });
}

export function loginWithPassword({ email, password }: LoginInput): Promise<AuthTokens> {
  return apiRequest<AuthTokens>("/auth/login", { method: "POST", body: { email, password } });
}

/**
 * `signal` is required rather than optional: every caller of this endpoint is
 * a keystroke-driven check that MUST be cancellable, and an optional signal is
 * one that eventually gets omitted.
 *
 * `encodeURIComponent` closes the injection-shaped hole — `apiRequest`
 * interpolates `path` with no encoding and says so — but it does NOT neutralise
 * dot segments: `encodeURIComponent("..") === ".."`, so a literal `..` reaches
 * `fetch`'s URL parser and normalises away. `BASE_URL` is a bare origin, so the
 * worst case is a 404 on the same host with no session attached; there is
 * nothing to forge. Callers gate on `USERNAME_PATTERN` before calling, which is
 * where that belongs — a guard here would make the client DECIDE a name is
 * unacceptable, which the spec forbids. Noted after Task 1's review.
 */
export function usernameAvailability(
  username: string,
  signal: AbortSignal,
): Promise<{ available: boolean }> {
  return apiRequest<{ available: boolean }>(
    `/usernames/${encodeURIComponent(username)}/availability`,
    { signal },
  );
}

/**
 * Success hands the pair to Plan A and stops.
 *
 * No navigation here. The (auth) group's layout redirects when the session
 * status flips, which is what keeps the spec's "exactly one redirect" true —
 * a replace() in this callback would be a second owner of the same decision.
 *
 * ## A CALLER MUST NOT RE-SUBMIT ON ERROR. Read this before writing `onError`.
 *
 * TanStack Query awaits `onSuccess` and routes its rejection into the
 * mutation's error state, so `isError` does NOT mean the request failed.
 * `signIn` writes the token pair to SecureStore and only THEN calls
 * `fetchMe()` — its own comment says so — which makes this sequence real:
 *
 *   201 Created -> tokens on disk -> fetchMe() rejects offline
 *   -> mutation is `error` with kind "offline" -> screen offers "coba lagi"
 *   -> a second POST /auth/register -> 409 "Email ini sudah terdaftar."
 *
 * The person is told their own thirty-second-old email is taken, while valid
 * credentials sit on the device. On login the same path mints a second server
 * session that lives to the refresh token's full TTL.
 *
 * There is no automatic retry to worry about: `queryClient` sets no
 * `mutations` block, so v5's mutation default of 0 applies. The retry here is
 * a human finger, which is worse, because the UI invites it.
 *
 * `mutation.data` CANNOT be used to detect this — query-core's reducer does
 * `case "error": return { ...state, data: void 0, ... }`, so it is always
 * `undefined` alongside an error, and a retry keyed on it never fires. That
 * was the first attempt at this fix and it was dead code; Task 2's writer
 * caught it by reading the installed package rather than implementing what it
 * was told.
 *
 * So: on error, call `isSignInFailure(error)`. When it is true the POST
 * already succeeded — retry with `signIn(error.tokens)` and NEVER re-submit
 * the form. Found in Task 1's review, corrected after Task 2's.
 */
/**
 * Hand a successful 2xx to Plan A, and if THAT fails, say so without losing
 * the pair.
 *
 * `signIn` writes the tokens to SecureStore first and only then calls
 * `fetchMe()`, so a rejection here means the account exists, the credentials
 * are on the device, and only the last step is missing. Re-submitting the form
 * in that state asks the server to register the same email twice.
 */
async function finish(tokens: AuthTokens): Promise<void> {
  try {
    await signIn(tokens);
  } catch (error: unknown) {
    throw { ...asApiError(error), tokens } satisfies SignInFailure;
  }
}

export function useRegister(): UseMutationResult<AuthTokens, ApiError, RegisterInput> {
  return useMutation<AuthTokens, ApiError, RegisterInput>({
    mutationFn: registerAccount,
    onSuccess: finish,
  });
}

export function useLogin(): UseMutationResult<AuthTokens, ApiError, LoginInput> {
  return useMutation<AuthTokens, ApiError, LoginInput>({
    mutationFn: loginWithPassword,
    onSuccess: finish,
  });
}
