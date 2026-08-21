/**
 * What went wrong, from the point of view of the person looking at the screen.
 *
 * Four kinds because there are four genuinely different things to say — and
 * because a taxonomy with one "error" kind produces a product that says "Ada
 * gangguan di server" when somebody's password was simply wrong.
 *
 * | kind         | cause                        | what the person sees                |
 * |--------------|------------------------------|-------------------------------------|
 * | offline      | no connectivity              | "Tidak ada koneksi" + retry         |
 * | validation   | 422, or a 409 naming a field | messages under the fields that failed |
 * | rateLimited  | 429                          | the countdown from retryAfterSeconds |
 * | unauthorized | 401 after a failed refresh   | back to the welcome screen          |
 * | server       | 5xx, or a malformed response | "Ada gangguan di server" + retry     |
 */
export type ApiErrorKind = "offline" | "validation" | "rateLimited" | "unauthorized" | "server";

export interface ApiError {
  kind: ApiErrorKind;
  /** Already in Bahasa Indonesia and ready to show. Never a raw error string. */
  message: string;
  /** Field name to message, for `kind: "validation"`. */
  fields?: Record<string, string>;
  /** Seconds, for `kind: "rateLimited"`. */
  retryAfterSeconds?: number;
  /**
   * The server's request id, for a log line or a support thread.
   * NEVER rendered to a person.
   */
  requestId?: string;
}

/** The envelope every endpoint answers in. Transport, not payload. */
export interface Envelope {
  meta?: { request_id?: string };
  data?: unknown;
  error?: { code: string; message: string; details?: unknown };
}

const OFFLINE = "Tidak ada koneksi. Periksa jaringan, lalu coba lagi.";
const SERVER = "Ada gangguan di server. Coba lagi sebentar lagi.";
const RATE_LIMITED = "Terlalu banyak percobaan. Tunggu sebentar.";
const UNAUTHORIZED = "Sesi kamu sudah berakhir. Masuk lagi, ya.";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Recognise an envelope, or answer null.
 *
 * Null is an ordinary outcome, not an exception: a proxy in front of the API
 * answers a 502 with HTML, and a body that is not our envelope is exactly the
 * "malformed response" the taxonomy maps to `server`.
 */
export function narrowEnvelope(body: unknown): Envelope | null {
  if (!isRecord(body)) return null;

  const envelope: Envelope = {};

  const meta: unknown = body.meta;
  if (isRecord(meta) && typeof meta.request_id === "string") {
    envelope.meta = { request_id: meta.request_id };
  }
  if ("data" in body) envelope.data = body.data;

  const error: unknown = body.error;
  if (isRecord(error) && typeof error.code === "string" && typeof error.message === "string") {
    envelope.error = { code: error.code, message: error.message, details: error.details };
  }

  return envelope;
}

/**
 * The string values of a details object, and nothing else.
 *
 * Checked rather than cast: a details object carrying a nested value would
 * otherwise reach a `<Text>` as "[object Object]".
 */
function stringFields(details: unknown): Record<string, string> | undefined {
  if (!isRecord(details)) return undefined;

  const fields: Record<string, string> = {};
  for (const [key, value] of Object.entries(details)) {
    if (typeof value === "string") fields[key] = value;
  }
  return Object.keys(fields).length > 0 ? fields : undefined;
}

function retryAfter(details: unknown): number | undefined {
  if (!isRecord(details)) return undefined;
  const seconds = details.retry_after_seconds;
  return typeof seconds === "number" && seconds > 0 ? seconds : undefined;
}

export function offlineError(): ApiError {
  return { kind: "offline", message: OFFLINE };
}

export function serverError(requestId?: string): ApiError {
  return { kind: "server", message: SERVER, requestId };
}

/**
 * Map a non-2xx response to what a person should be told.
 *
 * The server's own message wins wherever there is one: the API answers in
 * Bahasa Indonesia by default and honours `Accept-Language`, so a second copy
 * of every message on the client is a second thing to keep in step.
 */
export function toApiError(status: number, body: unknown): ApiError {
  const envelope = narrowEnvelope(body);
  const requestId = envelope?.meta?.request_id;
  const error = envelope?.error;
  const message = error?.message ?? SERVER;
  const fields = stringFields(error?.details);

  if (status === 429) {
    return {
      kind: "rateLimited",
      message: error?.message ?? RATE_LIMITED,
      retryAfterSeconds: retryAfter(error?.details),
      requestId,
    };
  }

  if (status === 401) {
    return { kind: "unauthorized", message: error?.message ?? UNAUTHORIZED, requestId };
  }

  if (status === 422) {
    return { kind: "validation", message, fields, requestId };
  }

  // A 409 that names a field is a validation failure from the person's side.
  // "Email ini sudah terdaftar" belongs under the email input, not in a
  // server-fault banner — and without this branch a taken email reaches
  // somebody as "Ada gangguan di server".
  if (status === 409 && fields !== undefined) {
    return { kind: "validation", message, fields, requestId };
  }

  if (status >= 400 && status < 500 && error !== undefined) {
    // A 403, a 404, or a 409 with no field detail. The server's message is
    // still the right thing to show; there is simply no field to attach it to.
    return { kind: "server", message, requestId };
  }

  return serverError(requestId);
}
