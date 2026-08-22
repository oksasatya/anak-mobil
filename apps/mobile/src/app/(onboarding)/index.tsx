import { Redirect } from "expo-router";

import { useSession } from "@/shared";

/**
 * Where `/(onboarding)` lands.
 *
 * `gates.tsx` sends anybody who still owes a display name or a first car to
 * this group without saying which of the two is missing, so the split is made
 * here. The order matches the wizard's own: a name first, because the aha
 * screen and the garage both greet somebody by it.
 *
 * `displayName` alone, not `needsProfile`'s full test — the username half is
 * collected at registration (AM-50), so a signed-in account that reaches this
 * route with a null username has a problem no onboarding step can fix.
 */
export default function OnboardingEntry() {
  const { user } = useSession();
  if (user?.displayName == null) return <Redirect href="/(onboarding)/profile" />;
  return <Redirect href="/(onboarding)/vehicle" />;
}
