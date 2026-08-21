import Ionicons from "@expo/vector-icons/Ionicons";
import { router } from "expo-router";
import { Pressable, StyleSheet, Text, View } from "react-native";

import { AmAvatar, AmBrandLogo, AmCard } from "@/components/display";
import { TabScreen } from "@/components/shell";
import { AmSkeleton } from "@/components/state";
import { SignOutConfirm } from "@/features/auth/SignOutConfirm";
import { formatKilometres } from "@/features/garage/format";
import { useVehicles } from "@/features/garage/queries";
import { useActiveVehicle } from "@/features/garage/useActiveVehicle";
import { resolveProfileIdentity } from "@/features/profile/identity";
import { useSession } from "@/shared";
import { kicker, numeric, useTheme } from "@/theme";

/**
 * Who you are, the car the app is currently about, and the way out.
 *
 * Identity comes from the session the bootstrap gate already fetched — one
 * `GET /me` per launch, not one per screen that wants a name.
 *
 * Signing out is `SignOutConfirm` (Plan B, `features/auth/SignOutConfirm.tsx`):
 * it already owns the confirmation sheet, the double-tap guard, and the one
 * call into `signOut()`. This screen mounts it and nothing else — rebuilding
 * that transaction here is the second sign-out control the TEMPORARY comment
 * on the AM-14 healthcheck screen existed to prevent.
 *
 * TWO THINGS THE REDESIGN SPECIFIES AND THIS SCREEN DELIBERATELY OMITS:
 *
 *   * A cover photo. There is no photo upload anywhere in the app yet, so the
 *     slot would be a control whose destination does not exist — an empty
 *     frame inviting a tap that goes nowhere.
 *   * A "Bergabung Agustus 2026" pill. `Me` carries id, email, username,
 *     display_name, and has_vehicles, and nothing else (`shared/api/me.ts`).
 *     A join date the server never sent would be invented data, which is the
 *     one thing this product must not do. It returns when `/me` carries it.
 */
export default function ProfileScreen() {
  const theme = useTheme();
  const { user } = useSession();
  const vehicles = useVehicles();
  const active = useActiveVehicle(vehicles.data);

  // AppGate means a signed-out person never reaches this screen, so a null
  // user is the moment before the session resolves — a skeleton, not an
  // error.
  if (!user) {
    return (
      <TabScreen>
        <View style={{ gap: theme.space[3], alignItems: "center", paddingTop: theme.space[6] }}>
          <AmSkeleton height={88} width={88} radius="pill" />
          <AmSkeleton height={22} width="56%" />
          <AmSkeleton height={13} width="36%" />
        </View>
        <AmSkeleton height={80} radius="lg" />
      </TabScreen>
    );
  }

  const identity = resolveProfileIdentity(user);
  // The email line adds nothing when it is already the headline — the
  // account has neither a display name nor a username.
  const showEmail = identity.name !== user.email;
  const meta = active
    ? [
        active.year?.toString(),
        active.colour,
        active.mileage_km === null ? null : formatKilometres(active.mileage_km),
      ].filter((part): part is string => Boolean(part))
    : [];

  return (
    <TabScreen>
      <Text accessibilityRole="header" style={[theme.type.h1, { color: theme.color.textPrimary }]}>
        Profil
      </Text>

      <View style={{ alignItems: "center", gap: theme.space[3], paddingVertical: theme.space[3] }}>
        <AmAvatar name={identity.name} size={88} />
        <View style={{ alignItems: "center", gap: theme.space[1] }}>
          <Text style={[theme.type.h2, { color: theme.color.textPrimary }]}>{identity.name}</Text>
          {identity.handle ? (
            <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
              {identity.handle}
            </Text>
          ) : null}
          {showEmail ? (
            <Text style={[theme.type.caption, { color: theme.color.textTertiary }]}>
              {user.email}
            </Text>
          ) : null}
        </View>
      </View>

      {active ? (
        <View style={{ gap: theme.space[2] }}>
          <Text style={[theme.type.micro, kicker, { color: theme.color.textTertiary }]}>
            Garasi kamu
          </Text>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel={`Buka ${active.name} di Beranda`}
            onPress={() => router.navigate("/home")}
            style={({ pressed }) => ({ opacity: pressed ? 0.82 : 1 })}
          >
            <AmCard role="working" padding={3}>
              <View style={[styles.row, { gap: theme.space[4] }]}>
                <AmBrandLogo domain={active.brand_logo_domain} name={active.name} size={56} />
                <View style={styles.text}>
                  <Text
                    numberOfLines={1}
                    style={[theme.type.label, { color: theme.color.textPrimary }]}
                  >
                    {active.name}
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
                <Ionicons name="chevron-forward" size={16} color={theme.color.textTertiary} />
              </View>
            </AmCard>
          </Pressable>
        </View>
      ) : null}

      <View style={{ marginTop: "auto", gap: theme.space[3] }}>
        <SignOutConfirm variant="destructive-quiet" />
        {identity.handle ? (
          <Text style={[theme.type.caption, styles.centered, { color: theme.color.textTertiary }]}>
            {`Masuk sebagai ${identity.handle}`}
          </Text>
        ) : null}
      </View>
    </TabScreen>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center" },
  text: { flex: 1, gap: 2 },
  centered: { textAlign: "center" },
});
