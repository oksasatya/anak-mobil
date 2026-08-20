import { createSyncStoragePersister } from "@tanstack/query-sync-storage-persister";
import { QueryClient } from "@tanstack/react-query";
import { persistQueryClient } from "@tanstack/react-query-persist-client";
import { createMMKV } from "react-native-mmkv";

/**
 * Server state, cached. Never tokens — those live in expo-secure-store and
 * nowhere else, and this store is plain unencrypted MMKV.
 *
 * v4's Nitro-modules API creates instances via `createMMKV`, not `new MMKV()`
 * — `MMKV` itself is a type-only export.
 */
const storage = createMMKV({ id: "am.query" });

/** MMKV is sync and names its methods differently; the persister wants Storage. */
const mmkvStorage = {
  getItem: (key: string) => storage.getString(key) ?? null,
  setItem: (key: string, value: string) => {
    storage.set(key, value);
  },
  removeItem: (key: string) => {
    storage.remove(key);
  },
};

/**
 * One client for the app's whole life.
 *
 * A module-level singleton rather than a per-account instance, deliberately.
 * Swapping the client on sign-in would mean remounting the provider, and a
 * changing `key` on anything wrapping the route tree unmounts every screen and
 * throws away its state. The cache is emptied by `signOut` instead.
 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // AM-18 reads from cache. Five minutes fresh, a day usable while a
      // refetch happens behind it.
      staleTime: 5 * 60 * 1000,
      gcTime: 24 * 60 * 60 * 1000,
      // The client already retries a 401 exactly once, at the transport layer.
      // Retrying a 401, a 422, or a 429 here would multiply it.
      retry: 1,
    },
    // CORRECTED after the fix-batch review (ledger 59). Was
    // `dehydrateOptions.shouldDehydrateMutation` on the `persistQueryClient`
    // call below — a property of that one call site. `query-core` resolves
    // `options.shouldDehydrateMutation ?? client.getDefaultOptions().dehydrate
    // ?.shouldDehydrateMutation ?? default`, so a client-level default covers
    // any future `dehydrate`/persister call site instead of only this one.
    //
    // A paused mutation dehydrates with its `variables` by default. Once a
    // screen wires login through `useMutation`, that is `{email, password}`
    // reaching this plaintext MMKV store the moment the request pauses
    // offline.
    dehydrate: {
      shouldDehydrateMutation: () => false,
    },
  },
});

/** Namespaced per account — see `startPersistence`. Shared with the sweep below. */
const CACHE_PREFIX = "am.query.";

function cacheKey(userId: string): string {
  return `${CACHE_PREFIX}${userId}`;
}

let unsubscribe: (() => void) | null = null;

/**
 * Begin persisting this account's cache, and restore whatever it already has.
 *
 * **Keyed by account id**, so that switching accounts cannot surface the
 * previous one's data even if a delete were to fail. Sign-out deletes the cache
 * outright; this is the second lock on the same door, and the thing behind that
 * door is somebody's garage.
 *
 * Called only after `GET /me` has answered, because the id is the key. Until
 * then the client runs unpersisted, which is right: there is nothing worth
 * keeping before anybody is signed in.
 *
 * Awaits the restore, so the bootstrap gate can hold the first frame until the
 * cache is actually warm rather than letting a screen render empty and pop.
 */
export async function startPersistence(userId: string): Promise<void> {
  stopPersistence();

  const [unsub, restored] = persistQueryClient({
    queryClient,
    persister: createSyncStoragePersister({ storage: mmkvStorage, key: cacheKey(userId) }),
    maxAge: 24 * 60 * 60 * 1000,
    // Bump when a cached shape changes incompatibly; an old cache is then
    // discarded rather than deserialised into the wrong type.
    buster: "v1",
    // CORRECTED after the fix-batch review (ledger 59). `shouldDehydrateMutation`
    // moved to `queryClient`'s own `defaultOptions.dehydrate` above — a
    // client-level default, so it covers any future `dehydrate`/persister call
    // site rather than only this one.
  });

  unsubscribe = unsub;
  await restored;
}

/** Stop writing to disk. Does not delete what is already there. */
export function stopPersistence(): void {
  unsubscribe?.();
  unsubscribe = null;
}

/**
 * Remove every persisted query cache, regardless of whose id it is keyed
 * under. Part of the sign-out transaction — called unconditionally.
 *
 * `sessionUserId()` is null on a path that genuinely happens: a cold start
 * with a stored session calls `fetchMe()` before `setSignedIn` ever runs: a
 * 401 there means refresh gets refused and `signOut()` runs with `store.user`
 * still null. A purge keyed on the id cannot run on that path at all, and
 * without this sweep the account's garage — plate, VIN, service cost — stays
 * on disk in plain unencrypted MMKV after the account is signed out. A prefix
 * scan needs no id, so it is the actual guarantee.
 *
 * ponytail: linear in the number of stored keys — `getAllKeys()` plus a
 * string-prefix check per key. Fine at this app's scale (one cache key per
 * account that has ever signed in on this device); revisit if that stops
 * being small.
 */
export function purgeAllPersistedCache(): void {
  stopPersistence();
  for (const key of storage.getAllKeys()) {
    if (key.startsWith(CACHE_PREFIX)) storage.remove(key);
  }
}
