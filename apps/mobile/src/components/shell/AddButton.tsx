import { router } from "expo-router";
import { useBottomTabBarHeight } from "expo-router/js-tabs";
import { useState } from "react";
import { StyleSheet, View } from "react-native";

import { AmBottomSheet } from "@/components/display";
import { AmButton, AmSelect } from "@/components/input";
import { useVehicles } from "@/features/garage/queries";
import { useActiveVehicle } from "@/features/garage/useActiveVehicle";
import { ADD_ACTIONS, hasAddActions } from "@/features/shell/addActions";
import { setActiveVehicleId } from "@/shared";
import { useTheme } from "@/theme";

/**
 * The global add action (AM-16 AC2, AC3).
 *
 * Two components rather than one early return inside hooks: `ADD_ACTIONS` is
 * a module constant, so this outer component decides once whether the
 * feature exists at all, and the inner one — which holds every hook,
 * including the vehicles query — is never mounted while the registry is
 * empty. Written as a single component, the empty case would still fire a
 * query on every tab.
 */
export function AddButton() {
  if (!hasAddActions(ADD_ACTIONS)) return null;
  return <AddButtonContent />;
}

function AddButtonContent() {
  const theme = useTheme();
  const tabBarHeight = useBottomTabBarHeight();
  const vehicles = useVehicles();
  const active = useActiveVehicle(vehicles.data);
  const [open, setOpen] = useState(false);

  const options = (vehicles.data ?? []).map((vehicle) => ({
    value: vehicle.id,
    label: vehicle.name,
  }));

  return (
    <>
      <View
        style={[styles.float, { right: theme.pagePadding, bottom: tabBarHeight + theme.space[4] }]}
      >
        <AmButton label="Tambah" variant="accent" onPress={() => setOpen(true)} />
      </View>

      <AmBottomSheet
        visible={open}
        onClose={() => setOpen(false)}
        // AC3: the sheet says which car it is adding to before it says what.
        title={active ? `Tambah ke ${active.name}` : "Tambah"}
      >
        <View style={{ gap: theme.space[3] }}>
          {options.length > 1 ? (
            <AmSelect
              label="Mobil"
              value={active?.id ?? null}
              options={options}
              onChange={setActiveVehicleId}
            />
          ) : null}
          {ADD_ACTIONS.map((action) => (
            <AmButton
              key={action.key}
              label={action.label}
              variant="secondary"
              onPress={() => {
                setOpen(false);
                router.navigate(action.href);
              }}
            />
          ))}
        </View>
      </AmBottomSheet>
    </>
  );
}

const styles = StyleSheet.create({
  float: { position: "absolute" },
});
