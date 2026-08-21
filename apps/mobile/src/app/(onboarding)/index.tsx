import { StyleSheet, Text, View } from "react-native";

import { useTheme } from "@/theme";

// ponytail: a placeholder so the gate has somewhere to redirect to and can be
// demonstrated. Plan D replaces this group with the profile step, the six-step
// wizard, and the aha screen.
export default function OnboardingPlaceholder() {
  const theme = useTheme();
  return (
    <View style={[styles.container, { padding: theme.space[6], gap: theme.space[3] }]}>
      <Text style={[theme.type.h3, { color: theme.color.textPrimary }]}>Lengkapi profil</Text>
      <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
        Langkah profil dan mobil pertama menyusul.
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, justifyContent: "center" },
});
