import { Stack } from "expo-router";

import { AppGate } from "@/shared";

// Plan C replaces the <Stack> body with the five-tab navigator. It does not
// touch AppGate — the gate is the authorization boundary for this whole
// subtree, and an overwrite that dropped it would be invisible.
export default function AppLayout() {
  return (
    <AppGate>
      <Stack
        screenOptions={{ headerShown: false, contentStyle: { backgroundColor: "transparent" } }}
      />
    </AppGate>
  );
}
