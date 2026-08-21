import type { ReactNode } from "react";
import type { ViewStyle } from "react-native";

import { AmMaterial } from "@/components/material";
import { useTheme, type Theme } from "@/theme";

export interface AmCardProps {
  readonly children: ReactNode;
  /**
   * `surface` is the default card (docs/design.md §46, revised). `working`
   * is for anything read to make a decision — service history, fitment
   * results, AI evidence, AI warnings — and is solid.
   */
  readonly role?: "surface" | "working";
  readonly padding?: keyof Theme["space"];
  readonly radius?: keyof Theme["radius"];
  readonly style?: ViewStyle;
}

export function AmCard({
  children,
  role = "surface",
  padding = 4,
  radius = "lg",
  style,
}: AmCardProps) {
  const theme = useTheme();
  return (
    <AmMaterial role={role} radius={radius} style={[{ padding: theme.space[padding] }, style]}>
      {children}
    </AmMaterial>
  );
}
