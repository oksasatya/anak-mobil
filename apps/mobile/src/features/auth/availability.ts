export type Availability = "idle" | "checking" | "available" | "taken" | "unknown";

export type ResolvedOutcome = "available" | "taken" | "unknown";

/** A resolution tagged with the username it answers for. */
export interface Resolution {
  readonly username: string;
  readonly outcome: ResolvedOutcome;
}

/**
 * What the hook renders, derived purely from its inputs — kept in its own
 * file with zero imports so it is testable without a renderer (this repo's
 * test runner has none — ENVIRONMENT item 8) and without pulling in `./api`,
 * which value-imports `@/shared` and, through it, `react-native-mmkv` and
 * `expo-router`'s `Redirect` — both crash outside a device.
 *
 * Deriving rather than reducing is also what keeps the hook's effect free of
 * a direct `setState` call in its synchronous body (`react-hooks/set-state
 * -in-effect` — `useEffect` only calls `setState` inside its async
 * `.then`/`.catch`, the sanctioned pattern). `resolution.username !==
 * username` is what a keystroke-cancelled (aborted) or otherwise stale
 * answer looks like once derived: it is simply not for the name on screen
 * right now, so it falls through to `checking` rather than needing an
 * explicit "ignore this" event.
 *
 * `outcome === "unknown"` covers a 429, a 5xx, and offline alike — the
 * hook's single `.catch()` does not distinguish them, and none of them may
 * ever produce `taken`. Only a genuine `{ available: false }` answer does
 * that. Indonesian carrier CGNAT puts many real users behind one IP, so a
 * shared-address 429 on this HINT endpoint is ordinary, and telling a
 * person a free name is taken because the hint failed would be worse than
 * the hint being silent.
 */
export function deriveAvailability(
  enabled: boolean,
  username: string,
  resolution: Resolution | null,
): Availability {
  if (!enabled) return "idle";
  if (resolution !== null && resolution.username === username) return resolution.outcome;
  return "checking";
}

/** Only a confirmed collision blocks the button — AM-50 AC2. */
export function disablesSubmit(availability: Availability): boolean {
  return availability === "taken";
}

/**
 * How a successful answer becomes an outcome.
 *
 * Lifted out of the hook after Task 3's review. The hook is untestable here
 * (it value-imports `./api` -> `@/shared` -> `react-native-mmkv`), so while
 * these two lines lived inside it, changing `OUTCOME_ON_FAILURE` to `"taken"`
 * left all 22 of the task's tests green — the exact security-adjacent
 * regression the suite is named for, and the one thing it could not catch.
 */
export function outcomeOfResult(result: { readonly available: boolean }): ResolvedOutcome {
  return result.available ? "available" : "taken";
}

/**
 * What EVERY failure resolves to: a 429, a 5xx, a DNS failure, offline.
 *
 * Never `"taken"`. Indonesian carrier CGNAT puts many real users behind one
 * address, so a shared-address 429 on this HINT endpoint is ordinary — and
 * telling somebody a free name is taken because the hint failed is worse than
 * the hint saying nothing. The server rejects a real collision at submit time
 * regardless.
 */
export const OUTCOME_ON_FAILURE: ResolvedOutcome = "unknown";
