import type { ApiError } from "@/shared";

export type RegisterConflict = "email" | "username" | null;

/**
 * Which field a failed registration collided on, if any.
 *
 * CG-2 is closed (see the plan's "The three gaps are CLOSED" table): a taken
 * email or username never reaches the client as a bare 409 — Plan A's
 * `toApiError` already routes a 409 that names a field to `kind: "validation"`
 * with `fields` populated, the same path every other field error takes. There
 * is therefore no `code === "conflict"` fallback to read; one function still
 * exists so no screen encodes the choice itself.
 */
export function registerConflictOf(error: ApiError): RegisterConflict {
  // Presence, not truthiness. `stringFields` in the shared error taxonomy
  // admits an empty string, so `details: {username: ""}` would be falsy here
  // and the screen would show no conflict at all for a taken username. Not
  // reachable against today's backend, whose messages are non-empty consts,
  // but `fieldErrorsOf` already keys on presence and these two should agree.
  // Corrected after Task 1's review.
  if (error.fields?.username !== undefined) return "username";
  if (error.fields?.email !== undefined) return "email";
  return null;
}
