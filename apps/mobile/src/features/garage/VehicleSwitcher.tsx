import Ionicons from "@expo/vector-icons/Ionicons";
import { Pressable, StyleSheet, Text, View } from "react-native";

import { AmBottomSheet, AmBrandLogo } from "@/components/display";
import { numeric, useTheme } from "@/theme";

import { formatKilometres } from "./format";
import type { Vehicle } from "./types";

export interface VehicleSwitcherProps {
  readonly visible: boolean;
  readonly onClose: () => void;
  readonly vehicles: readonly Vehicle[];
  readonly activeId: string | null;
  readonly onSelect: (id: string) => void;
}

/**
 * Choosing which car the app is currently about.
 *
 * `AmSelect` already owns the one-line picker and every other picker in the
 * app goes through it — this is not a second pattern, it is the same
 * `AmBottomSheet` with a two-line row. A car is identified by its year,
 * colour, and mileage as much as by its name ("Avanza" is two cars in a
 * household), and a single-line label cannot carry that.
 *
 * There is no plate and no VIN here, and there is no field for one: that is
 * the server's privacy boundary and this screen mirrors it rather than
 * re-deciding it.
 */
export function VehicleSwitcher({
  visible,
  onClose,
  vehicles,
  activeId,
  onSelect,
}: VehicleSwitcherProps) {
  const theme = useTheme();

  return (
    <AmBottomSheet visible={visible} onClose={onClose} title="Mobil aktif">
      <View accessibilityRole="radiogroup">
        {vehicles.map((vehicle) => {
          const selected = vehicle.id === activeId;
          const meta = [
            vehicle.year?.toString(),
            vehicle.colour,
            vehicle.mileage_km === null ? null : formatKilometres(vehicle.mileage_km),
          ].filter((part): part is string => Boolean(part));

          return (
            <Pressable
              key={vehicle.id}
              accessibilityRole="radio"
              accessibilityState={{ selected }}
              accessibilityLabel={vehicle.name}
              accessibilityHint={meta.length > 0 ? meta.join(", ") : undefined}
              onPress={() => {
                onSelect(vehicle.id);
                onClose();
              }}
              style={({ pressed }) => [
                styles.option,
                {
                  minHeight: 56,
                  paddingHorizontal: theme.space[3],
                  gap: theme.space[3],
                  borderRadius: theme.radius.md,
                  backgroundColor: pressed ? theme.color.surfaceSubtle : "transparent",
                },
              ]}
            >
              <AmBrandLogo domain={vehicle.brand_logo_domain} name={vehicle.name} size={30} />
              <View style={styles.text}>
                <Text
                  numberOfLines={1}
                  style={[theme.type.label, { color: theme.color.textPrimary }]}
                >
                  {vehicle.name}
                </Text>
                {meta.length > 0 ? (
                  <Text
                    numberOfLines={1}
                    style={[theme.type.caption, numeric, { color: theme.color.textSecondary }]}
                  >
                    {meta.join(" · ")}
                  </Text>
                ) : null}
              </View>
              {selected ? (
                <Ionicons name="checkmark" size={18} color={theme.color.accentText} />
              ) : null}
            </Pressable>
          );
        })}
      </View>
    </AmBottomSheet>
  );
}

const styles = StyleSheet.create({
  option: { flexDirection: "row", alignItems: "center" },
  text: { flex: 1, gap: 1 },
});
