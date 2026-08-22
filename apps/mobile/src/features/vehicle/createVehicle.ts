import { useMutation } from "@tanstack/react-query";

import { apiRequest, type ApiError } from "@/shared";

export interface CreateVehicleInput {
  readonly variantId: string | null;
  readonly describedAs: string;
  readonly year: number | null;
}

/**
 * `POST /vehicles` refuses a car with neither a variant_id nor a non-empty
 * described_as (adapter/http/vehicles.rs:176), so a skipped variant must
 * arrive with a description or the save is a 422.
 *
 * Brand and model only. The seed's generation names already repeat the model
 * ("Avanza Gen 3"), so folding them in produces "Toyota Avanza Avanza Gen 3".
 */
export function describedAsFrom(brand: string, model: string, year: number | null): string {
  return [brand, model, year === null ? null : String(year)]
    .filter((part): part is string => part !== null && part.trim() !== "")
    .join(" ");
}

/**
 * cost_visibility is deliberately absent: the server defaults it to private
 * (`default_cost_visibility`), and an absent field can never widen who sees
 * what a car cost. `private` is absent too — onboarding collects no plate,
 * VIN, or price.
 */
export function useCreateVehicle() {
  return useMutation<{ id: string }, ApiError, CreateVehicleInput>({
    mutationFn: (input) =>
      apiRequest<{ id: string }>("/vehicles", {
        method: "POST",
        body: {
          variant_id: input.variantId,
          described_as: input.describedAs,
          year: input.year,
        },
      }),
  });
}
