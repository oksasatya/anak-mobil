import { contrastRatio } from "@anakmobil/tokens/derive";
import { useState } from "react";
import { ScrollView, StyleSheet, Switch, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmAvatar, AmBadge, AmCard, AmChip } from "@/components/display";
import { AmButton, AmSelect, AmTextField } from "@/components/input";
import { AmMaterial, useMaterialTextColor } from "@/components/material";
import { AmEmptyState, AmErrorState, AmSkeleton, useToast } from "@/components/state";
import {
  numeric,
  useCapabilityControl,
  useMaterialCapability,
  useTheme,
  useThemeControl,
} from "@/theme";

type Transmission = "manual" | "matic" | "cvt" | "dct";

const TRANSMISSIONS: readonly { value: Transmission; label: string }[] = [
  { value: "manual", label: "Manual" },
  { value: "matic", label: "Automatic" },
  { value: "cvt", label: "CVT" },
  { value: "dct", label: "DCT" },
];

function Section({
  title,
  children,
}: {
  readonly title: string;
  readonly children: React.ReactNode;
}) {
  const theme = useTheme();
  return (
    <View style={{ gap: theme.space[3] }}>
      <Text style={[theme.type.h3, { color: theme.color.textPrimary }]}>{title}</Text>
      {children}
    </View>
  );
}

