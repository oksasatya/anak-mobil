import { Stack } from "expo-router";

import { OnboardingGate } from "@/shared";

// Plan D replaces the <Stack> body with the profile step, the six-step
// wizard, and the aha screen. It does not touch OnboardingGate.
export default function OnboardingLayout() {
  return (
    <OnboardingGate>
      <Stack
        screenOptions={{ headerShown: false, contentStyle: { backgroundColor: "transparent" } }}
      />
    </OnboardingGate>
  );
}
