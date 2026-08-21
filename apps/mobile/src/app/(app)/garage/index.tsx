import { router } from "expo-router";
import { StyleSheet, Text, View } from "react-native";

import { TabScreen } from "@/components/shell";
import { AmEmptyState } from "@/components/state";
import { useTheme } from "@/theme";

/**
 * AM-16's out-of-scope line is "isi setiap tab", so the garage screen itself
 * belongs to the garage epic. What ships here is the honest version of an
 * empty room: what it will hold, and where the thing you came for is today.
 */
export default function GarageScreen() {
  const theme = useTheme();
  return (
    <TabScreen>
      <Text accessibilityRole="header" style={[theme.type.h1, { color: theme.color.textPrimary }]}>
        Garasi
      </Text>
      <View style={styles.centre}>
        <AmEmptyState
          icon="car-sport-outline"
          title="Garasi lengkap belum dibuka"
          body="Nanti di sini ada semua mobil kamu — foto, modifikasi, dan riwayat servis lengkapnya. Untuk sekarang, mobil aktif kamu ada di Beranda."
          actionLabel="Buka Beranda"
          onAction={() => router.navigate("/home")}
        />
      </View>
    </TabScreen>
  );
}

const styles = StyleSheet.create({
  centre: { flex: 1, justifyContent: "center" },
});
