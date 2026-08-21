const MONTHS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "Mei",
  "Jun",
  "Jul",
  "Agu",
  "Sep",
  "Okt",
  "Nov",
  "Des",
] as const;

/** 4200000 -> "4.200.000". Indonesian groups thousands with dots. */
function groupThousands(digits: string): string {
  return digits.replace(/\B(?=(\d{3})+(?!\d))/g, ".");
}

/**
 * "4200000.00" -> "Rp 4.200.000".
 *
 * The server sends money as a decimal string on purpose — a JSON number is a
 * double in most clients — so this formats the string rather than parsing it.
 * Nothing on this screen does arithmetic with a price, so nothing needs to
 * turn one into a number and risk rounding it.
 *
 * Sen are dropped: `summary.rs` pins the scale at two and rupiah has no
 * subunit in practice, so "4.200.000,00" would be noise on a card.
 */
export function formatRupiah(decimal: string): string {
  // NOTE for callers: a NULL total is omitted by the caller, never passed here
  // as "0". `Tidak boleh ada` — "A null cost is omitted, never rendered as
  // Rp 0". A genuine server "0.00" IS real and Rp 0 is correct for it; the
  // server sends JSON null, not "0", when there is no rollup, so the two mean
  // different things and must look different.
  //
  // The `?? "0"` that used to sit on the next line was dead — `String.split`
  // never returns an empty array — and it modelled exactly the anti-pattern
  // above for the next reader. Removed after Task 1's review.
  const whole = decimal.split(".")[0];
  const negative = whole.startsWith("-");
  const digits = negative ? whole.slice(1) : whole;
  return `${negative ? "-" : ""}Rp ${groupThousands(digits)}`;
}

/** 146120 -> "146.120 km". */
export function formatKilometres(km: number): string {
  return `${groupThousands(Math.trunc(km).toString())} km`;
}

/**
 * "2026-08-12" -> "12 Agu 2026".
 *
 * A twelve-entry table rather than `Intl`/`toLocaleDateString`: Hermes ships
 * without full ICU on some Android builds, and a date that silently renders in
 * English on one platform is worse than a table nobody has to think about.
 * An unparseable value is returned as-is rather than guessed at.
 */
export function formatShortDate(iso: string): string {
  const [year, month, day] = iso.split("-");
  const name = MONTHS[Number(month) - 1];
  // The day is tested for SHAPE, not truthiness. The old guard was
  // `!year || !day || !name`, which passes any non-empty string — so
  // `"2026-08-12T00:00:00Z"` left `day = "12T00:00:00Z"`, `Number(day)` was
  // NaN, and the function returned **"NaN Agu 2026"** to a person, while its
  // own doc comment above promised an unparseable value comes back as-is.
  // `"2026-08-99"` rendered "99 Agu 2026" the same way. Not reachable from
  // /vehicles today (the server serialises strictly YYYY-MM-DD) but this is an
  // exported formatter and the next caller is not bound by that. Found in
  // Task 1's review.
  if (!year || !name || !/^\d{1,2}$/.test(day ?? "")) return iso;
  return `${Number(day)} ${name} ${year}`;
}
