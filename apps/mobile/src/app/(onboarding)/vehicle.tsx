import { useQueryClient } from "@tanstack/react-query";
import { useRouter } from "expo-router";
import { useEffect } from "react";
import { ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmButton, AmSelect } from "@/components/input";
import { AmEmptyState, AmErrorState, AmSkeleton, useToast } from "@/components/state";
import { FormNotice } from "@/features/auth/FormNotice";
import { WizardProgress } from "@/features/onboarding/WizardProgress";
import { WIZARD_STEPS, canAdvance, useDraft } from "@/features/onboarding/draft";
import { vehiclesQueryKey } from "@/features/garage/queries";
import { VehiclePhotoPlaceholder } from "@/features/vehicle/VehiclePhotoPlaceholder";
import {
  generationOptions,
  toOptions,
  useBrands,
  useGenerations,
  useModels,
  useVariants,
  variantOptions,
  yearOptions,
} from "@/features/vehicle/catalog";
import { describedAsFrom, useCreateVehicle } from "@/features/vehicle/createVehicle";
import { refreshMe, setActiveVehicleId, useSession } from "@/shared";
import { useTheme } from "@/theme";

export default function VehicleWizard() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const router = useRouter();
  const queryClient = useQueryClient();
  const toast = useToast();
  const { user } = useSession();
  const draft = useDraft();
  const adoptUser = useDraft((state) => state.adoptUser);
  const create = useCreateVehicle();

  const userId = user?.id ?? null;
  // Keyed on the id alone. Depending on the whole draft object would re-run
  // this on every keystroke and every step change, because zustand hands back
  // a new object each time state moves.
  useEffect(() => {
    if (userId !== null) adoptUser(userId);
  }, [userId, adoptUser]);

  const brands = useBrands();
  const models = useModels(draft.brand?.id ?? null);
  const generations = useGenerations(draft.model?.id ?? null);
  const variants = useVariants(draft.generation?.id ?? null);

  const index = WIZARD_STEPS.indexOf(draft.step);
  const isLast = index === WIZARD_STEPS.length - 1;

  // AM-113 AC1: backwards is always allowed and clears nothing. The cascade
  // in the store fires on a CHANGED value, not on revisiting a step.
  const back = () => {
    const previous = WIZARD_STEPS[index - 1];
    if (previous) draft.goTo(previous);
  };

  const forward = () => {
    const next = WIZARD_STEPS[index + 1];
    if (next) draft.goTo(next);
  };

  const save = () => {
    const brand = draft.brand;
    const model = draft.model;
    if (!brand || !model) return;

    // Captured BEFORE the refresh below flips it. AM-113 AC5 branches on
    // whether this is the account's first car, and `me` is about to say it is
    // not.
    const wasFirstCar = user?.hasVehicles === false;
    const vehicleName = describedAsFrom(brand.name, model.name, draft.year);

    create.mutate(
      {
        variantId: draft.variant?.id ?? null,
        describedAs: vehicleName,
        year: draft.year,
      },
      {
        onSuccess: async ({ id }) => {
          setActiveVehicleId(id);

          // Awaited, not fired and forgotten. The onboarding gate routes on
          // `Me.hasVehicles`, which lives in the session store rather than the
          // query cache — `refreshMe()` is the only thing that rewrites it,
          // and navigating before it resolves sends the person straight back
          // into the wizard they just finished.
          await refreshMe();
          await queryClient.invalidateQueries({ queryKey: vehiclesQueryKey });

          if (wasFirstCar) {
            router.replace({
              pathname: "/aha",
              params: { vehicleId: id, vehicleName },
            });
          } else {
            // AM-113 AC5 sends a non-first car to "halaman kendaraannya" —
            // AM-116, which has no screen. The garage is the honest
            // substitute, and the car is already active there.
            toast({ message: `${vehicleName} masuk garasi.`, tone: "success" });
            router.replace("/(app)/garage");
          }

          // After the navigation, not before: clearing first drops the wizard
          // back to step one for the frame between the reset and the route
          // change, which reads as the whole thing being thrown away.
          draft.clear();
        },
      },
    );
  };

  // Rendered in place of the picker while a level is loading, has failed, or
  // has come back empty. An empty catalog level is a real state on a fresh
  // database (`make db-drop` without `make db-seed`), not a fault.
  const stepBody = () => {
    switch (draft.step) {
      case "brand":
        if (brands.isPending) return <Skeletons />;
        if (brands.isError)
          return (
            <AmErrorState
              title="Katalog gagal dimuat"
              body={brands.error.message}
              onRetry={() => void brands.refetch()}
            />
          );
        if (brands.data.length === 0)
          return (
            <AmEmptyState
              title="Katalog masih kosong"
              body="Belum ada merek yang bisa dipilih. Coba muat ulang sebentar lagi."
              actionLabel="Muat ulang"
              onAction={() => void brands.refetch()}
            />
          );
        return (
          <AmSelect
            label="Merek"
            value={draft.brand?.id ?? null}
            options={toOptions(brands.data)}
            placeholder="Pilih merek"
            onChange={(id) => {
              const picked = brands.data.find((entry) => entry.id === id);
              if (picked) draft.setBrand({ id: picked.id, name: picked.name });
            }}
          />
        );

      case "model":
        if (models.isPending) return <Skeletons />;
        if (models.isError)
          return (
            <AmErrorState
              title="Model gagal dimuat"
              body={models.error.message}
              onRetry={() => void models.refetch()}
            />
          );
        if (models.data.length === 0)
          return (
            <AmEmptyState
              title="Belum ada model untuk merek ini"
              body="Katalog belum mencatat model apa pun di bawah merek ini."
              actionLabel="Pilih merek lain"
              onAction={back}
            />
          );
        return (
          <AmSelect
            label="Model"
            value={draft.model?.id ?? null}
            options={toOptions(models.data)}
            placeholder="Pilih model"
            onChange={(id) => {
              const picked = models.data.find((entry) => entry.id === id);
              if (picked) draft.setModel({ id: picked.id, name: picked.name });
            }}
          />
        );

      case "generation":
        if (generations.isPending) return <Skeletons />;
        if (generations.isError)
          return (
            <AmErrorState
              title="Generasi gagal dimuat"
              body={generations.error.message}
              onRetry={() => void generations.refetch()}
            />
          );
        if (generations.data.length === 0)
          return (
            <AmEmptyState
              title="Belum ada generasi untuk model ini"
              body="Katalog belum mencatat generasi apa pun di bawah model ini."
              actionLabel="Pilih model lain"
              onAction={back}
            />
          );
        return (
          <AmSelect
            label="Generasi"
            value={draft.generation?.id ?? null}
            options={generationOptions(generations.data)}
            placeholder="Pilih generasi"
            onChange={(id) => {
              const picked = generations.data.find((entry) => entry.id === id);
              if (picked)
                draft.setGeneration({
                  id: picked.id,
                  name: picked.name,
                  yearStart: picked.year_start,
                  yearEnd: picked.year_end,
                  years: picked.years,
                });
            }}
          />
        );

      case "year": {
        // AM-113 AC2: the range comes from the chosen generation, and the
        // server supplies it — year_start with year_end, where null means the
        // generation is still in production.
        const generation = draft.generation;
        if (!generation) return null;
        return (
          <AmSelect
            label={`Tahun (${generation.years})`}
            value={draft.year === null ? null : String(draft.year)}
            options={yearOptions({
              id: generation.id,
              name: generation.name,
              year_start: generation.yearStart,
              year_end: generation.yearEnd,
              years: generation.years,
            })}
            placeholder="Pilih tahun"
            onChange={(value) => draft.setYear(Number(value))}
          />
        );
      }

      case "variant":
        if (variants.isPending) return <Skeletons />;
        if (variants.isError)
          return (
            <AmErrorState
              title="Varian gagal dimuat"
              body={variants.error.message}
              onRetry={() => void variants.refetch()}
            />
          );
        if (variants.data.length === 0)
          return (
            <AmEmptyState
              title="Belum ada varian untuk generasi ini"
              body="Kamu tetap bisa lanjut. Varian menentukan kode mesin dan transmisi, dan itu bisa dilengkapi nanti."
              actionLabel="Lanjut tanpa varian"
              onAction={() => {
                draft.skipVariant();
                forward();
              }}
            />
          );
        return (
          <View style={{ gap: theme.space[3] }}>
            <AmSelect
              label="Varian"
              value={draft.variant?.id ?? null}
              options={variantOptions(variants.data)}
              placeholder="Pilih varian"
              onChange={(id) => {
                const picked = variants.data.find((entry) => entry.id === id);
                if (picked) draft.setVariant({ id: picked.id, name: picked.name });
              }}
            />
            {/*
              AM-113 AC3. This skips the VARIANT, which is what carries the
              engine code and transmission — it is not a skip of the wizard.
              Filling it in later is the vehicle page, which is AM-116; the
              server already accepts it through PUT /vehicles/{id}.
            */}
            <AmButton
              label="Saya tidak tahu varian mobil saya"
              variant="ghost"
              onPress={() => {
                draft.skipVariant();
                forward();
              }}
            />
          </View>
        );

      case "photo":
        return (
          <View style={{ gap: theme.space[3] }}>
            <VehiclePhotoPlaceholder />
            <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
              Unggah foto belum tersedia. Mobil kamu akan tampil dengan gambar netral ini sampai
              fiturnya siap.
            </Text>
          </View>
        );
    }
  };

  return (
    <ScrollView
      contentContainerStyle={{
        flexGrow: 1,
        padding: theme.pagePadding,
        paddingTop: insets.top + theme.space[6],
        paddingBottom: insets.bottom + theme.space[10],
        gap: theme.space[6],
      }}
      keyboardShouldPersistTaps="handled"
    >
      <View style={{ gap: theme.space[3] }}>
        <Text
          accessibilityRole="header"
          style={[theme.type.h1, { color: theme.color.textPrimary }]}
        >
          Mobil kamu apa?
        </Text>
        <WizardProgress step={draft.step} />
      </View>

      {stepBody()}

      <View style={{ marginTop: "auto", gap: theme.space[3] }}>
        {create.error ? <FormNotice tone="danger" message={create.error.message} /> : null}
        {/*
          There is NO skip control here, and there must never be one. AM-55
          AC2: "tidak ada tombol lewati, karena aplikasi tanpa mobil tidak
          punya isi". The enforcement is Plan A's route gate; this screen
          simply offers no way out, and adds no second guard of its own.
        */}
        <AmButton
          label={isLast ? "Simpan mobil saya" : "Lanjut"}
          variant="accent"
          size="lg"
          disabled={!canAdvance(draft, draft.step)}
          loading={create.isPending}
          onPress={isLast ? save : forward}
        />
        {index > 0 ? <AmButton label="Kembali" variant="ghost" onPress={back} /> : null}
      </View>
    </ScrollView>
  );
}

function Skeletons() {
  const theme = useTheme();
  return (
    <View style={{ gap: theme.space[2] }}>
      <AmSkeleton height={20} width="40%" />
      <AmSkeleton height={52} />
    </View>
  );
}
