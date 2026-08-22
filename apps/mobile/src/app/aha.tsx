import { Redirect, useLocalSearchParams, useRouter } from "expo-router";
import { useEffect } from "react";
import { ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmCard } from "@/components/display";
import { AmButton } from "@/components/input";
import { useAhaSeen } from "@/features/onboarding/ahaSeen";
import { VehiclePhotoPlaceholder } from "@/features/vehicle/VehiclePhotoPlaceholder";
import { useSession } from "@/shared";
import { useTheme } from "@/theme";

/**
 * AM-56, AC2 mode.
 *
 * AC1 wants build, known-issue, part, and community counts for the car. No
 * endpoint computes any of them, and this project does not invent numbers —
 * so the screen ships without them rather than with placeholders that would
 * read as real data.
 *
 * The seam for AC1 is this file's composition: when an endpoint exists, a
 * CommunityCounts block is placed above <FirstHere /> and the rest of the
 * screen is untouched. Nothing is scaffolded for it in advance.
 *
 * WHY THIS IS A ROOT ROUTE AND NOT `(onboarding)/aha`, which is where the plan
 * put it: `OnboardingGate` redirects the WHOLE group to `(app)` as soon as
 * `!needsProfile && !needsFirstVehicle`, and the save sequence sets
 * `hasVehicles: true` (via `refreshMe`) before it navigates — so the screen
 * celebrating the first car was redirected away before it could draw. Verified
 * on the simulator: the wizard went straight to the home tab. Moving it here,
 * beside `catalog.tsx`, is the fix that does not touch `gates.tsx`, which its
 * own comment puts off-limits to this plan. The screen is not onboarding
 * anyway — its precondition is that a car now EXISTS.
 */
export default function AhaScreen() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const router = useRouter();
  const { status, user } = useSession();
  const markSeen = useAhaSeen((state) => state.markSeen);
  const seen = useAhaSeen((state) => state.seen);

  const { vehicleId, vehicleName } = useLocalSearchParams<{
    vehicleId: string;
    vehicleName: string;
  }>();

  // AC4's other half: a stale link back here after the person has moved on
  // goes straight home rather than replaying the moment.
  useEffect(() => {
    if (vehicleId && seen.includes(vehicleId)) router.replace("/(app)");
  }, [vehicleId, seen, router]);

  const leave = (href: "/(app)" | "/(app)/garage") => {
    if (vehicleId) markSeen(vehicleId);
    router.replace(href);
  };

  // Outside every group gate, so this route owns the one check it needs. It
  // deliberately does NOT test `hasVehicles`: this screen is reached at the
  // exact moment that flips, and gating on it would be the bug it was moved
  // here to escape.
  if (status === "loading") return null;
  if (status === "signedOut" || user === null) return <Redirect href="/(auth)" />;

  return (
    <ScrollView
      contentContainerStyle={{
        padding: theme.pagePadding,
        paddingTop: insets.top + theme.space[8],
        paddingBottom: insets.bottom + theme.space[10],
        gap: theme.space[6],
      }}
    >
      <View style={{ gap: theme.space[2] }}>
        <Text
          accessibilityRole="header"
          style={[theme.type.display, { color: theme.color.textPrimary }]}
        >
          {vehicleName ?? "Mobil kamu"} sudah masuk garasi
        </Text>
        <Text style={[theme.type["body-lg"], { color: theme.color.textSecondary }]}>
          Ini titik awal garasi digital kamu.
        </Text>
      </View>

      <VehiclePhotoPlaceholder />

      <FirstHere onAction={() => leave("/(app)/garage")} />

      <AiInvitation />

      <AmButton label="Lanjut" variant="accent" size="lg" onPress={() => leave("/(app)")} />
    </ScrollView>
  );
}

/**
 * AC2. Deliberately not AmEmptyState: this is not an empty list, it is the
 * opening move of the platform, and it reads as an invitation rather than an
 * absence.
 */
function FirstHere({ onAction }: { readonly onAction: () => void }) {
  const theme = useTheme();
  return (
    <AmCard role="working">
      <View style={{ gap: theme.space[3] }}>
        <Text style={[theme.type.h3, { color: theme.color.textPrimary }]}>
          Jadilah yang pertama
        </Text>
        <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
          Belum ada yang berbagi soal mobil ini. Apa pun yang kamu catat — modifikasi, servis,
          masalah yang kamu temui — jadi rujukan pertama buat pemilik berikutnya.
        </Text>
        <AmButton label="Lihat garasi saya" onPress={onAction} />
      </View>
    </AmCard>
  );
}

/**
 * AC3: the invitation is present in every state. The AI menu itself is E8 and
 * is out of scope by the ticket's own line, so this says what will be possible
 * rather than linking into a screen that does not exist. It is not a disabled
 * button — a control that cannot be pressed is worse than a sentence that is
 * honest.
 */
function AiInvitation() {
  const theme = useTheme();
  return (
    <AmCard role="surface">
      <View style={{ gap: theme.space[2] }}>
        <Text style={[theme.type.label, { color: theme.color.textSecondary }]}>AnakMobil AI</Text>
        <Text style={[theme.type.body, { color: theme.color.textPrimary }]}>
          Nanti kamu bisa tanya apa saja soal mobil ini — servis, part yang cocok, keluhan yang
          sering muncul — dan jawabannya bersandar pada catatan pemilik lain.
        </Text>
      </View>
    </AmCard>
  );
}
