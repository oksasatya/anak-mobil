import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

import { onboardingStorage } from "./storage";

export interface AhaSeenState {
  readonly seen: readonly string[];
  markSeen(vehicleId: string): void;
  clear(): void;
}

/**
 * AM-56 AC4: "layar aha tidak muncul lagi untuk mobil yang sama".
 *
 * A list rather than a Set: a person has a handful of cars, `includes` over
 * single digits costs nothing, and a Set does not survive JSON persistence
 * without a serialiser nobody needs.
 */
export const useAhaSeen = create<AhaSeenState>()(
  persist(
    (set, get) => ({
      seen: [],
      markSeen: (vehicleId) => {
        if (get().seen.includes(vehicleId)) return;
        set({ seen: [...get().seen, vehicleId] });
      },
      clear: () => set({ seen: [] }),
    }),
    {
      name: "anakmobil.onboarding.ahaSeen",
      storage: createJSONStorage(() => onboardingStorage),
    },
  ),
);
