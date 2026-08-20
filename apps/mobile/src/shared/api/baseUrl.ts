/**
 * The API's origin, read once and shared by every fetch in this layer.
 *
 * CORRECTED after the fix-batch review (ledger 42, ledger 60). Was
 * `process.env.EXPO_PUBLIC_API_URL ?? ""` duplicated across `client.ts`,
 * `refresh.ts`, and `signOut.ts` — the duplicated-literal rule the Rust gate
 * states and the TS gate omits.
 *
 * ponytail: an unset `EXPO_PUBLIC_API_URL` yields `""`, so every request
 * becomes a relative fetch that rejects and is reported forever as "Tidak
 * ada koneksi" — a misconfigured build is indistinguishable from a dead
 * network. A real fix needs a build-time assertion; revisit if a
 * misconfigured build actually ships.
 */
export const BASE_URL = process.env.EXPO_PUBLIC_API_URL ?? "";
