import { fetchMe } from "@/shared/api/me";
import { startPersistence } from "@/shared/api/queryClient";
import { writeSession } from "@/shared/session/secure";
import { setSignedIn } from "@/shared/session/store";
import { clearActiveVehicle } from "@/shared/vehicle/activeVehicle";

/**
 * The only way a session starts.
 *
 * Register and login both receive a token pair and hand it here. No screen
 * writes to secure storage itself — centralising that write is most of what
 * this module exists for, and a second copy of it in a form's `onSuccess` is
 * how a token ends up somewhere it should not be.
 *
 * The order is fixed and is an acceptance criterion:
 *
 *   1. write the pair (clearing any `refresh_pending` marker, which
 *      `writeSession` does unconditionally)
 *   2. GET /me
 *   3. start the per-account cache and await the restore
 *   4. populate the store and flip `status` to `signedIn`, together
 *
 * Flipping status before the user is loaded would let a group layout render
 * against `user === null`. Fetching before writing would send the request with
 * no token. Starting the cache before flipping status mirrors what
 * `useBootstrap` already does, and for the same reason: the first frame after
 * `signedIn` should be warm, not empty-then-popping.
 *
 * CORRECTED after Task 16 (ledger 18). `startPersistence` used to have
 * exactly one call site in the whole plan — `useBootstrap` — which meant the
 * session somebody actually signs into ran entirely unpersisted: not just the
 * restore skipped (correct, there is nothing to restore on a fresh sign-in)
 * but the *subscribe* too, so nothing was ever written for that session and
 * the next launch's bootstrap found an empty cache.
 *
 * `expires_in` is accepted and deliberately discarded: it is a hint, a 401 is
 * the only authority, and a stored expiry is an invitation to believe
 * otherwise. It is in the signature because it is in the server's response, and
 * discarding it here is clearer than making every caller destructure around it.
 *
 * If step 2 rejects offline, the pair from step 1 is already on disk: the
 * next launch signs the account in without another login, which is correct
 * behaviour — but a caller that retries `signIn` on that rejection mints a
 * second server session that lives to the refresh token's full TTL rather than
 * reusing the one just written. `onError` needs to know that before it retries.
 *
 * Step 3 can reject too (ledger 92) — a truncated cache from a previous
 * kill, same hazard `bootstrap.ts` documents for its own `startPersistence`
 * call. Persisting is a cache optimisation and must never be able to fail a
 * sign-in the way step 2 already can, so its rejection is swallowed here.
 */
export async function signIn(tokens: {
  access_token: string;
  refresh_token: string;
  expires_in: number;
}): Promise<void> {
  // `signIn` is the only route into a session (the FROZEN CONTRACT above), so
  // this one guard covers every path that starts one: normal login, the
  // bootstrap discarding credentials on a `refreshPending` marker, and an
  // Android backup restore where MMKV survives a reinstall but
  // Keystore-backed SecureStore does not. Without it, whichever account's id
  // was last written to `am.client` stays there, and the next account's shell
  // opens on a car it does not own.
  clearActiveVehicle();

  await writeSession({
    access: tokens.access_token,
    refresh: tokens.refresh_token,
  });

  const user = await fetchMe();
  await startPersistence(user.id).catch(() => {});
  setSignedIn(user);
}
