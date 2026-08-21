import { QueryClientProvider } from "@tanstack/react-query";
import {
  DarkTheme,
  DefaultTheme,
  Stack,
  ThemeProvider as NavigationThemeProvider,
} from "expo-router";
import * as SplashScreen from "expo-splash-screen";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { StyleSheet, Text, View } from "react-native";

import { AmBrandLockup } from "@/components/display";
import { AmGround } from "@/components/material";
import { ToastProvider } from "@/components/state";
import { queryClient, useBootstrap } from "@/shared";
import { CapabilityControlContext, ThemeProvider, useAppFonts, useTheme } from "@/theme";

/**
 * The router's navigation container paints its theme's `background` behind
 * every screen (rgb(242,242,242) by default), and that opaque fill is what
 * covered AmGround — `contentStyle: transparent` clears only the screen
 * layer, not the container. expo-router vendors its own theming (SDK 56+
 * dropped react-navigation), so the override imports from "expo-router".
 */
function TransparentNavigationTheme({ children }: { readonly children: ReactNode }) {
  const theme = useTheme();
  const value = useMemo(() => {
    const base = theme.name === "dark" ? DarkTheme : DefaultTheme;
    return { ...base, colors: { ...base.colors, background: "transparent" } };
  }, [theme.name]);
  return <NavigationThemeProvider value={value}>{children}</NavigationThemeProvider>;
}

// Keep the splash screen up rather than flashing the system font for a frame
// and then reflowing every line when Plus Jakarta Sans arrives.
SplashScreen.preventAutoHideAsync().catch(() => {});

// ThemeProvider sits above the route tree so every screen and every
// primitive resolves the same theme; QueryClientProvider sits immediately
// inside it so every screen can useQuery regardless of which gated group it
// renders under. CapabilityControlContext sits next so the catalogue's
// "force no blur" switch reaches every primitive in the tree, not only the
// catalogue screen itself. AmGround is the bottom layer so the stack renders
// transparently over the graphite gradient. ToastProvider sits inside
// AmGround so the toast renders above the ground but below nothing else.
export default function RootLayout() {
  const fontsReady = useAppFonts();
  const sessionReady = useBootstrap();
  const [forceTint, setForceTint] = useState(false);
  const capability = useMemo(() => ({ forceTint, setForceTint }), [forceTint]);

  // Hide the NATIVE splash as soon as the fonts are ready, and carry the wait
  // in-app from there.
  //
  // It used to hold until the session resolved as well, so a cold start with a
  // stored session sat on a bare mark for the length of a `/me` round trip —
  // and the native splash cannot say anything: it is one static PNG with no
  // wordmark and no way to add one without a new asset. Fonts must land first
  // or the wordmark would draw in the system face and reflow when Plus
  // Jakarta Sans arrives, which is the exact flash `preventAutoHideAsync` exists to stop.
  useEffect(() => {
    if (fontsReady) SplashScreen.hideAsync().catch(() => {});
  }, [fontsReady]);

  if (!fontsReady) return null;

  return (
    <ThemeProvider>
      <QueryClientProvider client={queryClient}>
        <CapabilityControlContext.Provider value={capability}>
          <AmGround>
            <ToastProvider>
              {sessionReady ? (
                <TransparentNavigationTheme>
                  <Stack
                    screenOptions={{
                      headerShown: false,
                      contentStyle: { backgroundColor: "transparent" },
                    }}
                  />
                </TransparentNavigationTheme>
              ) : (
                <LaunchView />
              )}
            </ToastProvider>
          </AmGround>
        </CapabilityControlContext.Provider>
      </QueryClientProvider>
    </ThemeProvider>
  );
}

/**
 * What the app shows while the session resolves.
 *
 * It continues the native splash rather than replacing it: same graphite
 * ground, same mark, same 76pt width, same centre — so the only thing that
 * changes at the handoff is that the wordmark appears. The native splash
 * carries no wordmark and cannot be given one without a new asset, which is
 * why the brand's own name never used to be on screen at launch at all.
 *
 * No spinner. There is nothing here a person can act on, the wait is normally
 * one round trip, and a spinner under a logo reads as "something is wrong"
 * rather than "one moment".
 */
function LaunchView() {
  const theme = useTheme();
  return (
    <View style={{ flex: 1, alignItems: "center", justifyContent: "center" }}>
      <AmBrandLockup variant="launch" animate />
      {/* The brand line, verbatim from docs/design.md §76 and the PRD. It sits
          at the foot of the screen rather than under the mark: the lockup's
          entrance is the moment, and a second line arriving beside it competes
          with the thing it is introducing. */}
      <Text
        style={[
          theme.type.caption,
          styles.tagline,
          { color: theme.color.textTertiary, bottom: theme.space[16] },
        ]}
      >
        Mobil lo. Build lo. Komunitas lo.
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  tagline: { position: "absolute", left: 0, right: 0, textAlign: "center" },
});
