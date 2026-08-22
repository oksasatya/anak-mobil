import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

import { onboardingStorage } from "./storage";

export const WIZARD_STEPS = ["brand", "model", "generation", "year", "variant", "photo"] as const;

export type WizardStep = (typeof WIZARD_STEPS)[number];

export interface Choice {
  readonly id: string;
  readonly name: string;
}

export interface GenerationChoice extends Choice {
  readonly yearStart: number;
  readonly yearEnd: number | null;
  readonly years: string;
}

/**
 * Bumped whenever the persisted shape changes. A draft written by an older
 * shape is discarded rather than migrated — a half-restored wizard is worse
 * than starting the six steps again, and a migration for a draft measured in
 * minutes of a person's time is not worth writing.
 */
export const DRAFT_VERSION = 1;

interface DraftData {
  version: number;
  /** Stamped on first write; a draft belonging to another account is discarded. */
  userId: string | null;
  displayName: string;
  step: WizardStep;
  brand: Choice | null;
  model: Choice | null;
  generation: GenerationChoice | null;
  year: number | null;
  variant: Choice | null;
  /** Distinct from `variant === null`: "not chosen yet" cannot advance, "skipped" can. */
  variantSkipped: boolean;
}

const EMPTY: DraftData = {
  version: DRAFT_VERSION,
  userId: null,
  displayName: "",
  step: "brand",
  brand: null,
  model: null,
  generation: null,
  year: null,
  variant: null,
  variantSkipped: false,
};

export interface DraftState extends DraftData {
  setDisplayName(name: string): void;
  setBrand(choice: Choice): void;
  setModel(choice: Choice): void;
  setGeneration(choice: GenerationChoice): void;
  setYear(year: number): void;
  setVariant(choice: Choice): void;
  skipVariant(): void;
  goTo(step: WizardStep): void;
  adoptUser(userId: string): void;
  clear(): void;
}

export const useDraft = create<DraftState>()(
  persist(
    (set, get) => ({
      ...EMPTY,

      setDisplayName: (displayName) => set({ displayName }),

      // The cascade, and the guard that makes AM-113 AC1 work.
      //
      // Choosing a different brand invalidates everything below it — a Civic
      // generation under Toyota is not a thing. But going BACK to the brand
      // step and re-confirming the SAME brand must clear nothing, or "kembali
      // ke langkah mana pun tanpa kehilangan isian" is false. Hence the
      // early return on an unchanged id, repeated at each level.
      setBrand: (brand) => {
        if (get().brand?.id === brand.id) return;
        set({
          brand,
          model: null,
          generation: null,
          year: null,
          variant: null,
          variantSkipped: false,
        });
      },

      setModel: (model) => {
        if (get().model?.id === model.id) return;
        set({ model, generation: null, year: null, variant: null, variantSkipped: false });
      },

      setGeneration: (generation) => {
        if (get().generation?.id === generation.id) return;
        // Year is cleared because the new generation's range may not contain
        // it. Variant is cleared because variants hang off the generation.
        set({ generation, year: null, variant: null, variantSkipped: false });
      },

      // Year does NOT cascade: /catalog/generations/{id}/variants is keyed by
      // the generation alone, so the variant list does not depend on the year.
      setYear: (year) => set({ year }),

      setVariant: (variant) => set({ variant, variantSkipped: false }),
      skipVariant: () => set({ variant: null, variantSkipped: true }),

      goTo: (step) => set({ step }),

      adoptUser: (userId) => {
        const current = get();
        if (current.userId !== null && current.userId !== userId) {
          set({ ...EMPTY, userId });
          return;
        }
        if (current.userId === null) set({ userId });
      },

      clear: () => set({ ...EMPTY }),
    }),
    {
      name: "anakmobil.onboarding.draft",
      storage: createJSONStorage(() => onboardingStorage),
      version: DRAFT_VERSION,
      // A shape written by an older version is dropped, not migrated.
      migrate: () => ({ ...EMPTY }),
    },
  ),
);

/**
 * Whether the wizard may leave `step`.
 *
 * The photo step always may — AM-113's technical note authorises skipping it,
 * and there is no upload endpoint to make it anything else.
 */
export function canAdvance(state: DraftState, step: WizardStep): boolean {
  switch (step) {
    case "brand":
      return state.brand !== null;
    case "model":
      return state.model !== null;
    case "generation":
      return state.generation !== null;
    case "year":
      return state.year !== null;
    case "variant":
      return state.variant !== null || state.variantSkipped;
    case "photo":
      return true;
  }
}
