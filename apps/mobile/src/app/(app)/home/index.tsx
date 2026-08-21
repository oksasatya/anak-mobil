import { useState } from "react";
import { StyleSheet, Text, View } from "react-native";

import { AmCard } from "@/components/display";
import { AmButton } from "@/components/input";
import { TabScreen } from "@/components/shell";
import { AmEmptyState, AmErrorState, AmSkeleton } from "@/components/state";
import { useVehicles } from "@/features/garage/queries";
import { useActiveVehicle } from "@/features/garage/useActiveVehicle";
import { VehicleCard } from "@/features/garage/VehicleCard";
import { VehicleSwitcher } from "@/features/garage/VehicleSwitcher";
import { errorBody } from "@/features/shell/errorCopy";
import { refreshMe, setActiveVehicleId, useSession } from "@/shared";
import { kicker, useTheme } from "@/theme";

/**
 * §19: Home is not an infinite feed. Header, the selected vehicle, and what
 * its history says — and nothing this release cannot answer honestly.
 */
export default function HomeScreen() {
  const theme = useTheme();
  const { user } = useSession();
  const vehicles = useVehicles();
  const active = useActiveVehicle(vehicles.data);
  const [switching, setSwitching] = useState(false);

  const name = user?.displayName ?? user?.username;
  const list = vehicles.data ?? [];

  // The shell is only reachable when GET /me says the account has a car, so an
  // empty list means the last one went away somewhere else. Re-running the
  // bootstrap gate is what puts the person back into the first-car wizard;
  // this screen must not grow its own copy of that route.
  // `refreshMe()`, and NOTHING else. `invalidateQueries()` cannot refresh
  // `hasVehicles`: `/me` is not a react-query query at all — `useSession`
  // reads a zustand store written only by `setSignedIn`/`setUser`, and the
  // only `useQuery` in the app is `useVehicles`. So the old version left
  // `hasVehicles` true and `router.replace("/")` bounced straight back here,
  // and the one action in the empty state looped. Plan A named this exact
  // anti-pattern in `shared/api/me.ts` — "invalidating every query and
  // bouncing through `/` is not [the recovery]". `refreshMe` writes the store,
  // `AppGate` re-renders and redirects to `(onboarding)` itself, which also
  // keeps "exactly one redirect" true. Found in Task 2's review.
  const restartOnboarding = () => {
    void refreshMe();
  };

  return (
    <TabScreen>
      <Text accessibilityRole="header" style={[theme.type.h1, { color: theme.color.textPrimary }]}>
        {name ? `Halo, ${name}` : "Beranda"}
      </Text>

      {vehicles.isPending ? (
        <AmCard role="working" padding={5} radius="xl">
          <View style={{ gap: theme.space[3] }}>
            <AmSkeleton height={11} width={88} />
            <AmSkeleton height={24} width="70%" />
            <AmSkeleton height={12} width="38%" />
            <AmSkeleton height={34} width="55%" />
            <View style={{ flexDirection: "row", gap: theme.space[3] }}>
              <AmSkeleton height={32} width="30%" />
              <AmSkeleton height={32} width="30%" />
              <AmSkeleton height={32} width="30%" />
            </View>
          </View>
        </AmCard>
      ) : null}

      {vehicles.isError ? (
        <View style={{ flex: 1, justifyContent: "center" }}>
          <AmErrorState
            icon="cloud-offline-outline"
            title="Garasi gagal dimuat"
            body={errorBody(vehicles.error)}
            onRetry={() => void vehicles.refetch()}
          />
        </View>
      ) : null}

      {vehicles.isSuccess && !active ? (
        <View style={{ flex: 1, justifyContent: "center" }}>
          <AmEmptyState
            icon="car-outline"
            title="Belum ada mobil di garasi"
            body="Semua isi aplikasi ini berangkat dari mobil kamu. Tambahkan satu dulu, sisanya menyusul."
            actionLabel="Tambah mobil"
            onAction={restartOnboarding}
          />
        </View>
      ) : null}

      {active ? (
        <>
          {/*
            The kicker and the switcher are rendered HERE, by the screen, and
            not inside VehicleCard — a control nested inside the card renders
            but never fires on this build (see the note on VehicleCard). §61
            still applies: one car has nothing to switch to, so with one car
            the control is absent rather than present-and-inert.
          */}
          <View style={[styles.header, { marginBottom: -theme.space[3] }]}>
            <Text style={[theme.type.micro, kicker, { color: theme.color.textTertiary }]}>
              Mobil aktif
            </Text>
            {list.length > 1 ? (
              <AmButton
                label="Ganti"
                variant="ghost"
                size="sm"
                onPress={() => setSwitching(true)}
              />
            ) : null}
          </View>
          <VehicleCard vehicle={active} />
        </>
      ) : null}

      {list.length > 1 ? (
        <VehicleSwitcher
          visible={switching}
          onClose={() => setSwitching(false)}
          vehicles={list}
          activeId={active?.id ?? null}
          onSelect={setActiveVehicleId}
        />
      ) : null}
    </TabScreen>
  );
}

const styles = StyleSheet.create({
  // The row carries a 44pt button, so the gap the ScrollView already puts
  // between children would leave the kicker floating away from its card. The
  // negative margin pulls the pair back together.
  header: { flexDirection: "row", alignItems: "center", justifyContent: "space-between" },
});
