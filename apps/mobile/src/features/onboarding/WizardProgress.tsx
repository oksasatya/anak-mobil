import { StyleSheet, Text, View } from "react-native";

import { WIZARD_STEPS, type WizardStep } from "@/features/onboarding/draft";
import { numeric, useTheme } from "@/theme";

const LABELS: Record<WizardStep, string> = {
  brand: "Merek",
  model: "Model",
  generation: "Generasi",
  year: "Tahun",
  variant: "Varian",
  photo: "Foto",
};

export interface WizardProgressProps {
  readonly step: WizardStep;
}

/** AM-113 AC1: "kemajuan saya terlihat di setiap langkah". */
export function WizardProgress({ step }: WizardProgressProps) {
  const theme = useTheme();
  const position = WIZARD_STEPS.indexOf(step) + 1;
  const total = WIZARD_STEPS.length;

  return (
    <View style={{ gap: theme.space[2] }}>
      <View style={styles.row}>
        <Text style={[theme.type.label, { color: theme.color.textSecondary }]}>{LABELS[step]}</Text>
        <Text style={[theme.type.caption, numeric, { color: theme.color.textTertiary }]}>
          Langkah {position} dari {total}
        </Text>
      </View>
      {/*
        React Native has no <progress>, so accessibilityRole is the real
        element here rather than a stand-in for one.
      */}
      <View
        accessibilityRole="progressbar"
        accessibilityValue={{ min: 1, max: total, now: position }}
        style={{
          height: theme.space[1],
          borderRadius: theme.radius.pill,
          backgroundColor: theme.color.border,
          overflow: "hidden",
        }}
      >
        <View
          style={{
            width: `${(position / total) * 100}%`,
            height: "100%",
            backgroundColor: theme.color.accent,
          }}
        />
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", justifyContent: "space-between" },
});
