import { useQuery } from "@tanstack/react-query";

import type { AmSelectOption } from "@/components/input";
import { apiRequest } from "@/shared";

import type { CatalogEntry, Generation, Variant } from "./types";

/**
 * Each level is enabled only once its parent has been chosen. That `enabled`
 * chain IS AM-113 AC2's narrowing — there is no client-side filtering to get
 * wrong, because the server only ever returns the children of the parent in
 * the path.
 */
export function useBrands() {
  return useQuery({
    queryKey: ["catalog", "brands"],
    queryFn: () => apiRequest<CatalogEntry[]>("/catalog/brands"),
  });
}

export function useModels(brandId: string | null) {
  return useQuery({
    queryKey: ["catalog", "models", brandId],
    queryFn: () => apiRequest<CatalogEntry[]>(`/catalog/brands/${brandId}/models`),
    enabled: brandId !== null,
  });
}

export function useGenerations(modelId: string | null) {
  return useQuery({
    queryKey: ["catalog", "generations", modelId],
    queryFn: () => apiRequest<Generation[]>(`/catalog/models/${modelId}/generations`),
    enabled: modelId !== null,
  });
}

export function useVariants(generationId: string | null) {
  return useQuery({
    queryKey: ["catalog", "variants", generationId],
    queryFn: () => apiRequest<Variant[]>(`/catalog/generations/${generationId}/variants`),
    enabled: generationId !== null,
  });
}

export function toOptions(entries: readonly CatalogEntry[]): AmSelectOption<string>[] {
  return entries.map((entry) => ({ value: entry.id, label: entry.name }));
}

/**
 * The years string goes into the label rather than a subtitle line: it is how
 * a person recognises their generation, and AmSelect has one line per option.
 *
 * Appended only when the name does not already carry it. The seeded names do
 * — "Gen 3 (2022–kini)" — so appending unconditionally rendered
 * "Gen 3 (2022–kini) · 2022–", which reads as a data fault rather than a
 * label. Found by opening the generation step, not by the type checker.
 */
export function generationOptions(list: readonly Generation[]): AmSelectOption<string>[] {
  return list.map((g) => ({
    value: g.id,
    label: g.name.includes(g.years) ? g.name : `${g.name} · ${g.years}`,
  }));
}

export function variantOptions(list: readonly Variant[]): AmSelectOption<string>[] {
  return list.map((v) => ({
    value: v.id,
    label: v.engine_code ? `${v.name} · ${v.engine_code}` : v.name,
  }));
}

/**
 * AM-113 AC2: the year list is the generation's own range, newest first.
 *
 * `year_end: null` means still in production, so the range ends at the
 * current year. `currentYear` is a parameter rather than a call to the clock
 * for the same reason `derive_reminders` takes `today` — it is what makes the
 * function checkable without waiting a year.
 *
 * The clamp matters: a generation whose start is in the future would
 * otherwise produce an empty list, and an empty year step has no honest
 * empty state because a generation always has at least its opening year.
 */
export function yearOptions(
  g: Generation,
  currentYear: number = new Date().getFullYear(),
): AmSelectOption<string>[] {
  const last = Math.max(g.year_start, g.year_end ?? currentYear);
  const years: AmSelectOption<string>[] = [];
  for (let year = last; year >= g.year_start; year -= 1) {
    years.push({ value: String(year), label: String(year) });
  }
  return years;
}
