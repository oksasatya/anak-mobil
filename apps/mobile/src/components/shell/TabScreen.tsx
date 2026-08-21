import { useBottomTabBarHeight } from "expo-router/js-tabs";
import type { ReactNode } from "react";
import { ScrollView, StyleSheet, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { useTheme } from "@/theme";

import { AddButton } from "./AddButton";

export interface TabScreenProps {
  readonly children: ReactNode;
}

/**
 * What every tab's content sits in.
 *
 * The tab bar is absolutely positioned so AmGround shows through it, which
 * takes it out of the layout flow — so every screen owes its own bottom
 * inset, and owing it once here is better than owing it in five screens.
 *
 * `flexGrow: 1` on the content container lets a short screen centre itself
 * with a plain `flex: 1` child, which is what the empty tabs do.
 */
export function TabScreen({ children }: TabScreenProps) {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const tabBarHeight = useBottomTabBarHeight();

  const padding = {
    padding: theme.pagePadding,
    paddingTop: insets.top + theme.space[4],
    paddingBottom: tabBarHeight + theme.space[6],
    gap: theme.space[5],
  };

  return (
    <View style={styles.fill}>
      <ScrollView contentContainerStyle={[styles.grow, padding]}>{children}</ScrollView>
      {/* The add action belongs to the shell, not to a screen — every tab
          gets it, and it renders nothing while no form exists to add to. */}
      <AddButton />
    </View>
  );
}

const styles = StyleSheet.create({
  fill: { flex: 1 },
  grow: { flexGrow: 1 },
});
