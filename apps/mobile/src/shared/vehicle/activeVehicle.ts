import { createMMKV } from "react-native-mmkv";
import { create } from "zustand";

/**
 * Client state, in its own store.
 *
 * Not in the query cache: that holds server state and is cleared, busted, and
 * keyed per account. Which car somebody is looking at is a preference, and it
 * should survive a cache bust the way a scroll position would.
 *
 * v4's Nitro-modules API creates instances via `createMMKV`, not `new MMKV()`
 * — `MMKV` itself is a type-only export (matches `api/queryClient.ts`).
 */
const storage = createMMKV({ id: "am.client" });
const KEY = "activeVehicleId";

interface ActiveVehicleState {
  id: string | null;
}

/**
 * Read synchronously at module load rather than in an effect.
 *
 * MMKV is synchronous, so there is no reason to render one frame with no
 * active vehicle and then reflow the garage when it arrives.
 */
const useStore = create<ActiveVehicleState>(() => ({
  id: storage.getString(KEY) ?? null,
}));

/** The car currently in focus, or null when none has been chosen. */
export function useActiveVehicleId(): string | null {
  return useStore((state) => state.id);
}

/** Choose the active car, or clear it with null. Persists immediately. */
export function setActiveVehicleId(id: string | null): void {
  if (id === null) {
    storage.remove(KEY);
  } else {
    storage.set(KEY, id);
  }
  useStore.setState({ id });
}

/**
 * Forget it entirely. Called by the sign-out transaction.
 *
 * Not because a vehicle id is a secret, but because it belongs to the previous
 * account: the next person has no car with that id, so every query keyed on it
 * would answer 404 and the garage would open on an error.
 */
export function clearActiveVehicle(): void {
  setActiveVehicleId(null);
}
