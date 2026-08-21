import { router } from "expo-router";
import { StyleSheet, Text, View } from "react-native";

import { TabScreen } from "@/components/shell";
import { AmEmptyState } from "@/components/state";
import { useTheme } from "@/theme";

/**
 * Explore is community output — builds, parts, solutions from cars like
 * yours. There is no output yet because there are no garages yet, and the
 * spec's rule is that the platform launches empty and says so rather than
 * showing a wall of invented content.
 */
export default function ExploreScreen() {
  const theme = useTheme();
  return (
    <TabScreen>
      <Text accessibilityRole="header" style={[theme.type.h1, { color: theme.color.textPrimary }]}>
        Jelajah
      </Text>
      <View style={styles.centre}>
        <AmEmptyState
          icon="compass-outline"
          title="Jelajah belum ada isinya"
          body="Nanti di sini kamu bisa menemukan modifikasi, part, dan solusi dari mobil yang sama dengan punyamu. Isinya datang dari garasi anggota — belum ada satu pun yang terisi."
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
