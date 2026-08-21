import Ionicons from "@expo/vector-icons/Ionicons";
import { router } from "expo-router";
import { useState } from "react";
import { Pressable, StyleSheet, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmBottomSheet } from "@/components/display";
import { AmSelect } from "@/components/input";
import { useVehicles } from "@/features/garage/queries";
import { useActiveVehicle } from "@/features/garage/useActiveVehicle";
import {
  ADD_ACTIONS,
  hasAddActions,
  isAddActionReady,
  type AddAction,
} from "@/features/shell/addActions";
import { setActiveVehicleId } from "@/shared";
import { useTheme } from "@/theme";

import { DOCK_BUTTON, DOCK_INSET, dockBottom } from "./dock";

/**
 * The global add action (AM-16 AC2, AC3).
 *
 * Two components rather than one early return inside hooks: `ADD_ACTIONS` is
 * a module constant, so this outer component decides once whether the feature
 * exists at all, and the inner one — which holds every hook, including the
 * vehicles query — is never mounted while the registry is empty. Written as a
 * single component, the empty case would still fire a query on every tab.
 */
export function AddButton() {
  if (!hasAddActions(ADD_ACTIONS)) return null;
  return <AddButtonContent />;
}

interface RowProps {
  readonly action: AddAction;
  readonly onNavigate: (action: AddAction) => void;
}

/**
 * One row of the sheet.
 *
 * A row whose form does not exist yet is drawn dimmed, labelled "Belum
 * tersedia", and is not a `Pressable` at all — not a disabled one, not one
 * that swallows the tap. There is nothing to press, so there is nothing that
 * looks pressable.
 *
 * The row is a plain bordered View rather than an `AmCard`: a `Pressable`
 * nested inside `AmMaterial` renders but never fires on this build (see the
 * note on `VehicleCard`), and these rows have to work the moment their epic
 * lands.
 */
function ActionRow({ action, onNavigate }: RowProps) {
  const theme = useTheme();
  const ready = isAddActionReady(action);

  const body = (
    <View style={[styles.row, { gap: theme.space[4] }]}>
      <View
        style={[
          styles.tile,
          { borderRadius: theme.radius.md, backgroundColor: theme.color.surfaceSubtle },
        ]}
      >
        <Ionicons
          name={action.icon}
          size={19}
          color={ready ? theme.color.textSecondary : theme.color.textTertiary}
        />
      </View>
      <View style={styles.text}>
        <Text
          style={[
            theme.type.label,
            { color: ready ? theme.color.textPrimary : theme.color.textTertiary },
          ]}
        >
          {action.label}
        </Text>
        <Text style={[theme.type.caption, { color: theme.color.textTertiary }]}>
          {ready ? action.description : "Belum tersedia"}
        </Text>
      </View>
      {ready ? (
        <Ionicons name="chevron-forward" size={16} color={theme.color.textTertiary} />
      ) : null}
    </View>
  );

  const shell = {
    minHeight: 64,
    paddingHorizontal: theme.space[3],
    borderRadius: theme.radius.lg,
    borderWidth: 1,
    borderColor: theme.color.border,
    backgroundColor: theme.material.working.solid,
  };

  if (!ready) {
    return (
      <View accessible accessibilityLabel={`${action.label}, belum tersedia`} style={shell}>
        {body}
      </View>
    );
  }

  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={action.label}
      accessibilityHint={action.description}
      onPress={() => onNavigate(action)}
      style={({ pressed }) => [shell, { opacity: pressed ? 0.82 : 1 }]}
    >
      {body}
    </Pressable>
  );
}

function AddButtonContent() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const vehicles = useVehicles();
  const active = useActiveVehicle(vehicles.data);
  const [open, setOpen] = useState(false);

  const options = (vehicles.data ?? []).map((vehicle) => ({
    value: vehicle.id,
    label: vehicle.name,
  }));

  const navigate = (action: AddAction) => {
    setOpen(false);
    if (action.href !== null) router.navigate(action.href);
  };

  return (
    <>
      {/* Beside the tab bar, not above it — the two are one dock, and their
          geometry is the shared constant in ./dock.ts. */}
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Tambah"
        onPress={() => setOpen(true)}
        style={({ pressed }) => [
          styles.float,
          {
            right: DOCK_INSET,
            bottom: dockBottom(insets.bottom),
            width: DOCK_BUTTON,
            height: DOCK_BUTTON,
            borderRadius: DOCK_BUTTON / 2,
            backgroundColor: theme.color.accent,
            opacity: pressed ? 0.9 : 1,
          },
        ]}
      >
        <Ionicons name="add" size={26} color={theme.color.onAccent} />
      </Pressable>

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
            <ActionRow key={action.key} action={action} onNavigate={navigate} />
          ))}
        </View>
      </AmBottomSheet>
    </>
  );
}

const styles = StyleSheet.create({
  float: { position: "absolute", alignItems: "center", justifyContent: "center" },
  row: { flexDirection: "row", alignItems: "center", flex: 1 },
  text: { flex: 1, gap: 1 },
  tile: { width: 40, height: 40, alignItems: "center", justifyContent: "center" },
});