export default function Catalog() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const { resolved, setScheme } = useThemeControl();
  const { forceTint, setForceTint } = useCapabilityControl();
  const capability = useMaterialCapability();
  const toast = useToast();

  const [text, setText] = useState("");
  const [transmission, setTransmission] = useState<Transmission | null>(null);
  const [chip, setChip] = useState<string>("Daily");

  // The Material section's sample labels go through useMaterialTextColor
  // rather than theme.color.textPrimary/Secondary/Tertiary directly — the
  // only place in this screen that exercises the __DEV__ guard which throws
  // when a text role sits on a material that cannot carry it (Task 4 review,
  // ledger H1).
  const chromePrimary = useMaterialTextColor("chrome", "primary");
  const surfacePrimary = useMaterialTextColor("surface", "primary");
  const surfaceSecondary = useMaterialTextColor("surface", "secondary");
  const workingPrimary = useMaterialTextColor("working", "primary");
  const workingSecondary = useMaterialTextColor("working", "secondary");
  const workingTertiary = useMaterialTextColor("working", "tertiary");

  // The contrast contract, computed live against the tokens actually loaded.
  // Task 1's test asserts the same pairs in CI; showing them here is what
  // makes AM-15 AC2 checkable by eye on a real device.
  const pairs: readonly { label: string; fg: string; bg: string }[] = [
    { label: "primary / working", fg: theme.color.textPrimary, bg: theme.material.working.solid },
    {
      label: "secondary / working",
      fg: theme.color.textSecondary,
      bg: theme.material.working.solid,
    },
    { label: "tertiary / working", fg: theme.color.textTertiary, bg: theme.material.working.solid },
    { label: "primary / surface", fg: theme.color.textPrimary, bg: theme.material.surface.solid },
    {
      label: "secondary / surface",
      fg: theme.color.textSecondary,
      bg: theme.material.surface.solid,
    },
    { label: "primary / chrome", fg: theme.color.textPrimary, bg: theme.material.chrome.solid },
    { label: "onAccent / accent", fg: theme.color.onAccent, bg: theme.color.accent },
  ];

  return (
    <ScrollView
      contentContainerStyle={{
        padding: theme.pagePadding,
        paddingTop: insets.top + theme.space[4],
        paddingBottom: insets.bottom + theme.space[10],
        gap: theme.space[8],
      }}
    >
      <Section title="Katalog Komponen">
        <AmCard role="working">
          <View style={{ gap: theme.space[3] }}>
            <View style={styles.row}>
              <Text style={[theme.type.body, { color: theme.color.textPrimary }]}>Tema gelap</Text>
              <Switch
                accessibilityLabel="Tema gelap"
                value={resolved === "dark"}
                onValueChange={(on) => setScheme(on ? "dark" : "light")}
              />
            </View>
            <View style={styles.row}>
              <Text style={[theme.type.body, { color: theme.color.textPrimary }]}>
                Paksa tanpa blur
              </Text>
              <Switch
                accessibilityLabel="Paksa tanpa blur"
                value={forceTint}
                onValueChange={setForceTint}
              />
            </View>
            <Text style={[theme.type.caption, { color: theme.color.textTertiary }]}>
              Material aktif: {capability}
            </Text>
          </View>
        </AmCard>
      </Section>

      <Section title="Material">
        <AmMaterial role="chrome" style={{ padding: theme.space[4] }}>
          <Text style={[theme.type.label, { color: chromePrimary }]}>
            chrome — hanya teks primer
          </Text>
        </AmMaterial>
        <AmMaterial role="surface" style={{ padding: theme.space[4], gap: theme.space[2] }}>
          <Text style={[theme.type.body, { color: surfacePrimary }]}>surface — primer</Text>
          <Text style={[theme.type.body, { color: surfaceSecondary }]}>surface — sekunder</Text>
        </AmMaterial>
        <AmMaterial role="working" style={{ padding: theme.space[4], gap: theme.space[2] }}>
          <Text style={[theme.type.body, { color: workingPrimary }]}>working — primer</Text>
          <Text style={[theme.type.body, { color: workingSecondary }]}>working — sekunder</Text>
          <Text style={[theme.type.body, { color: workingTertiary }]}>working — tersier</Text>
        </AmMaterial>
      </Section>

      <Section title="Kontras">
        <AmCard role="working">
          <View style={{ gap: theme.space[2] }}>
            {pairs.map((pair) => {
              const ratio = contrastRatio(pair.fg, pair.bg);
              return (
                <View key={pair.label} style={styles.row}>
                  <Text style={[theme.type.caption, { color: theme.color.textSecondary }]}>
                    {pair.label}
                  </Text>
                  <Text
                    style={[
                      theme.type.caption,
                      numeric,
                      {
                        color:
                          ratio >= 4.5
                            ? theme.color.semanticText.success
                            : theme.color.semanticText.danger,
                      },
                    ]}
                  >
                    {ratio.toFixed(2)}:1
                  </Text>
                </View>
              );
            })}
          </View>
        </AmCard>
      </Section>

      <Section title="Tipografi">
        <AmCard role="working">
          <View style={{ gap: theme.space[2] }}>
            <Text style={[theme.type.display, { color: theme.color.textPrimary }]}>Display</Text>
            <Text style={[theme.type.h1, { color: theme.color.textPrimary }]}>H1</Text>
            <Text style={[theme.type.h2, { color: theme.color.textPrimary }]}>H2</Text>
            <Text style={[theme.type.h3, { color: theme.color.textPrimary }]}>H3</Text>
            <Text style={[theme.type.title, { color: theme.color.textPrimary }]}>Title</Text>
            <Text style={[theme.type["body-lg"], { color: theme.color.textPrimary }]}>
              Body Large
            </Text>
            <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>Body</Text>
            <Text style={[theme.type.label, { color: theme.color.textSecondary }]}>Label</Text>
            <Text style={[theme.type.caption, { color: theme.color.textTertiary }]}>Caption</Text>
            <Text style={[theme.type.micro, { color: theme.color.textTertiary }]}>Micro</Text>
            <Text style={[theme.type.title, numeric, { color: theme.color.textPrimary }]}>
              18×8.5 ET40
            </Text>
            <Text style={[theme.type.title, numeric, { color: theme.color.textPrimary }]}>
              225/40 R18
            </Text>
            <Text style={[theme.type.title, numeric, { color: theme.color.textPrimary }]}>
              146,120 KM
            </Text>
            <Text style={[theme.type.title, numeric, { color: theme.color.textPrimary }]}>
              Rp 14.500.000
            </Text>
          </View>
        </AmCard>
      </Section>

      <Section title="Tombol">
        <View style={{ gap: theme.space[3] }}>
          <AmButton label="Primary" onPress={() => toast({ message: "Primary ditekan" })} />
          <AmButton
            label="Accent"
            variant="accent"
            onPress={() => toast({ message: "Tersimpan", tone: "success" })}
          />
          <AmButton label="Secondary" variant="secondary" onPress={() => {}} />
          <AmButton label="Ghost" variant="ghost" onPress={() => {}} />
          <AmButton
            label="Destructive"
            variant="destructive"
            onPress={() => toast({ message: "Dihapus", tone: "danger" })}
          />
          <AmButton label="Disabled" disabled onPress={() => {}} />
          <AmButton label="Loading" loading onPress={() => {}} />
          <AmButton label="Small" size="sm" onPress={() => {}} />
          <AmButton label="Large" size="lg" onPress={() => {}} />
        </View>
      </Section>

      <Section title="Masukan">
        <AmTextField
          label="Plat nomor"
          value={text}
          onChangeText={setText}
          placeholder="B 1234 XYZ"
        />
        <AmTextField
          label="Dengan petunjuk"
          value=""
          onChangeText={() => {}}
          hint="Isi sesuai STNK"
        />
        <AmTextField
          label="Dengan kesalahan"
          value="B"
          onChangeText={() => {}}
          error="Plat nomor belum lengkap"
        />
        <AmTextField label="Nonaktif" value="Tidak bisa diubah" onChangeText={() => {}} disabled />
        <AmSelect
          label="Transmisi"
          value={transmission}
          options={TRANSMISSIONS}
          onChange={setTransmission}
        />
      </Section>

      <Section title="Tampilan">
        <AmCard role="surface">
          <Text style={[theme.type.body, { color: theme.color.textPrimary }]}>Kartu surface</Text>
        </AmCard>
        <AmCard role="working">
          <Text style={[theme.type.body, { color: theme.color.textPrimary }]}>Kartu working</Text>
        </AmCard>
        <View style={[styles.wrap, { gap: theme.space[2] }]}>
          {["Daily", "Track", "Stance", "Touring", "Show"].map((label) => (
            <AmChip
              key={label}
              label={label}
              selected={chip === label}
              onPress={() => setChip(label)}
            />
          ))}
        </View>
        <View style={[styles.wrap, { gap: theme.space[2] }]}>
          <AmBadge label="Verified" tone="success" icon="✓" />
          <AmBadge label="Perlu dicek" tone="warning" icon="!" />
          <AmBadge label="Tidak cocok" tone="danger" icon="×" />
          <AmBadge label="Informasi" tone="info" icon="i" />
          <AmBadge label="Netral" />
        </View>
        <View style={[styles.wrap, { gap: theme.space[2] }]}>
          <AmAvatar name="Oksa Satya" />
          <AmAvatar name="Budi" size={56} />
        </View>
      </Section>

      <Section title="Keadaan">
        <AmCard role="working">
          <View style={{ gap: theme.space[2] }}>
            <AmSkeleton height={20} width="60%" />
            <AmSkeleton height={14} />
            <AmSkeleton height={14} width="80%" />
          </View>
        </AmCard>
        <AmCard role="working">
          <AmEmptyState
            title="Belum ada modifikasi"
            body="Mulai bangun garasi digital kamu."
            actionLabel="Tambah modifikasi pertama"
            onAction={() => toast({ message: "Aksi empty state" })}
          />
        </AmCard>
        <AmCard role="working">
          <AmErrorState
            title="Garasi gagal dimuat"
            body="Data kamu aman. Coba beberapa saat lagi."
            onRetry={() => toast({ message: "Mencoba lagi", tone: "info" })}
          />
        </AmCard>
        <AmButton
          label="Tampilkan toast"
          variant="secondary"
          onPress={() => toast({ message: "Contoh pemberitahuan" })}
        />
      </Section>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", justifyContent: "space-between" },
  wrap: { flexDirection: "row", flexWrap: "wrap" },
});
