import type { ReactNode } from "react";
import { ScrollView, StyleSheet } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { useTheme } from "@/theme";

import { dockClearance } from "./dock";

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
 * That inset is `dockClearance`, not `useBottomTabBarHeight()`: the bar now
 * floats above the bottom edge, and the navigator measures the bar without
 * the gap beneath it.
 *
 * `flexGrow: 1` on the content container lets a short screen centre itself
 * with a plain `flex: 1` child, which is what the empty tabs do.
 */
export function TabScreen({ children }: TabScreenProps) {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const padding = {
    padding: theme.pagePadding,
    paddingTop: insets.top + theme.space[4],
    paddingBottom: dockClearance(insets.bottom) + theme.space[5],
    gap: theme.space[5],
  };

  // The add button used to be rendered HERE, and it was drawn UNDERNEATH the
  // tab bar: a screen is a child of the navigator, and the navigator paints
  // its bar over every screen. It now lives beside <Tabs> in (app)/_layout.tsx,
  // which is the only place in the tree that is above the bar.
  return <ScrollView contentContainerStyle={[styles.grow, padding]}>{children}</ScrollView>;
}

const styles = StyleSheet.create({
  grow: { flexGrow: 1 },
});
