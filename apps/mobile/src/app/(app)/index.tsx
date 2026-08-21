import Constants from "expo-constants";
import { Link } from "expo-router";
import { useEffect, useState } from "react";
import { ActivityIndicator, StyleSheet, Text, View } from "react-native";

import { SignOutConfirm } from "@/features/auth/SignOutConfirm";
import { useSession } from "@/shared";
import { useTheme } from "@/theme";

// Inlined by babel at build time; the one value the bundle carries.
const API_URL = process.env.EXPO_PUBLIC_API_URL ?? "(belum diatur)";
// Not EXPO_PUBLIC_, so not inlined — surfaced through app.config.ts `extra`.
const VARIANT =
  (Constants.expoConfig?.extra?.appVariant as string | undefined) ?? "(tidak diketahui)";

type Health = "memeriksa" | "terhubung" | "gagal";

export default function Healthcheck() {
  // AM-15 made the app-wide ground theme-aware, which turned this screen's
  // previously implicit colours invisible in dark mode (RN's default text is
  // #000000 — 1.12:1 on the dark ground). The text colours therefore come
  // from the theme; the layout and copy stay AM-14's.
  const theme = useTheme();
  const { user } = useSession();
  const [status, setStatus] = useState<Health>("memeriksa");
  const [detail, setDetail] = useState("");

  useEffect(() => {
    let alive = true;
    fetch(`${API_URL}/healthz`)
      .then((res) => {
        if (!alive) return;
        setStatus(res.ok ? "terhubung" : "gagal");
        setDetail(`HTTP ${res.status}`);
      })
      .catch((err: unknown) => {
        if (!alive) return;
        setStatus("gagal");
        setDetail(err instanceof Error ? err.message : "kesalahan tidak dikenal");
      });
    return () => {
      alive = false;
    };
  }, []);

  return (
    <View style={styles.container}>
      <Text style={[styles.title, { color: theme.color.textPrimary }]}>Status Koneksi API</Text>
      <Text style={[styles.row, { color: theme.color.textPrimary }]}>Profil: {VARIANT}</Text>
      <Text style={[styles.row, { color: theme.color.textPrimary }]}>Alamat API: {API_URL}</Text>
      {status === "memeriksa" ? (
        <View style={styles.statusRow}>
          <ActivityIndicator />
          <Text style={[styles.row, { color: theme.color.textPrimary }]}>Memeriksa…</Text>
        </View>
      ) : (
        <Text
          style={[
            styles.row,
            {
              color:
                status === "terhubung"
                  ? theme.color.semanticText.success
                  : theme.color.semanticText.danger,
            },
          ]}
        >
          {status === "terhubung" ? `Terhubung (${detail})` : `Gagal terhubung — ${detail}`}
        </Text>
      )}
      <Link href="/catalog" style={[styles.row, { color: theme.color.accentText }]}>
        Buka katalog komponen
      </Link>
      <Text style={[styles.row, { color: theme.color.textPrimary }]}>
        Masuk sebagai: {user?.displayName ?? user?.username ?? "—"}
      </Text>
      {/* TEMPORARY mount. AM-51 AC4 says sign-out lives "dari setelan", and the
          settings screen is Plan C's. This sits on the healthcheck screen only
          so the flow is exercisable before that exists. Plan C moves it into
          the profile tab and DELETES this line — shipping both would put two
          destructive sign-out controls in the app. */}
      <SignOutConfirm />
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, justifyContent: "center", gap: 12, padding: 24 },
  title: { fontSize: 20, fontWeight: "600" },
  row: { fontSize: 15 },
  statusRow: { flexDirection: "row", alignItems: "center", gap: 8 },
});
