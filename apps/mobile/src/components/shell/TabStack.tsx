import { Stack } from "expo-router";

/**
 * A tab's own navigator.
 *
 * This is the whole of AM-16 AC1's stack half: because each tab is a
 * DIRECTORY whose `_layout.tsx` renders this, each tab owns a stack that
 * stays mounted while another tab is on screen, and `popToTopOnBlur` defaults
 * to `false`
 * (expo-router/build/react-navigation/bottom-tabs/types.d.ts:200). Nothing in
 * this repository may set it to `true`.
 *
 * `contentStyle` is transparent for the same reason the root layout's is:
 * AmGround is the bottom layer and an opaque screen hides it.
 */
export function TabStack() {
  return (
    <Stack
      screenOptions={{ headerShown: false, contentStyle: { backgroundColor: "transparent" } }}
    />
  );
}
