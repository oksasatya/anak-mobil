import { useEffect, useState } from "react";

import { usernameAvailability } from "./api";
import {
  deriveAvailability,
  outcomeOfResult,
  OUTCOME_ON_FAILURE,
  type Availability,
  type Resolution,
} from "./availability";

// Re-exported so `register.tsx` can import both the hook and its type from
// this one module, per the plan's file structure. The derivation itself
// lives in `./availability` — a file with zero imports — so it stays
// unit-testable; importing this module in a test would pull in `./api` and,
// through it, `@/shared`'s `react-native-mmkv` and `expo-router` dependents,
// which crash outside a device (see `./availability`'s header comment).
export type { Availability } from "./availability";
export { disablesSubmit } from "./availability";

/**
 * AM-50's technical note asks for "a few hundred milliseconds". 400ms is long
 * enough that a normal typist does not fire a request per character and short
 * enough that the answer feels like it belongs to what was just typed.
 */
const DEBOUNCE_MS = 400;

/**
 * Debounced availability, with cancellation as a property of the cleanup
 * rather than as a flag somebody has to check.
 *
 * One AbortController per attempt: the effect's cleanup aborts it and clears
 * the timer, so a keystroke that lands mid-flight kills the request outright
 * instead of leaving a late answer to overwrite a newer one. The returned
 * value is DERIVED (`deriveAvailability`), not reduced — `setResolution` is
 * called only inside the async `.then`/`.catch`, never synchronously in the
 * effect body, which is what keeps this clear of
 * `react-hooks/set-state-in-effect`.
 *
 * React Native's `AbortSignal` polyfill has no static `AbortSignal.timeout()`
 * — it is not used here; the controller is built by hand. NOTE: it is
 * not ARMED — the only abort is the cleanup below, fired by the next
 * keystroke or unmount. A black-hole network therefore leaves the hint
 * reading "Memeriksa ketersediaan…" until then. Nothing is blocked by that
 * (`disablesSubmit("checking")` is false), which is why no 5s bound was
 * added; the earlier wording claimed one existed. Corrected after Task 3's
 * review.
 *
 * TanStack Query would also cancel here, but only as a consequence of the
 * last observer detaching — a library behaviour to trust rather than a line
 * to read. Cancellation is a stated requirement of this ticket, so it is
 * written down. There is also nothing worth caching: an availability answer
 * goes stale the moment somebody else registers.
 */
export function useUsernameAvailability(username: string, enabled: boolean): Availability {
  const [resolution, setResolution] = useState<Resolution | null>(null);

  useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    const timer = setTimeout(() => {
      usernameAvailability(username, controller.signal)
        .then((result) => {
          setResolution({ username, outcome: outcomeOfResult(result) });
        })
        .catch(() => {
          // An abort is this effect being replaced by a newer keystroke, not
          // a failure to report — nothing is written, and `deriveAvailability`
          // already falls back to `checking` for a resolution that does not
          // (or no longer) matches the current username.
          if (controller.signal.aborted) return;
          setResolution({ username, outcome: OUTCOME_ON_FAILURE });
        });
    }, DEBOUNCE_MS);

    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [username, enabled]);

  return deriveAvailability(enabled, username, resolution);
}
