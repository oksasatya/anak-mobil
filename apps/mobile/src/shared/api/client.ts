import { ensureRefreshed } from "@/shared/api/refresh";
import { BASE_URL } from "@/shared/api/baseUrl";
import {
  narrowEnvelope,
  offlineError,
  serverError,
  toApiError,
  type ApiError,
} from "@/shared/api/errors";
import { readSession } from "@/shared/session/secure";
import { currentEpoch } from "@/shared/session/store";
import { signOut } from "@/shared/session/signOut";

export interface RequestInit_ {
  method?: string;
  body?: unknown;
  signal?: AbortSignal;
}

async function send(path: string, init: RequestInit_, token: string | null): Promise<Response> {
  const headers: Record<string, string> = {
    "content-type": "application/json",
    // The API answers in Bahasa Indonesia by default; sending it makes the
    // choice explicit rather than accidental.
    "accept-language": "id",
  };
  if (token !== null) headers.authorization = `Bearer ${token}`;

  // No X-Request-Id is sent, deliberately. The server always mints its own and
  // never reads an inbound one — a caller-supplied value reaching a log line is
  // how log injection works — so sending one would be ignored at best.
  return fetch(`${BASE_URL}${path}`, {
    method: init.method ?? "GET",
    headers,
    body: init.body === undefined ? undefined : JSON.stringify(init.body),
    signal: init.signal,
  });
}

/**
 * One request, and the envelope unwrapped.
 *
 * `apiRequest<Me>("/me")` resolves a `Me` — never a `{meta, data, error}`. The
 * envelope is transport; a caller wants the thing. Anything else throws an
 * `ApiError` from the taxonomy, already carrying text a person can read.
 *
 * On a 401 it refreshes ONCE, through the single-flight promise, and retries
 * the request ONCE. A second 401 after a successful refresh means the token was
 * never the problem, and a retry loop would only make it a longer failure.
 *
 * There is no proactive refresh anywhere. `expires_in` is a hint the server
 * sends so a client need not trust its own clock; a 401 is the only authority
 * on whether a token still works.
 *
 * `path` is interpolated into the URL with no encoding — callers encode their
 * own path segments (`encodeURIComponent`) before calling. The server's
 * canonicaliser is the actual authority on identifiers like a username, so
 * this is not a safety gap; it is a contract callers must not assume away.
 */
export async function apiRequest<T>(path: string, init: RequestInit_ = {}): Promise<T> {
  // Captured before the request goes out. If sign-out happens while this is in
  // flight, the response is dropped rather than returned — which is what stops
  // a late response writing the previous account's data into a cache that was
  // just cleared.
  const epoch = currentEpoch();

  const stored = await readSession();
  let response: Response;
  try {
    response = await send(path, init, stored?.access ?? null);
  } catch {
    // A rejected fetch in React Native is a TypeError, and it is the only
    // connectivity signal there is. That is why no NetInfo dependency exists.
    throw offlineError();
  }

  // CORRECTED after the fix-batch review (ledger 56). The epoch check further
  // below, after the response body is read, only catches a refresh born
  // BEFORE bumpEpoch() — one born DURING sign-out captures the already-bumped
  // value and never trips there.
  // signOut() bumps the epoch first, then spends up to 5s on the logout POST
  // plus teardown before clearSession(); any request 401ing in that window
  // used to reach ensureRefreshed() and could write a fresh live pair onto a
  // device that had just signed out. Testing the epoch here too keeps an
  // apiRequest already overtaken by a sign-out out of the refresh branch
  // entirely. Residual, accepted as narrow (same class as ledger 8's
  // ponytail ceiling): a brand-new apiRequest issued mid-sign-out captures
  // the already-bumped epoch at its own top and is not caught by this check.
  if (response.status === 401 && stored !== null && currentEpoch() === epoch) {
    let fresh: string;
    try {
      fresh = await ensureRefreshed();
    } catch (err: unknown) {
      // CORRECTED after Task 12's review (ledger 36). The original said
      // `await signOut()` unconditionally, which discarded the credentials on a
      // transient network blip — contradicting the spec's session contract,
      // `refresh.ts`'s own comment, the bootstrap note in Task 16, and this
      // task's acceptance criterion "a network failure during refresh does not
      // clear credentials". Somebody entering a tunnel with an expired access
      // token was signed out and had to re-enter a password.
      //
      // Only a REFUSED refresh is final. An offline one leaves the credentials
      // alone; the next launch makes that call. This is only safe alongside the
      // pending-marker guard in `refresh.ts` (ledger 35) — without it, keeping
      // the credentials turns the next 401 into a replay of a spent token.
      //
      // CORRECTED after the fix-batch review (ledger 55). `signOut()` can
      // itself reject (a Keychain delete, an MMKV call), and this await had no
      // guard of its own — that rejection reached the caller instead of `err`,
      // so `apiRequest` surfaced the sign-out's teardown failure rather than
      // the ApiError it actually failed with.
      if ((err as ApiError).kind !== "offline") await signOut().catch(() => {});
      throw err;
    }
    try {
      response = await send(path, init, fresh);
    } catch {
      throw offlineError();
    }
  }

  const body: unknown = await response.json().catch(() => null);

  if (currentEpoch() !== epoch) {
    // Signed out while this was in flight. Nothing here belongs to whoever is
    // using the app now.
    throw { kind: "unauthorized", message: "Sesi sudah berganti." } satisfies ApiError;
  }

  if (!response.ok) throw toApiError(response.status, body);

  const envelope = narrowEnvelope(body);
  if (envelope === null || envelope.data === undefined) {
    // A 2xx that is not our envelope. The taxonomy calls that a server fault,
    // because from the client's side it is indistinguishable from one.
    throw serverError(envelope?.meta?.request_id);
  }

  return envelope.data as T;
}
