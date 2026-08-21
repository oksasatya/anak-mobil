import Ionicons from "@expo/vector-icons/Ionicons";
import { Tabs } from "expo-router/js-tabs";
import { StyleSheet, Text, View, type ColorValue } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmMaterial } from "@/components/material";
import { AddButton } from "@/components/shell/AddButton";
import { DOCK_BAR_HEIGHT, DOCK_INSET, dockBarRight, dockBottom } from "@/components/shell/dock";
import { ADD_ACTIONS, hasAddActions } from "@/features/shell/addActions";
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

/**
 * A tab label that shrinks rather than truncates.
 *
 * The bar gives up 82pt to the add button, which leaves five items about 61pt
 * each — enough for every label except "Komunitas", which came back as
 * "Komuni…". `tabBarLabelStyle` cannot express "shrink to fit"; a label
 * component can. 11px stays the token size for the four that fit, and only the
 * long one steps down, to no less than 85% of it.
 */
function TabLabel({ label, color }: { readonly label: string; readonly color: ColorValue }) {
  const theme = useTheme();
  return (
    <Text
      numberOfLines={1}
      adjustsFontSizeToFit
      minimumFontScale={0.85}
      style={[theme.type.micro, styles.label, { color }]}
    >
      {label}
    </Text>
  );
}

export default function AppLayout() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();

  return (
    <AppGate>
      {/*
        The add button is a SIBLING of the navigator, not a child of a screen.
        Rendered inside TabScreen it sat under the tab bar, because the bar is
        painted over every screen the navigator hosts. Here it is above both.
      */}
      <View style={styles.shell}>
        <Tabs
          // A NAVIGATOR prop, not a screen option (BottomTabNavigationConfig).
          //
          // The navigator pads the bottom safe area *inside* the bar, which is
          // right for a bar welded to the screen edge and wrong for one that
          // floats above it: the iPhone home-indicator inset would come out of
          // the bar height and eat into the labels. The dock already clears the
          // indicator by sitting on top of it (`dockBottom`), so the bar itself
          // owes nothing.
          safeAreaInsets={{ bottom: 0 }}
          screenOptions={{
            headerShown: false,
            // The ground is the app's bottom layer; a scene with a fill hides it.
            sceneStyle: { backgroundColor: "transparent" },
            // §17: active icon and label are the brand accent. `accentText` rather
            // than `accent` because this one value colours the LABEL too, and raw
            // #ED491C is 3.77:1 as text on white.
            tabBarActiveTintColor: theme.color.accentText,
            tabBarInactiveTintColor: theme.color.textSecondary,
            // §17 again: labels are always visible, never icon-only. The size
            // lives on TabLabel, which can shrink to fit; this only keeps the
            // navigator from reserving a different height for its own default.
            tabBarLabelStyle: theme.type.micro,
            // A floating pill, not a bar welded to the bottom edge. It rides
            // above the home indicator rather than under it, and gives up its
            // right-hand slot to the add button only when there is one to give
            // it to (components/shell/dock.ts).
            tabBarStyle: [
              styles.bar,
              {
                // MARGINS, not `left`/`right`. The navigator writes its own
                // `left: 0, right: 0` onto the bar after this style object, so
                // those two are silently discarded — the bar rendered edge to
                // edge and swallowed the add button behind it. A margin sits
                // inside the frame the navigator resolves and survives.
                marginLeft: DOCK_INSET,
                marginRight: dockBarRight(hasAddActions(ADD_ACTIONS)),
                bottom: dockBottom(insets.bottom),
                height: DOCK_BAR_HEIGHT,
                borderRadius: theme.radius.pill,
              },
            ],
            tabBarItemStyle: styles.item,
            tabBarBackground: () => (
              // `{null}` children: AmMaterial requires the prop, and a tab-bar
              // background is a fill with nothing inside it.
              <AmMaterial role="chrome" radius="pill" style={StyleSheet.absoluteFill}>
                {null}
              </AmMaterial>
            ),
          }}
        >
          <Tabs.Screen
            name="home"
            options={{
              title: "Beranda",
              tabBarLabel: ({ color }) => <TabLabel label="Beranda" color={color} />,
              tabBarIcon: ({ color, size }) => (
                <Ionicons name="home-outline" color={color} size={size} />
              ),
            }}
          />
          <Tabs.Screen
            name="garage"
            options={{
              title: "Garasi",
              tabBarLabel: ({ color }) => <TabLabel label="Garasi" color={color} />,
              tabBarIcon: ({ color, size }) => (
                <Ionicons name="car-sport-outline" color={color} size={size} />
              ),
            }}
          />
          <Tabs.Screen
            name="explore"
            options={{
              title: "Jelajah",
              tabBarLabel: ({ color }) => <TabLabel label="Jelajah" color={color} />,
              tabBarIcon: ({ color, size }) => (
                <Ionicons name="compass-outline" color={color} size={size} />
              ),
            }}
          />
          <Tabs.Screen
            name="community"
            options={{
              title: "Komunitas",
              tabBarLabel: ({ color }) => <TabLabel label="Komunitas" color={color} />,
              tabBarIcon: ({ color, size }) => (
                <Ionicons name="people-outline" color={color} size={size} />
              ),
            }}
          />
          <Tabs.Screen
            name="profile"
            options={{
              title: "Profil",
              tabBarLabel: ({ color }) => <TabLabel label="Profil" color={color} />,
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
        <AddButton />
      </View>
    </AppGate>
  );
}

const styles = StyleSheet.create({
  shell: { flex: 1 },
  label: { textAlign: "center" },
  // Absolute so the ground shows through the bar. The edge comes from
  // AmMaterial, so the navigator's own hairline and elevation are removed
  // rather than drawn on top of it. Position and size are applied inline —
  // they depend on the safe-area inset and on whether the add button exists.
  bar: {
    position: "absolute",
    backgroundColor: "transparent",
    borderTopWidth: 0,
    elevation: 0,
    // The pill clips its own corners; without this the navigator draws items
    // past the rounded ends.
    overflow: "hidden",
  },
  // The bar carries no safe-area padding of its own, so the item only needs
  // enough to keep the icon off the pill's top edge. Horizontal padding is
  // ZERO: the bar gives up 82pt to the add button, which leaves each of five
  // items about 61pt, and the default side padding truncated "Komunitas" to
  // "Komuni…".
  item: { paddingVertical: 4, paddingHorizontal: 0 },
});
