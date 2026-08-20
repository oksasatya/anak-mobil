import * as SecureStore from "expo-secure-store";

import type { ApiError } from "@/shared/api/errors";

/**
 * The session as it survives a cold start.
 *
 * One key holding one JSON record, deliberately, rather than three keys. Three
 * writes are three chances to be killed halfway and leave a refresh token
 * paired with an access token from a different pair; a single record cannot
 * half-write.
 *
 * `expires_in` is absent on purpose. The server sends a duration so a client
 * need not trust its own clock, and it may be used to refresh early — it may
 * never be used to decide a token is still valid. A 401 is the only authority,
 * and a stored expiry is an invitation to believe otherwise.
 */
export interface StoredSession {
  access: string;
  refresh: string;
  /**
   * Set immediately BEFORE a refresh request goes out, cleared once the new
   * pair is written.
   *
   * Finding it set at launch means the previous refresh's outcome is unknown:
   * the server may have rotated and the response may have been lost. Presenting
   * the old refresh token then looks exactly like a replayed stolen token, and
   * the server answers a replay by revoking every session on every device. So
   * the client discards its credentials and asks for a password instead. One
   * device asks for a login; the account does not lose the tablet.
   */
  refreshPending: boolean;
}

const KEY = "am.session";

/**
 * The stored session, or null when there is none.
 *
 * A read that throws is a session that cannot be used — the Keychain is
 * unavailable while the device is locked under some configurations — and the
 * recovery from "no session" and "unreadable session" is the same screen, so
 * they answer the same.
 */
export async function readSession(): Promise<StoredSession | null> {
  try {
    const raw = await SecureStore.getItemAsync(KEY);
    if (raw === null) return null;

    const parsed: unknown = JSON.parse(raw);
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      typeof (parsed as StoredSession).access !== "string" ||
      typeof (parsed as StoredSession).refresh !== "string"
    ) {
      return null;
    }
    const value = parsed as StoredSession;
    return {
      access: value.access,
      refresh: value.refresh,
      refreshPending: value.refreshPending === true,
    };
  } catch {
    return null;
  }
}

/**
 * Replace the stored session. Always clears `refreshPending`.
 *
 * Takes no `refreshPending` — the field is unconditionally overwritten below,
 * so a signature that accepted it would invite a caller to believe passing
 * `true` kept it set. `Omit` makes that belief a type error instead.
 */
export async function writeSession(value: Omit<StoredSession, "refreshPending">): Promise<void> {
  // `WHEN_UNLOCKED_THIS_DEVICE_ONLY`: the default (`WHEN_UNLOCKED`) is included
  // in an encrypted device backup and restored onto different hardware — restore
  // iPhone A's backup onto iPhone B and both devices hold the same refresh
  // token, and the first one to refresh gets the server to treat the other as a
  // thief and revoke every session on every device. `_THIS_DEVICE_ONLY` ties the
  // Keychain item to this device's Secure Enclave key, so it is simply absent
  // from the restore. Still `WHEN_UNLOCKED`, not `AFTER_FIRST_UNLOCK` — there is
  // no background refresh (Task 12), so nothing needs to read it before the
  // first unlock.
  await SecureStore.setItemAsync(KEY, JSON.stringify({ ...value, refreshPending: false }), {
    keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
  });
}

/**
 * Record that a refresh is about to be attempted.
 *
 * Written BEFORE the request, never after. A marker set after a response
 * arrives records nothing about the case it exists for — a response that never
 * arrived.
 */
export async function markRefreshPending(): Promise<void> {
  const current = await readSession();
  // Fail closed. `readSession` returns null for two different reasons — "no
  // session" and "session unreadable" (bad JSON, wrong shape, or the Keychain
  // rejecting the read because the screen locked mid-refresh) — and conflating
  // them is right for a read, where a transient lock must not destroy a good
  // session. It is wrong here: resolving quietly on "unreadable" would let the
  // refresh proceed with the marker never written, and if the response is then
  // lost, the next launch replays the already-rotated token — read by the
  // server as reuse, answered by revoking every session on every device.
  // Throwing routes the caller (refresh.ts's `run()`, which does not catch
  // this) to `signOut()` instead: one device asks for a password, the rest of
  // the account's sessions stay up.
  //
  // CORRECTED after the fix-batch review (ledger 46). A bare `Error` was the
  // odd one out — every other exit from this layer throws an `ApiError`-
  // shaped object — and it violated `ApiError.message`'s own written contract
  // at `errors.ts`: "Already in Bahasa Indonesia and ready to show. Never a
  // raw error string." The moment a Plan B screen rendered `err.message`,
  // somebody would have read the raw English string. Functionally unchanged:
  // `client.ts` tests `kind !== "offline"`, and "unauthorized" is still not
  // "offline", so `signOut()` still runs exactly as before.
  if (current === null) {
    throw {
      kind: "unauthorized",
      message: "Sesi tidak terbaca. Masuk lagi, ya.",
    } satisfies ApiError;
  }
  await SecureStore.setItemAsync(KEY, JSON.stringify({ ...current, refreshPending: true }), {
    keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
  });
}

/** Remove the session entirely. Part of the sign-out transaction. */
export async function clearSession(): Promise<void> {
  await SecureStore.deleteItemAsync(KEY);
}
