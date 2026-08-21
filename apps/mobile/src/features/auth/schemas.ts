import { z } from "zod";

/**
 * The client-side mirror of the server's username canonicaliser.
 *
 * `a-z0-9._`, 3-30 characters, no leading or trailing dot or underscore, no
 * consecutive dots. The first and last character classes are alphanumeric,
 * which is what enforces the leading/trailing rule; `{1,28}` between them is
 * what makes the total 3-30.
 *
 * This exists for instant feedback and NOTHING ELSE. The server's
 * canonicaliser is the authority and the only thing that decides a name is
 * acceptable — see the spec's "Tidak boleh ada".
 */
export const USERNAME_PATTERN = /^(?!.*\.\.)[a-z0-9][a-z0-9._]{1,28}[a-z0-9]$/;

/** The server's floor, in characters rather than bytes. */
export const MIN_PASSWORD = 8;

export const loginSchema = z.object({
  email: z.email("Format email belum benar."),
  password: z.string().min(1, "Kata sandi belum diisi."),
});

export const registerSchema = z.object({
  email: z.email("Format email belum benar."),
  password: z.string().min(MIN_PASSWORD, `Minimal ${MIN_PASSWORD} karakter.`),
  username: z
    .string()
    // `.trim().toLowerCase()` FIRST, mirroring the server's own order:
    // `username.rs` does `raw.trim().to_ascii_lowercase()` and only THEN
    // validates. Without these two, the client validated the raw string and
    // rejected names the server accepts — `Oksa` and `"  Budi  "` both failed
    // here with "Awali dan akhiri dengan huruf atau angka", a message
    // describing a rule the person did not break. Pasting a name from a
    // password manager is the ordinary way in, so this was not an edge case.
    // Added after Task 1's review; the parsed value is now the canonical one.
    .trim()
    .toLowerCase()
    .min(3, "Minimal 3 karakter.")
    .max(30, "Maksimal 30 karakter.")
    .regex(USERNAME_PATTERN, "Awali dan akhiri dengan huruf atau angka. Tanpa titik ganda."),
  consent: z.literal(true, "Setujui dulu syarat layanan dan kebijakan privasi."),
});

/**
 * The first message per field, keyed by field name.
 *
 * First rather than all: AM-57 puts one message under the field that failed,
 * and a stack of three under one input is noise, not help.
 */
export function fieldErrorsOf(error: z.ZodError): Record<string, string> {
  const out: Record<string, string> = {};
  for (const issue of error.issues) {
    const key = issue.path.at(0);
    if (typeof key === "string" && !(key in out)) out[key] = issue.message;
  }
  return out;
}
