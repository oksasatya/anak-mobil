import { fetchMe } from "@/shared/api/me";
import { startPersistence } from "@/shared/api/queryClient";
import { readSession, writeSession } from "@/shared/session/secure";
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

/**
 * Finish a sign-in whose token write already succeeded.
 *
 * ## Why this exists, and why calling `signIn` twice is NOT the same thing
 *
 * `signIn` writes the pair FIRST and fetches `/me` second, so a rejection
 * leaves valid credentials on disk. Both auth screens offer a "Lanjutkan" that
 * resumes from there rather than re-submitting the form — a second
 * `POST /auth/register` would tell somebody their own new email is taken, and a
 * second `POST /auth/login` would mint a duplicate server session.
 *
 * The first version of that resume called `signIn(heldPair)` again, and that
 * is a session-revocation bug rather than a retry:
 *
 *   1. `POST /auth/login` -> 200 with pair P1. `writeSession(P1)`; disk = P1.
 *   2. `fetchMe()` 401s — server clock skew on `iat`/`nbf`, or any first-request
 *      rejection. `apiRequest`'s refresh branch now sees a stored session
 *      (step 1 just wrote it), so `ensureRefreshed()` rotates and writes P2.
 *      **P1's refresh token is now spent.**
 *   3. The retried `/me` fails too -> `signIn` rejects -> the screen holds P1.
 *   4. The person reconnects and taps Lanjutkan -> `signIn(P1)` ->
 *      `writeSession(P1)` **overwrites the live P2 with the spent P1**.
 *   5. The next 401 presents a spent refresh token. The server reads that as
 *      reuse and revokes every session on every device — see `secure.ts`'s own
 *      note on why a `refreshPending` marker is a definitive discard.
 *
 * So a resume must never re-run step 1. Whatever is on disk is at least as new
 * as the pair the screen is holding, precisely because a resume can only be
 * reached after a successful write. The passed pair is used ONLY when the disk
 * has somehow been emptied in between (a sign-out landing mid-flight), where
 * writing it back is the strictly better of two bad options: without it there
 * is nothing to authenticate with at all.
 *
 * `clearActiveVehicle()` is deliberately NOT repeated — `signIn` already ran
 * it before the write that got us here, and running it again would be a second
 * owner of that decision.
 *
 * Found in Task 2's review of Plan B.
 */
export async function resumeSignIn(tokens: {
  access_token: string;
  refresh_token: string;
  expires_in: number;
}): Promise<void> {
  const stored = await readSession();
  if (stored === null) {
    await writeSession({
      access: tokens.access_token,
      refresh: tokens.refresh_token,
    });
  }

  const user = await fetchMe();
  await startPersistence(user.id).catch(() => {});
  setSignedIn(user);
}
