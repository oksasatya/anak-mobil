import { useAhaSeen } from "@/features/onboarding/ahaSeen";
import { useDraft } from "@/features/onboarding/draft";
import { BASE_URL } from "@/shared/api/baseUrl";
import { purgeAllPersistedCache, queryClient } from "@/shared/api/queryClient";
import { clearSession, readSession } from "@/shared/session/secure";
import { bumpEpoch, setSignedOut } from "@/shared/session/store";
import { clearActiveVehicle } from "@/shared/vehicle/activeVehicle";

let inFlight: Promise<void> | null = null;

async function run(): Promise<void> {
  // FIRST. Everything below is cleanup; this is the part that makes a response
  // landing mid-cleanup get dropped instead of written. Without it, a request
  // that was already in flight writes fresh data into a cache that was just
  // cleared — which is how the next account sees the previous account's garage.
  //
  // It also gives "exactly one redirect" for free: ten requests failing to
  // refresh all call signOut, the first bumps the epoch, and the rest are the
  // same awaited promise.
  bumpEpoch();

  // Tell the API the session is over. Best-effort by design: the local session
  // ends whether or not this succeeds, because a person on a plane pressing
  // sign-out must still be signed out. A bare fetch rather than apiRequest —
  // importing the client here would close the cycle client.ts -> signOut.ts.
  const stored = await readSession();
  if (stored !== null) {
    // Bounded to 5s. Left unbounded, a captive portal held this open for as
    // long as iOS URLSession's own default (~60s) — and for that whole span
    // `setSignedOut()` below had not run yet, so every gate kept rendering the
    // garage of the account that had just pressed sign-out, and every other
    // request in flight sat behind the same wait. It is also what makes the
    // replay window `refresh.ts`'s `refreshPending` marker guards against
    // wider than it needs to be.
    //
    // `AbortSignal.timeout` is not usable here: React Native polyfills the
    // global `AbortSignal` from the `abort-controller` npm package
    // (`react-native/Libraries/Core/setUpXHR.js`), and that package's
    // `AbortSignal` has no static `timeout()` — calling it would throw before
    // `fetch` is ever reached. The bound is built from `AbortController` +
    // `setTimeout` instead.
    //
    // CORRECTED after the fix-batch review (ledger 58). `new AbortController()`
    // and its `setTimeout` used to sit above this try — the one line in the
    // failure-atomic span (the comment above `run()` says nothing between
    // `bumpEpoch()` and `setSignedOut()` may abort early) not provably safe,
    // since constructing an `AbortController` triggers React Native's lazy
    // `require` of the `abort-controller` polyfill on first access. Both now
    // construct inside the try, so a throw there is caught by the same swallow
    // as a failed fetch, and `clearTimeout` in `finally` still runs safely even
    // when `timer` was never assigned.
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      const controller = new AbortController();
      timer = setTimeout(() => controller.abort(), 5000);
      await fetch(`${BASE_URL}/auth/logout`, {
        method: "POST",
        headers: { authorization: `Bearer ${stored.access}` },
        signal: controller.signal,
      });
    } catch {
      // Deliberately swallowed. There is nothing to tell the person and
      // nothing to retry: the server's session expires on its own.
    } finally {
      clearTimeout(timer);
    }
  }

  // Everything from here is best-effort cleanup, guarded as one unit. Nothing
  // in this span may abort `run()` early: a rejection from
  // `purgeAllPersistedCache`'s underlying MMKV call, or from `clearSession`'s
  // Keychain delete when the device is locked under some configurations (the
  // same hazard `secure.ts` already concedes on its read path), must not
  // leave the tokens on disk with `status` still `"signedIn"` — the person who
  // tapped sign-out would see nothing happen and stay signed in on a device
  // that has just told them they aren't.
  try {
    // In-flight requests, cancelled. Anything Query is not managing is covered
    // by the epoch above.
    await queryClient.cancelQueries();
    queryClient.clear();

    // Unconditional. `sessionUserId()` is null on a path that genuinely
    // happens — cold start with a stored session, `fetchMe()` 401s before
    // `setSignedIn` ever runs, refresh gets refused, `client.ts` calls
    // `signOut()` with `store.user` still null — and a purge keyed on that id
    // could not run at all. The prefix sweep needs no id, so it is the actual
    // guarantee that this account's garage does not survive sign-out on disk.
    purgeAllPersistedCache();

    // Not sensitive on its own, but it belongs to the account that just
    // signed out: the next account has no car with this id, so every query
    // keyed on it would 404.
    clearActiveVehicle();

    // The half-finished car and the record of which aha screens have been
    // shown. Hygiene rather than the defence: `adoptUser` discards a draft
    // stamped with a different account on read, so a wipe missed here cannot
    // show one person's half-entered car to the next.
    useDraft.getState().clear();
    useAhaSeen.getState().clear();
  } finally {
    // Unconditional even if the try above threw. `.catch` rather than letting
    // a rejection here skip `setSignedOut()` too — the tokens might survive on
    // disk if the Keychain delete itself fails, but the app must still show as
    // signed out; a person who cannot delete a token should not be told they
    // are still logged in.
    await clearSession().catch(() => {});
    setSignedOut();
  }
}

/**
 * End the session: one transaction, one redirect.
 *
 * Single-flight, so ten simultaneous failures are one sign-out. The gates in
 * `gates.tsx` do the redirecting; nothing here navigates, because a navigation
 * call in a cleanup function is the second redirect the spec bans.
 */
export function signOut(): Promise<void> {
  inFlight ??= run().finally(() => {
    inFlight = null;
  });
  return inFlight;
}
