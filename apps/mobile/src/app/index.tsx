import Constants from 'expo-constants';
import { useEffect, useState } from 'react';
import { ActivityIndicator, StyleSheet, Text, View } from 'react-native';

// Inlined by babel at build time; the one value the bundle carries.
const API_URL = process.env.EXPO_PUBLIC_API_URL ?? '(belum diatur)';
// Not EXPO_PUBLIC_, so not inlined — surfaced through app.config.ts `extra`.
const VARIANT =
  (Constants.expoConfig?.extra?.appVariant as string | undefined) ?? '(tidak diketahui)';

type Health = 'memeriksa' | 'terhubung' | 'gagal';

export default function Healthcheck() {
  const [status, setStatus] = useState<Health>('memeriksa');
  const [detail, setDetail] = useState('');

  useEffect(() => {
    let alive = true;
    fetch(`${API_URL}/healthz`)
      .then((res) => {
        if (!alive) return;
        setStatus(res.ok ? 'terhubung' : 'gagal');
        setDetail(`HTTP ${res.status}`);
      })
      .catch((err: unknown) => {
        if (!alive) return;
        setStatus('gagal');
        setDetail(err instanceof Error ? err.message : 'kesalahan tidak dikenal');
      });
    return () => {
      alive = false;
    };
  }, []);

  return (
    <View style={styles.container}>
      <Text style={styles.title}>Status Koneksi API</Text>
      <Text style={styles.row}>Profil: {VARIANT}</Text>
      <Text style={styles.row}>Alamat API: {API_URL}</Text>
      {status === 'memeriksa' ? (
        <View style={styles.statusRow}>
          <ActivityIndicator />
          <Text style={styles.row}>Memeriksa…</Text>
        </View>
      ) : (
        <Text style={status === 'terhubung' ? styles.ok : styles.fail}>
          {status === 'terhubung'
            ? `Terhubung (${detail})`
            : `Gagal terhubung — ${detail}`}
        </Text>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, justifyContent: 'center', gap: 12, padding: 24 },
  title: { fontSize: 20, fontWeight: '600' },
  row: { fontSize: 15 },
  statusRow: { flexDirection: 'row', alignItems: 'center', gap: 8 },
  ok: { fontSize: 15, color: '#137333' },
  fail: { fontSize: 15, color: '#c5221f' },
});
