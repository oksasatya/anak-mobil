import { createMMKV } from "react-native-mmkv";
import type { StateStorage } from "zustand/middleware";

/**
 * One MMKV instance behind both onboarding stores.
 *
 * Its own instance rather than `am.client`: everything in here is discarded
 * as a unit the moment onboarding finishes or the account changes, and a
 * separate file makes that wipe a single operation rather than a list of keys
 * somebody has to keep in sync.
 *
 * v4's Nitro-modules API creates instances via `createMMKV`, not `new MMKV()`
 * — `MMKV` itself is a type-only export, and the delete is `remove`, not
 * `delete` (matches `shared/vehicle/activeVehicle.ts`).
 */
const mmkv = createMMKV({ id: "am.onboarding" });

export const onboardingStorage: StateStorage = {
  getItem: (name) => mmkv.getString(name) ?? null,
  setItem: (name, value) => mmkv.set(name, value),
  removeItem: (name) => mmkv.remove(name),
};
