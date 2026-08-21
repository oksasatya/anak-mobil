import { useCallback, useEffect, useState } from "react";

/**
 * Fallback wait when a 429 carries no `retryAfterSeconds` at all — the
 * limiter fired but the response omitted the number. Conservative rather
 * than short, since the alternative is a countdown that never blocks
 * anything.
 */
export const DEFAULT_RETRY_SECONDS = 60;

/**
 * A safe number of seconds to count down from.
 *
 * Missing (`undefined`), non-finite, or non-positive input all fall back to
 * `DEFAULT_RETRY_SECONDS` — never "NaN detik" on screen, and never a
 * countdown that starts already expired (which would re-enable the button
 * instantly on a 429 the server explicitly asked us to wait out).
 */
export function normalizeSeconds(seconds: number | undefined): number {
  if (seconds === undefined || !Number.isFinite(seconds) || seconds <= 0) {
    return DEFAULT_RETRY_SECONDS;
  }
  return seconds;
}

/**
 * Seconds left until `deadlineMs`, as of `nowMs`.
 *
 * Two guards beyond the plain subtraction:
 * - Clamped to 0: an expired deadline is 0 remaining, never negative.
 * - Clamped to `totalSeconds`: a device clock that jumps backwards would
 *   otherwise widen `deadlineMs - nowMs` past what was actually requested,
 *   showing a longer wait than the server asked for. Capping at the
 *   original total bounds the damage to "at most what the 429 said", never
 *   more.
 *
 * Rounds UP (`ceil`), never down: at "1.4 seconds left" the honest label is
 * "2" — rounding down would show "0 detik" for a moment while the button is
 * still (correctly) disabled, which reads as a stuck button.
 */
export function secondsRemaining(deadlineMs: number, nowMs: number, totalSeconds: number): number {
  const raw = Math.ceil((deadlineMs - nowMs) / 1000);
  if (raw <= 0) return 0;
  return Math.min(raw, totalSeconds);
}

export interface UseCountdownResult {
  readonly remaining: number;
  readonly start: (seconds: number | undefined) => void;
}

/**
 * Wall-clock countdown for the 429 rate-limit wait.
 *
 * Driven from a `Date.now()` deadline rather than a decrementing counter: a
 * `setInterval` that just subtracts 1 every tick loses time the moment the
 * app is backgrounded (iOS/Android throttle or suspend timers), and the
 * person returns to a countdown that is wrong in their favour or against
 * them. Recomputing from the deadline on every tick self-corrects instead.
 *
 * Deliberately NOT persisted. A restart clears it, the next attempt gets a
 * fresh 429 with a fresh number, and the server stays the authority on the
 * wait. Persisting it would be a second copy of the limiter's state on a
 * device that cannot be trusted with it anyway.
 */
export function useCountdown(): UseCountdownResult {
  const [deadline, setDeadline] = useState<number | null>(null);
  const [total, setTotal] = useState(0);
  const [remaining, setRemaining] = useState(0);

  useEffect(() => {
    if (deadline === null) return;
    const tick = () => {
      const left = secondsRemaining(deadline, Date.now(), total);
      setRemaining(left);
      if (left === 0) setDeadline(null);
    };
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [deadline, total]);

  const start = useCallback((seconds: number | undefined) => {
    const normalized = normalizeSeconds(seconds);
    setTotal(normalized);
    setDeadline(Date.now() + normalized * 1000);
  }, []);

  return { remaining, start };
}

/** `95` -> `"1:35"`. */
export function formatCountdown(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  return `${minutes}:${rest.toString().padStart(2, "0")}`;
}
