/**
 * The public surface of the mobile session foundation.
 *
 * Plans B, C, and D import from `@/shared` and from nowhere else — never
 * `@/shared/session/store`, never `@/shared/api/client`. The file layout behind
 * this barrel is Plan A's business and may move without touching them.
 *
 * Everything exported here is in Plan A's FROZEN CONTRACT. Adding to it is a
 * decision; changing a signature in it is a structural finding.
 */
export { apiRequest } from "./api/client";
export type { ApiError, ApiErrorKind } from "./api/errors";
export { refreshMe } from "./api/me";
export { queryClient } from "./api/queryClient";
export { useBootstrap } from "./bootstrap";
export { AppGate, AuthGate, OnboardingGate } from "./gates";
export { resumeSignIn, signIn } from "./session/signIn";
export { signOut } from "./session/signOut";
export { useSession } from "./session/store";
export type { Me, SessionStatus } from "./session/store";
export { setActiveVehicleId, useActiveVehicleId } from "./vehicle/activeVehicle";
