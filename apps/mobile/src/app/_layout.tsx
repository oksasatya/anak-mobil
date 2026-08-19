import {
  DarkTheme,
  DefaultTheme,
  Stack,
  ThemeProvider as NavigationThemeProvider,
} from "expo-router";
import * as SplashScreen from "expo-splash-screen";
import { useEffect, useMemo, useState, type ReactNode } from "react";

import { AmGround } from "@/components/material";
import { ToastProvider } from "@/components/state";
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
// and then reflowing every line when Inter arrives.
SplashScreen.preventAutoHideAsync().catch(() => {});

// ThemeProvider sits above the route tree so every screen and every
// primitive resolves the same theme; CapabilityControlContext sits just
// inside it so the catalogue's "force no blur" switch reaches every
// primitive in the tree, not only the catalogue screen itself. AmGround is
// the bottom layer so the stack renders transparently over the graphite
// gradient. ToastProvider sits inside AmGround so the toast renders above
// the ground but below nothing else.
export default function RootLayout() {
  const fontsReady = useAppFonts();
  const [forceTint, setForceTint] = useState(false);
  const capability = useMemo(() => ({ forceTint, setForceTint }), [forceTint]);

  useEffect(() => {
    if (fontsReady) SplashScreen.hideAsync().catch(() => {});
  }, [fontsReady]);

  if (!fontsReady) return null;

  return (
    <ThemeProvider>
      <CapabilityControlContext.Provider value={capability}>
        <AmGround>
          <ToastProvider>
            <TransparentNavigationTheme>
              <Stack
                screenOptions={{
                  headerShown: false,
                  contentStyle: { backgroundColor: "transparent" },
                }}
              />
            </TransparentNavigationTheme>
          </ToastProvider>
        </AmGround>
      </CapabilityControlContext.Provider>
    </ThemeProvider>
  );
}
