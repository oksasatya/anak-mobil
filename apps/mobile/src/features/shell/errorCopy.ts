import type { ApiError } from "@/shared";

/**
 * Plan A's `ApiError` is a plain shape rather than an `Error` subclass, so a
 * consumer that wants its `kind` has to narrow. `@/shared` re-exports the
 * type only, not a guard — so this one stays here rather than duplicating a
 * narrowing rule that does not exist elsewhere yet.
 */
function isApiError(value: unknown): value is ApiError {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    // `.message` is dereferenced below, so the guard checks it. Without this,
    // an object carrying `kind` but no string message rendered the literal
    // word "undefined" to a person. Found in Task 1's review.
    typeof (value as { message?: unknown }).message === "string"
  );
}

/**
 * What a person reads under an error title.
 *
 * The spec's taxonomy (§Error taxonomy) has FIVE kinds, and this maps them
 * things to say, and prefers the server's own message where one exists —
 * the API answers in Bahasa Indonesia by default. "Data kamu aman" is added
 * because §53 says an error state reassures about the data rather than
 * describing the failure.
 */
export function errorBody(error: unknown): string {
  if (!isApiError(error)) return "Ada gangguan. Data kamu aman — coba beberapa saat lagi.";
  if (error.kind === "offline")
    return "Tidak ada koneksi. Data kamu aman — coba lagi setelah online.";
  return `${error.message} Data kamu aman.`;
}
