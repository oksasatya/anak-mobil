import { useEffect, useState } from "react";

import { fetchMe } from "@/shared/api/me";
import { purgeAllPersistedCache, startPersistence } from "@/shared/api/queryClient";
import { clearSession, readSession } from "@/shared/session/secure";
import { currentEpoch, setSignedIn, setSignedOut } from "@/shared/session/store";

/**
 * Restore the session, once, before anything renders.
 *
 * The order is the whole point:
 *
 *   1. read secure storage
 *   2. a set `refreshPending` marker means the previous refresh's outcome is
 *      UNKNOWN — the server may already have rotated. Presenting that refresh
 *      token would look like a replay, and the server answers a replay by
 *      revoking every session on every device. So the credentials are discarded
 *      and this one device asks for a password.
 *   3. GET /me, which also proves the access token still works
 *   4. start the per-account cache and AWAIT the restore, so the first frame is
 *      warm rather than empty-then-popping
 *   5. only then flip status — and only if nothing else already did (see the
 *      epoch guard below)
 *
 * Accepted risk, handed forward from `signOut.ts`'s own deliberate
 * `clearSession().catch(() => {})` (ledger 61): a failed Keychain delete
 * leaves valid tokens on disk even though the app already shows `signedOut` —
 * correct for that session, because a person who cannot delete a token must
 * still see themselves as signed out. But the auth epoch lives in memory and
 * resets to 0 on every relaunch, so this bootstrap has no way to tell "these
 * tokens are stale because a delete failed" from "this is an ordinary cold
 * start" — it reads the pair, `/me` answers, and the account is signed back in
 * on this one device. Deliberately not closed here: a retry does not help
 * (the delete fails while the Keychain is locked, and an immediate retry finds
 * it still locked), and a tombstone durable enough to survive the very failure
 * the delete itself hit would have to live outside the Keychain — which
 * reopens the same "is this write durable" question one layer down for no
 * real gain. The blast radius is bounded: it is per-device, not per-account,
 * the tokens are still bound by the server's own TTL and rotation, and it can
 * only ever resurrect the same person's own session on their own device.
 */
export function useBootstrap(): boolean {
  const [done, setDone] = useState(false);

  useEffect(() => {
    let alive = true;
    // Captured before anything awaits. `alive` above guards unmount only — it
    // says nothing about `signOut()` running WHILE this effect is in flight.
    // Without this, a sign-out landing between `startPersistence` and
    // `setSignedIn` below would be overwritten: `setSignedIn(user)` lands
    // AFTER `setSignedOut()` already ran, resurrecting a live user object over
    // a Keychain that was just emptied — a garage flash, then 401s.
    const epoch = currentEpoch();

    void (async () => {
      const stored = await readSession();

      if (stored === null || stored.refreshPending) {
        // A `refreshPending` marker is a definitive credential discard (the
        // same kind `signOut` treats as requiring a purge, ledger 22): this
        // device is never presenting that refresh token again. Left
        // unpurged, the account's garage — plate, VIN, service cost — stays
        // on disk in plain unencrypted MMKV indefinitely, since nothing else
        // ever sweeps this account's cache key (ledger 94).
        if (stored?.refreshPending === true) {
          await clearSession();
          purgeAllPersistedCache();
        }
        if (alive) setSignedOut();
      } else {
        // `fetchMe` is bounded to 5s here, matching the budget `signOut.ts`
        // (lines 50-63) already uses for its own logout POST and for the
        // same reason: left unbounded, a captive portal or a hung API holds
        // this open for iOS URLSession's own ~60s default, and `_layout.tsx`
        // renders nothing until `done` flips — the splash looks frozen the
        // whole time (ledger 93). The controller is constructed inside the
        // try, same as `signOut.ts`, so a construction throw is caught by
        // the same catch below; `AbortSignal.timeout` is not an option here
        // for the reason documented at `signOut.ts:34-39` (React Native's
        // `abort-controller` polyfill has no static `timeout()`).
        //
        // On abort, `fetchMe` rejects, `apiRequest`'s catch turns that into
        // `offlineError()`, and the catch below treats it like any other
        // bootstrap failure: welcome screen, tokens kept (nothing here calls
        // `clearSession`), next launch tries again.
        let timer: ReturnType<typeof setTimeout> | undefined;
        try {
          const controller = new AbortController();
          timer = setTimeout(() => controller.abort(), 5000);
          const user = await fetchMe(controller.signal);
          // Persisting is a cache optimisation, never a reason to sign
          // someone out of a session `/me` already proved valid (ledger 91).
          // Verified against the installed package: `persistQueryClientRestore`
          // calls `removeClient()` and rethrows, so an unreadable/truncated
          // cache on disk would otherwise reach this `catch` and bounce a
          // valid session to the welcome screen. Ceiling: `.then(subscribe)`
          // never runs on this path, so nothing persists for the rest of
          // this session — the next launch calls `startPersistence` again
          // and re-arms it.
          await startPersistence(user.id).catch(() => {});
          if (alive && currentEpoch() === epoch) setSignedIn(user);
        } catch {
          // Any failure here — an expired session the refresh could not save,
          // an unreachable API, or the 5s abort above — lands on the welcome
          // screen. The tokens stay put when the cause was the network: the
          // client has already discarded them if the server actually refused.
          if (alive) setSignedOut();
        } finally {
          clearTimeout(timer);
        }
      }

      if (alive) setDone(true);
    })();

    return () => {
      alive = false;
    };
  }, []);

  return done;
}
