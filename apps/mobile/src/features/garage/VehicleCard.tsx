import { StyleSheet, Text, View } from "react-native";

import { AmBadge, AmBrandLogo, AmCard } from "@/components/display";
import { kicker, numeric, useTheme } from "@/theme";

import { formatKilometreValue, formatRupiah, formatShortDate } from "./format";
import type { Vehicle } from "./types";

export interface VehicleCardProps {
  readonly vehicle: Vehicle;
}

interface StatProps {
  readonly label: string;
  readonly value: string;
  readonly flex: number;
}

function Stat({ label, value, flex }: StatProps) {
  const theme = useTheme();
  return (
    <View style={{ flex, gap: 3 }}>
      <Text style={[theme.type.micro, kicker, { color: theme.color.textTertiary }]}>{label}</Text>
      <Text style={[theme.type.label, numeric, { color: theme.color.textPrimary }]}>{value}</Text>
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
 * The odometer is the hero because it is the number an owner checks, and
 * because it is the one figure every later feature — service intervals, a
 * build's mileage, a problem report — is measured against. Everything below
 * it is a summary row, deliberately quieter.
 *
 * Every number here comes from the server. A car with no history says so in
 * words rather than showing a row of zeroes, a null cost is omitted rather
 * than rendered as "Rp 0", and a car with no recorded mileage shows no
 * odometer rather than a zero the owner never drove — an invented number is
 * the thing this product must never do.
 *
 * NOTHING IN HERE IS INTERACTIVE, and that is a constraint rather than a
 * preference. On this build a `Pressable` nested inside this card renders but
 * never fires — verified on the simulator against an identical control placed
 * outside it, which does. The mockup draws the "Ganti" switcher inside this
 * card's header; it lives on the Home screen instead, above the card, because
 * a control that draws and does nothing is worse than one that is absent. See
 * the AmMaterial entry in the review ledger.
 */
export function VehicleCard({ vehicle }: VehicleCardProps) {
  const theme = useTheme();
  const summary = vehicle.summary;
  // Mileage is deliberately NOT in this line. It is the odometer hero below,
  // and the mockup this card follows carried it in both places — the same
  // "146.120 km" twice on one card, which reads as two different figures for
  // a moment before it reads as a repeat.
  const meta = [vehicle.year?.toString(), vehicle.colour].filter((part): part is string =>
    Boolean(part),
  );

  const hasHistory = Boolean(summary && summary.service_count > 0);
  const flagged = Boolean(summary && (summary.overdue_count > 0 || summary.due_soon_count > 0));

  return (
    <AmCard role="working" padding={5} radius="xl">
      <View style={{ gap: theme.space[3] }}>
        <View style={[styles.identity, { gap: theme.space[3] }]}>
          <AmBrandLogo domain={vehicle.brand_logo_domain} name={vehicle.name} size={34} />
          <View style={styles.identityText}>
            <Text
              accessibilityRole="header"
              numberOfLines={1}
              style={[theme.type.h3, { color: theme.color.textPrimary }]}
            >
              {vehicle.name}
            </Text>
            {meta.length > 0 ? (
              <Text style={[theme.type.caption, numeric, { color: theme.color.textSecondary }]}>
                {meta.join(" · ")}
              </Text>
            ) : null}
          </View>
        </View>

        {vehicle.mileage_km === null ? null : (
          <View style={{ gap: 2 }}>
            <Text style={[theme.type.display, numeric, { color: theme.color.textPrimary }]}>
              {formatKilometreValue(vehicle.mileage_km)}
              <Text style={[theme.type.label, { color: theme.color.textSecondary }]}> km</Text>
            </Text>
            <Text style={[theme.type.micro, kicker, { color: theme.color.textTertiary }]}>
              Odometer
            </Text>
          </View>
        )}

        {hasHistory && summary ? (
          <>
            <View style={[styles.rule, { backgroundColor: theme.color.border }]} />
            <View style={[styles.row, { gap: theme.space[3] }]}>
              <Stat label="Servis" value={`${summary.service_count} tercatat`} flex={1} />
              {summary.total_cost ? (
                <Stat label="Total biaya" value={formatRupiah(summary.total_cost)} flex={1.2} />
              ) : null}
              {summary.last_service_date ? (
                <Stat
                  label="Terakhir"
                  value={formatShortDate(summary.last_service_date)}
                  flex={1}
                />
              ) : null}
            </View>
          </>
        ) : (
          <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
            Belum ada riwayat servis untuk mobil ini.
          </Text>
        )}

        {flagged && summary ? (
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
  identity: { flexDirection: "row", alignItems: "center" },
  identityText: { flex: 1, gap: 2 },
  rule: { height: 1 },
  wrap: { flexDirection: "row", flexWrap: "wrap" },
});
