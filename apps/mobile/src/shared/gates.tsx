import { Redirect } from "expo-router";
import type { ReactNode } from "react";

import { useSession, type Me } from "@/shared/session/store";

/**
 * Onboarding completion is DERIVED, never stored.
 *
 * A person who has a car has finished onboarding. A stored completion flag
 * would be a second source of truth free to disagree with the first, and the
 * disagreement would surface as somebody stuck outside their own garage.
 *
 * `username` is set at registration, so a null one only happens for an account
 * created before the username migration — none exist, and treating it as
 * "profile incomplete" is the honest answer if one ever does.
 */
function needsProfile(user: Me): boolean {
  return user.username === null || user.displayName === null;
}

function needsFirstVehicle(user: Me): boolean {
  return !user.hasVehicles;
}

/**
 * The signed-out subtree.
 *
 * Redirects OUT the moment a session starts. This is why no screen calls
 * `router.replace()` in its `onSuccess`: with both a login screen and a
 * register screen able to sign in, that would be two redirects for one event,
 * and the spec bans the second one.
 */
export function AuthGate({ children }: { readonly children: ReactNode }): ReactNode {
  const { status, user } = useSession();

  if (status === "loading") return null;
  if (status === "signedIn" && user !== null) {
    return needsProfile(user) || needsFirstVehicle(user) ? (
      <Redirect href="/(onboarding)" />
    ) : (
      <Redirect href="/(app)" />
    );
  }
  return children;
}

/**
 * The onboarding subtree.
 *
 * Plan D replaces the layout body with its wizard stack; this gate is not part
 * of that body and is not its to edit. Which STEP renders is D's decision —
 * this only decides whether the group renders at all.
 */
export function OnboardingGate({ children }: { readonly children: ReactNode }): ReactNode {
  const { status, user } = useSession();

  if (status === "loading") return null;
  if (status === "signedOut" || user === null) return <Redirect href="/(auth)" />;
  if (!needsProfile(user) && !needsFirstVehicle(user)) return <Redirect href="/(app)" />;
  return children;
}

/**
 * The signed-in app.
 *
 * Plan C replaces the layout body with a Tabs navigator, inside this gate. A
 * deep link into a protected route lands here first and is held or redirected
 * before any screen mounts — which is what makes AM-55 AC2's "no skip" real
 * rather than a missing button.
 */
export function AppGate({ children }: { readonly children: ReactNode }): ReactNode {
  const { status, user } = useSession();

  if (status === "loading") return null;
  if (status === "signedOut" || user === null) return <Redirect href="/(auth)" />;
  if (needsProfile(user) || needsFirstVehicle(user)) return <Redirect href="/(onboarding)" />;
  return children;
}
