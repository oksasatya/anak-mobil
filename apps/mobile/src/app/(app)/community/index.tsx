import { router } from "expo-router";
import { StyleSheet, Text, View } from "react-native";

import { TabScreen } from "@/components/shell";
import { AmEmptyState } from "@/components/state";
import { useTheme } from "@/theme";

/**
 * The community epic has no implementation, so this says what will be here
 * and points at the one thing a person can do today. No member count, no
 * sample club, no "1.2rb anggota" — a fabricated community is the exact
 * thing the project's own rule forbids seeding.
 */
export default function CommunityScreen() {
  const theme = useTheme();
  return (
    <TabScreen>
      <Text accessibilityRole="header" style={[theme.type.h1, { color: theme.color.textPrimary }]}>
        Komunitas
      </Text>
      <View style={styles.centre}>
        <AmEmptyState
          icon="people-outline"
          title="Komunitas belum dimulai"
          body="Nanti di sini ada klub, diskusi, dan tanya-jawab sesama pemilik mobil. Yang pertama mengisi garasinya jadi yang pertama punya sesuatu untuk dibagikan."
          actionLabel="Lihat mobil kamu"
          onAction={() => router.navigate("/home")}
        />
      </View>
    </TabScreen>
  );
}

const styles = StyleSheet.create({
  centre: { flex: 1, justifyContent: "center" },
});
