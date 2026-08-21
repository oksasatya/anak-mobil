import { MIN_PASSWORD } from "./schemas";

/**
 * Length, and only length.
 *
 * The backend states the reasoning where it enforces the floor
 * (`check_password_shape`): composition rules — a digit, a symbol, a capital
 * — push people toward `Password1!` and NIST dropped them in 2017. So this
 * meter never asks for a character class, and it counts CHARACTERS rather
 * than UTF-16 code units — spreading the string walks Unicode code points,
 * matching the server's own `length_is_counted_in_characters_not_bytes` test.
 * An emoji password is where this bites: one character is a surrogate PAIR in
 * JS, so `.length` on eight of them reads 16 and would score two bands too
 * high. (An earlier version of this comment said "UTF-8 bytes"; JavaScript
 * has no operator that counts those, and the accented-character tests written
 * against that framing could not fail. Corrected after Task 3's review.)
 *
 * Kept in its own react-native-free file, not inside `PasswordStrength.tsx`:
 * that component imports `react-native`, whose Flow-typed internals cannot be
 * parsed by this repo's test runner (see `test/session.test.ts`'s header
 * comment) — importing it from a test crashes the whole run, not just one
 * assertion. Named `passwordScore`, not a case-variant of `PasswordStrength`
 * (`passwordStrength.ts` would collide with `PasswordStrength.tsx` on a
 * case-insensitive filesystem and silently resolve to the wrong file).
 */
export function strengthOf(password: string): 0 | 1 | 2 | 3 | 4 {
  const length = [...password].length;
  if (length === 0) return 0;
  if (length < MIN_PASSWORD) return 1;
  if (length < 12) return 2;
  if (length < 16) return 3;
  return 4;
}
