import { Stack } from "expo-router";

// The one route file Expo Router requires. A bare Stack, no navigation
// structure — tabs, nested stacks, and real routes are their own later story.
export default function RootLayout() {
  return <Stack screenOptions={{ headerShown: false }} />;
}
