import { apiRequest } from "@/shared/api/client";
import { setUser, type Me } from "@/shared/session/store";

/** The wire shape. snake_case, exactly as the API sends it. */
interface MeWire {
  id: string;
  email: string;
  username: string | null;
  display_name: string | null;
  has_vehicles: boolean;
}

/**
 * Read the caller's identity and derived onboarding state.
 *
 * `signal` is optional and forwarded as-is — only `useBootstrap` needs a
 * bound call (ledger 93); every other caller wants the unbounded default.
 */
export async function fetchMe(signal?: AbortSignal): Promise<Me> {
  const wire = await apiRequest<MeWire>("/me", { signal });
  return {
    id: wire.id,
    email: wire.email,
    username: wire.username,
    displayName: wire.display_name,
    hasVehicles: wire.has_vehicles,
  };
}

/**
 * Re-read `/me` and update the session store. `status` is untouched.
 *
 * Two callers, and the ORDER matters for both:
 *
 * - **Plan C**, when the app shell loads an empty vehicle list. `(app)` is only
 *   reachable with `hasVehicles === true`, so an empty list means the last car
 *   went away somewhere else and the cached `me` is stale. This is the precise
 *   recovery; invalidating every query and bouncing through `/` is not.
 *
 * - **Plan D**, immediately after `POST /vehicles`. At that moment the cached
 *   `me.hasVehicles` is still `false`, so navigating before this resolves sends
 *   somebody straight back into the wizard they just finished. And the PREVIOUS
 *   value of `hasVehicles` is what decides aha-screen versus garage — so read
 *   it BEFORE calling this, then refresh, then navigate.
 */
export async function refreshMe(): Promise<void> {
  setUser(await fetchMe());
}
