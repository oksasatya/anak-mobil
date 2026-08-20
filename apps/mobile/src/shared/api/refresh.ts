import { BASE_URL } from "@/shared/api/baseUrl";
import { offlineError, narrowEnvelope, serverError, type ApiError } from "@/shared/api/errors";
import {
  clearSession,
  markRefreshPending,
  readSession,
  writeSession,
} from "@/shared/session/secure";
import { currentEpoch } from "@/shared/session/store";

/**
 * The one refresh in flight, if there is one.
 *
 * Single-flight is a correctness requirement here, not a performance tweak. The
 * server treats a refresh token presented twice as theft and answers by
 * revoking every session the account has — correctly, for a real theft. Two
 * concurrent requests that each notice a 401 and each refresh with the same
 * stored token look identical to that from the server's side, so the naive
 * implementation signs somebody out of their phone, tablet, and laptop at
 * exactly the moment the app is making the most requests.
 */
let inFlight: Promise<string> | null = null;

interface TokenPair {
  access_token: string;
  refresh_token: string;
}

function narrowPair(data: unknown): TokenPair | null {
  if (typeof data !== "object" || data === null) return null;
  const pair = data as Partial<TokenPair>;
  return typeof pair.access_token === "string" && typeof pair.refresh_token === "string"
    ? { access_token: pair.access_token, refresh_token: pair.refresh_token }
    : null;
}

async function run(): Promise<string> {
  // Captured before anything awaits. A sign-out landing between here and the
  // `writeSession` below bumps the epoch; the check right before that write
  // catches it and refuses to resurrect a pair the sign-out transaction just
  // wiped from storage.
  const epoch = currentEpoch();
  const stored = await readSession();
  // CORRECTED after Task 12's review (ledger 35). The original tested only
  // `stored === null`, so the pending marker was consulted at cold start and
  // NOWHERE on the in-session path — the one place the catastrophe it guards
  // against actually happens. The sequence it allowed: a refresh rotates the
  // token server-side, its response is lost, the sign-out that follows spends
  // its first awaits on an untimed logout POST over the network that just
  // failed, and any second request 401ing in that window replays the SPENT
  // refresh token. The server reads that as reuse and revokes every session on
  // every device. Multiple concurrent 401s is the exact premise single-flight
  // exists for, so the window is well populated.
  //
  // Same predicate the bootstrap already uses. Must land with ledger 36.
  if (stored === null || stored.refreshPending) {
    throw { kind: "unauthorized", message: "Sesi kamu sudah berakhir." } satisfies ApiError;
  }

  // Written BEFORE the request leaves, which is the only ordering that records
  // anything about the case this exists for: a response that never arrives. If
  // the app is killed here, the next launch finds the marker, discards these
  // credentials, and asks for a password — rather than presenting a token the
  // server may already have rotated, which would look like a replay and cost
  // the account every one of its sessions.
  //
  // Residual, traced and left as-is (branch-gate finding 1's sibling): this
  // read-then-write is the same unguarded-storage-write class as the
  // `clearSession()` below, but its window is two adjacent awaits with no
  // network call between them — sub-millisecond. A sign-out plus a full
  // sign-in cannot land inside that gap the way they land inside a hung
  // fetch, so there is nothing here for the epoch check to actually catch.
  await markRefreshPending();

  let response: Response;
  // Bounded to 10s — branch-gate finding 2. `useBootstrap`'s 5s abort on
  // `fetchMe` (bootstrap.ts) reaches only `apiRequest`'s first `send()`, never
  // this fetch — so an ordinary cold start more than an hour after the last
  // request (`ACCESS_TTL` is one hour) 401s, lands here, and used to hang
  // against a captive portal or a stalled API for as long as iOS URLSession's
  // own ~60s default, freezing the splash the whole time (`useBootstrap`
  // cannot flip `done` until this promise settles). One bound for every
  // caller, not a per-caller signal: `run()` is a single-flight promise
  // shared by N callers, so a per-caller abort would let one caller's unmount
  // cancel everybody else's refresh. `AbortSignal.timeout` is not an option
  // here — React Native's `abort-controller` polyfill has no static
  // `timeout()`, see `signOut.ts:34-39` — so the bound is built from
  // `AbortController` + `setTimeout` instead, constructed inside this `try`
  // for the same reason `signOut.ts` does: a construction throw is caught by
  // the same catch as a failed fetch.
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    // A bare fetch, deliberately. Routing this through `apiRequest` would mean
    // a 401 here calls `ensureRefreshed` from inside `ensureRefreshed`.
    const controller = new AbortController();
    timer = setTimeout(() => controller.abort(), 10_000);
    response = await fetch(`${BASE_URL}/auth/refresh`, {
      method: "POST",
      headers: { "content-type": "application/json", "accept-language": "id" },
      body: JSON.stringify({ refresh_token: stored.refresh }),
      signal: controller.signal,
    });
  } catch {
    // The network failed — including this timeout aborting it — so the
    // server's state is unknown and the marker stays set. The credentials are
    // NOT discarded here — the next launch makes that call, once, with the
    // marker to tell it why.
    throw offlineError();
  } finally {
    clearTimeout(timer);
  }

  const body: unknown = await response.json().catch(() => null);
  const pair = response.ok ? narrowPair(narrowEnvelope(body)?.data) : null;

  // CORRECTED — branch-gate finding 1. Used to sit after the `pair === null`
  // block below and guard only `writeSession`, on the reasoning that the
  // server's ROTATE script already refuses a rotation once sign-out has
  // deleted the `sess:` key, so a race with sign-out is usually just a
  // wasted round trip. That reasoning covers the write; it says nothing
  // about `clearSession()` a few lines below, which had no guard of its
  // own. A refresh that started long before a sign-out and only answers
  // after it — a hung POST, the exact window `markRefreshPending` above
  // exists for — reaches `pair === null` (its token was revoked by the
  // sign-out) and deletes whatever a LATER sign-in has since written,
  // signing out a session that just started for no visible reason. Moved
  // here so it guards both branches; the check that used to sit after the
  // `pair === null` block is gone, subsumed by this one.
  if (currentEpoch() !== epoch) {
    throw { kind: "unauthorized", message: "Sesi sudah berganti." } satisfies ApiError;
  }

  if (pair === null) {
    // A refused refresh is final: the token is spent, replayed, or the session
    // is gone. Nothing to keep.
    await clearSession();
    throw response.ok
      ? serverError(narrowEnvelope(body)?.meta?.request_id)
      : ({
          kind: "unauthorized",
          message: "Sesi kamu sudah berakhir. Masuk lagi, ya.",
        } satisfies ApiError);
  }

  await writeSession({ access: pair.access_token, refresh: pair.refresh_token });
  return pair.access_token;
}

/**
 * Refresh once, however many callers ask.
 *
 * Every caller beyond the first awaits the same promise and gets the same new
 * access token.
 */
export function ensureRefreshed(): Promise<string> {
  inFlight ??= run().finally(() => {
    inFlight = null;
  });
  return inFlight;
}
