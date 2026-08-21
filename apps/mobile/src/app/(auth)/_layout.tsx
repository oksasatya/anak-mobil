import { Stack } from "expo-router";

import { AuthGate } from "@/shared";

// Plan B replaces the <Stack> body with welcome / login / register.
// It does not touch AuthGate.
export default function AuthLayout() {
  return (
    <AuthGate>
      <Stack
        screenOptions={{ headerShown: false, contentStyle: { backgroundColor: "transparent" } }}
      />
    </AuthGate>
  );
}
