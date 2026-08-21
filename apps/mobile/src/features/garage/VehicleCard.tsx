import { StyleSheet, Text, View } from "react-native";

import { AmBadge, AmCard } from "@/components/display";
import { numeric, useTheme } from "@/theme";

import { formatKilometres, formatRupiah, formatShortDate } from "./format";
import type { Vehicle } from "./types";

export interface VehicleCardProps {
  readonly vehicle: Vehicle;
}

interface RowProps {
  readonly label: string;
  readonly value: string;
}

function Row({ label, value }: RowProps) {
  const theme = useTheme();
  return (
    <View style={styles.row}>
      <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>{label}</Text>
      <Text style={[theme.type.body, numeric, { color: theme.color.textPrimary }]}>{value}</Text>
    </View>
  );
}

/**
 * The active car, and what its service history adds up to.
 *
 * `working`, not `surface`: §77 puts anything read to make a decision on the
 * solid material, and a service cost read outdoors at a workshop is the case
 * that rule exists for.
 *
 * Every number here comes from the server. A car with no history says so in
 * words rather than showing a row of zeroes, and a null cost is omitted
 * rather than rendered as "Rp 0" — a zero the person did not spend is
 * invented data.
 */
export function VehicleCard({ vehicle }: VehicleCardProps) {
  const theme = useTheme();
  const summary = vehicle.summary;
  const meta = [
    vehicle.year?.toString(),
    vehicle.colour,
    vehicle.mileage_km === null ? null : formatKilometres(vehicle.mileage_km),
  ].filter((part): part is string => Boolean(part));

  return (
    <AmCard role="working">
      <View style={{ gap: theme.space[3] }}>
        <View style={{ gap: theme.space[1] }}>
          <Text
            accessibilityRole="header"
            style={[theme.type.h2, { color: theme.color.textPrimary }]}
          >
            {vehicle.name}
          </Text>
          {meta.length > 0 ? (
            <Text style={[theme.type.caption, numeric, { color: theme.color.textSecondary }]}>
              {meta.join(" · ")}
            </Text>
          ) : null}
        </View>

        {summary && summary.service_count > 0 ? (
          <View style={{ gap: theme.space[2] }}>
            <Row label="Servis tercatat" value={summary.service_count.toString()} />
            {summary.total_cost ? (
              <Row label="Total biaya" value={formatRupiah(summary.total_cost)} />
            ) : null}
            {summary.last_service_date ? (
              <Row label="Servis terakhir" value={formatShortDate(summary.last_service_date)} />
            ) : null}
          </View>
        ) : (
          <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
            Belum ada riwayat servis untuk mobil ini.
          </Text>
        )}

        {summary && (summary.overdue_count > 0 || summary.due_soon_count > 0) ? (
          <View style={[styles.wrap, { gap: theme.space[2] }]}>
            {summary.overdue_count > 0 ? (
              <AmBadge tone="danger" icon="!" label={`${summary.overdue_count} servis terlambat`} />
            ) : null}
            {summary.due_soon_count > 0 ? (
              <AmBadge tone="warning" icon="•" label={`${summary.due_soon_count} servis segera`} />
            ) : null}
          </View>
        ) : null}
      </View>
    </AmCard>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", justifyContent: "space-between" },
  wrap: { flexDirection: "row", flexWrap: "wrap" },
});
