import { createContext, useContext, useMemo, useState, type ReactNode } from "react";
import { useColorScheme } from "react-native";

import { themes } from "./theme";
import type { Theme, ThemeName } from "./types";

/** `undefined` means "follow the device", which is the default. */
type SchemeOverride = ThemeName | undefined;

interface ThemeControl {
  readonly scheme: SchemeOverride;
  readonly setScheme: (scheme: SchemeOverride) => void;
  readonly resolved: ThemeName;
}

const ThemeContext = createContext<Theme | null>(null);
const ThemeControlContext = createContext<ThemeControl | null>(null);

export interface ThemeProviderProps {
  readonly children: ReactNode;
  /** Force a theme regardless of the device. Used by the component catalogue. */
  readonly initialScheme?: ThemeName;
}

export function ThemeProvider({ children, initialScheme }: ThemeProviderProps) {
  const system = useColorScheme();
  const [scheme, setScheme] = useState<SchemeOverride>(initialScheme);
  const resolved: ThemeName = scheme ?? (system === "dark" ? "dark" : "light");

  const theme = themes[resolved];
  const control = useMemo<ThemeControl>(
    () => ({ scheme, setScheme, resolved }),
    [scheme, resolved],
  );

  return (
    <ThemeControlContext.Provider value={control}>
      <ThemeContext.Provider value={theme}>{children}</ThemeContext.Provider>
    </ThemeControlContext.Provider>
  );
}

export function useTheme(): Theme {
  const theme = useContext(ThemeContext);
  if (!theme) throw new Error("useTheme must be used inside <ThemeProvider>");
  return theme;
}

export function useThemeControl(): ThemeControl {
  const control = useContext(ThemeControlContext);
  if (!control) throw new Error("useThemeControl must be used inside <ThemeProvider>");
  return control;
}
