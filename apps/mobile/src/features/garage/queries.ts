import { useQuery } from "@tanstack/react-query";

import { apiRequest, type ApiError } from "@/shared";

import type { Vehicle } from "./types";

/** Exported so a later mutation can invalidate exactly this, not everything. */
export const vehiclesQueryKey = ["vehicles"] as const;

/**
 * The account's cars, with each car's service rollup already attached.
 *
 * One request, not two. `GET /vehicles` returns every car's
 * `service_count`, `total_cost`, `last_service_date`, `overdue_count`, and
 * `due_soon_count` (vehicles.rs::list -> service_summary::for_list, two
 * queries for the whole garage rather than two per car). `GET
 * /vehicles/{id}/summary` is a different, richer answer — `cost_last_year`,
 * `odometer_km`, `by_category`, and the reminder list — and nothing on the
 * shell renders any of those. It is the endpoint the Home screen calls the
 * day it grows an "Upcoming Maintenance" block, and not before.
 */
export function useVehicles() {
  // `ApiError`, not the inferred `Error`: `ApiError` is an interface and
  // `client.ts` throws an object literal, so an inferred `TError = Error`
  // types `query.error` as something the runtime value is not — no `stack`,
  // fails `instanceof Error`. Found in Task 1's review.
  return useQuery<Vehicle[], ApiError>({
    queryKey: vehiclesQueryKey,
    queryFn: ({ signal }) => apiRequest<Vehicle[]>("/vehicles", { signal }),
  });
}
