import Ionicons from "@expo/vector-icons/Ionicons";
import { Tabs } from "expo-router/js-tabs";
import { StyleSheet } from "react-native";

import { AmMaterial } from "@/components/material";
import { AppGate } from "@/shared";
import { useTheme } from "@/theme";

/**
 * The app shell.
 *
 * `AppGate` is the authorization boundary for this whole subtree — it stops
 * a signed-out person, or one who has not finished onboarding, from reaching
 * anything below it. Plan C replaces only the navigator it renders; the gate
 * itself is untouched.
 *
 * `Tabs` is imported from `expo-router/js-tabs`, not from "expo-router": the
 * same export there is deprecated in SDK 57 (expo-router/build/exports.d.ts).
 * `NativeTabs` from `expo-router/unstable-native-tabs` was considered and
 * rejected — it renders the platform's own tab bar, which cannot carry the
 * `chrome` material and would paint over AmGround, and it is `unstable_`.
 *
 * AC1 is structural, not arranged here: every tab is a directory with its own
 * `_layout.tsx` rendering `<Stack>` (see components/shell/TabStack.tsx), and
 * `popToTopOnBlur` is left at its default of `false`.
 *
 * Tab ORDER is the order of these children —
 * expo-router/build/useScreens.js:63 uses the declared order when there is
 * one. Screens are added to this list by the tasks that create them.
 */
export const unstable_settings = { anchor: "home" };

export default function AppLayout() {
  const theme = useTheme();

  return (
    <AppGate>
      <Tabs
        screenOptions={{
          headerShown: false,
          // The ground is the app's bottom layer; a scene with a fill hides it.
          sceneStyle: { backgroundColor: "transparent" },
          // §17: active icon and label are the brand accent. `accentText` rather
          // than `accent` because this one value colours the LABEL too, and raw
          // #ED491C is 3.77:1 as text on white.
          tabBarActiveTintColor: theme.color.accentText,
          tabBarInactiveTintColor: theme.color.textSecondary,
          // §17 again: labels are always visible, never icon-only.
          tabBarLabelStyle: theme.type.micro,
          tabBarStyle: styles.bar,
          tabBarBackground: () => (
            // `{null}` children: AmMaterial requires the prop, and a tab-bar
            // background is a fill with nothing inside it.
            <AmMaterial role="chrome" radius="xs" style={StyleSheet.absoluteFill}>
              {null}
            </AmMaterial>
          ),
        }}
      >
        <Tabs.Screen
          name="home"
          options={{
            title: "Beranda",
            tabBarIcon: ({ color, size }) => (
              <Ionicons name="home-outline" color={color} size={size} />
            ),
          }}
        />
        <Tabs.Screen
          name="garage"
          options={{
            title: "Garasi",
            tabBarIcon: ({ color, size }) => (
              <Ionicons name="car-sport-outline" color={color} size={size} />
            ),
          }}
        />
        <Tabs.Screen
          name="explore"
          options={{
            title: "Jelajah",
            tabBarIcon: ({ color, size }) => (
              <Ionicons name="compass-outline" color={color} size={size} />
            ),
          }}
        />
        <Tabs.Screen
          name="community"
          options={{
            title: "Komunitas",
            tabBarIcon: ({ color, size }) => (
              <Ionicons name="people-outline" color={color} size={size} />
            ),
          }}
        />
        <Tabs.Screen
          name="profile"
          options={{
            title: "Profil",
            tabBarIcon: ({ color, size }) => (
              <Ionicons name="person-outline" color={color} size={size} />
            ),
          }}
        />
        {/*
          The AM-14 healthcheck screen. A Tabs navigator auto-registers every
          route file as a bar item, so leaving this undeclared would surface
          it as an unlabelled sixth tab the moment this file starts rendering
          <Tabs>. Its TEMPORARY SignOutConfirm mount moved to the Profile tab
          above in Task 4; this route stays reachable for AM-14's healthcheck,
          just hidden from the bar.
        */}
        {/* Not in the bar, but still routable. `index` redirects to Beranda;
            `healthcheck` is AM-14's dev screen, kept reachable and deleted
            when it stops earning its place. */}
        <Tabs.Screen name="index" options={{ href: null }} />
        <Tabs.Screen name="healthcheck" options={{ href: null }} />
      </Tabs>
    </AppGate>
  );
}

const styles = StyleSheet.create({
  // Absolute so the ground shows through the bar. The edge comes from
  // AmMaterial, so the navigator's own hairline and elevation are removed
  // rather than drawn on top of it.
  bar: {
    position: "absolute",
    backgroundColor: "transparent",
    borderTopWidth: 0,
    elevation: 0,
  },
});
