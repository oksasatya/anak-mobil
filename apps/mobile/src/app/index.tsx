import { Redirect } from "expo-router";

import { useSession } from "@/shared";

/**
 * The one entry point, and the only place that chooses a group.
 *
 * Each group's own gate then re-checks, so a deep link straight into a
 * protected route is held whether or not it came through here.
 */
export default function Index() {
  const { status, user } = useSession();

  // The splash screen is still up — the root layout holds it until the session
  // resolves — so there is nothing to render and nothing to flash.
  if (status === "loading") return null;
  if (status === "signedOut" || user === null) return <Redirect href="/(auth)" />;
  if (user.username === null || user.displayName === null || !user.hasVehicles) {
    return <Redirect href="/(onboarding)" />;
  }
  return <Redirect href="/(app)" />;
}
