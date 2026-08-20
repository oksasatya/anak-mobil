import { StyleSheet, Text, View } from "react-native";

import { useTheme } from "@/theme";

// ponytail: a placeholder so the gate has somewhere to redirect to and can be
// demonstrated. Plan B replaces this group with welcome / login / register.
export default function AuthPlaceholder() {
  const theme = useTheme();
  return (
    <View style={[styles.container, { padding: theme.space[6], gap: theme.space[3] }]}>
      <Text style={[theme.type.h3, { color: theme.color.textPrimary }]}>Belum masuk</Text>
      <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
        Layar masuk dan daftar menyusul.
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, justifyContent: "center" },
});
