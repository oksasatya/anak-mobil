import { useMutation } from "@tanstack/react-query";
import { useRouter } from "expo-router";
import { useEffect } from "react";
import { KeyboardAvoidingView, Platform, ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmAvatar, AmCard } from "@/components/display";
import { AmButton, AmTextField } from "@/components/input";
import { FormNotice } from "@/features/auth/FormNotice";
import { useDraft } from "@/features/onboarding/draft";
import { apiRequest, refreshMe, useSession, type ApiError } from "@/shared";
import { useTheme } from "@/theme";

/**
 * The server's own rule, mirrored — not a stricter one invented here.
 *
 * `PATCH /me` (adapter/http/profile.rs) trims, refuses empty, refuses past 60
 * characters, and refuses a name with no alphanumeric in it. Those three are
 * what the button gates on. Anything subtler the server rejects — an invisible
 * character, a bidi override — comes back as a 422 on `display_name` and
 * lands under the field, because duplicating that check here would be a second
 * definition of the rule that drifts the moment the server's changes.
 */
const MAX_NAME = 60;

function nameIsSendable(name: string): boolean {
  return name.length > 0 && name.length <= MAX_NAME && /[\p{L}\p{N}]/u.test(name);
}

export default function ProfileStep() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const router = useRouter();
  const { user } = useSession();

  // Subscribed field by field, and the actions taken as stable references.
  // Taking the whole store would hand every effect below a new object on each
  // keystroke, which is how an "adopt the user once" effect quietly becomes an
  // effect that runs on every character typed.
  const displayName = useDraft((state) => state.displayName);
  const setDisplayName = useDraft((state) => state.setDisplayName);
  const adoptUser = useDraft((state) => state.adoptUser);

  const userId = user?.id ?? null;
  const serverName = user?.displayName ?? null;

  useEffect(() => {
    if (userId === null) return;
    // A draft stamped with a different account is discarded rather than shown.
    adoptUser(userId);
    // Seeded from the server only when the draft is genuinely empty, so
    // somebody returning after a partial onboarding sees the name they already
    // saved — and somebody mid-edit does not have it overwritten.
    if (useDraft.getState().displayName === "" && serverName !== null) {
      useDraft.getState().setDisplayName(serverName);
    }
  }, [userId, serverName, adoptUser]);

  const name = displayName.trim();
  const valid = nameIsSendable(name);

  const save = useMutation<void, ApiError, string>({
    mutationFn: async (value) => {
      await apiRequest("/me", { method: "PATCH", body: { display_name: value } });
      // The gate in `gates.tsx` routes on the session store, which is not the
      // query cache — `refreshMe()` is the one thing that rewrites it. Awaited
      // before navigating, or the wizard opens against a stale profile.
      await refreshMe();
    },
    onSuccess: () => router.replace("/(onboarding)/vehicle"),
  });

  const fieldError = save.error?.fields?.display_name;
  const bannerError = save.error && save.error.kind !== "validation" ? save.error.message : null;

  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === "ios" ? "padding" : undefined}
      style={{ flex: 1 }}
    >
      <ScrollView
        keyboardShouldPersistTaps="handled"
        contentContainerStyle={{
          flexGrow: 1,
          padding: theme.pagePadding,
          paddingTop: insets.top + theme.space[10],
          paddingBottom: insets.bottom + theme.space[4],
          gap: theme.space[8],
        }}
      >
        <View style={{ gap: theme.space[2] }}>
          <Text
            accessibilityRole="header"
            style={[theme.type.h1, { color: theme.color.textPrimary }]}
          >
            Kenalan dulu
          </Text>
          <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
            Satu langkah singkat, lalu langsung ke mobil kamu.
          </Text>
        </View>

        <AmCard role="working">
          <View style={{ gap: theme.space[5] }}>
            <View style={{ alignItems: "center", gap: theme.space[3] }}>
              <AmAvatar name={name === "" ? "?" : name} size={72} />
              {/*
                AM-55 AC1 asks for an optional profile photo. There is no
                upload endpoint, no photo column on `users`, and no storage
                story — so the honest thing is to say so rather than render a
                control that cannot work. Initials stand in until there is one.
              */}
              <Text
                style={[
                  theme.type.caption,
                  { color: theme.color.textTertiary, textAlign: "center" },
                ]}
              >
                Foto profil belum bisa diunggah. Untuk sekarang, inisial nama kamu yang tampil.
              </Text>
            </View>

            <AmTextField
              label="Nama tampilan"
              value={displayName}
              onChangeText={setDisplayName}
              placeholder="Budi Santoso"
              hint="Nama ini yang dilihat pengguna lain."
              autoCapitalize="words"
              autoComplete="name"
              textContentType="name"
              maxLength={MAX_NAME}
              error={fieldError}
            />
          </View>
        </AmCard>

        <View style={{ marginTop: "auto", gap: theme.space[4] }}>
          {bannerError ? <FormNotice tone="danger" message={bannerError} /> : null}
          <AmButton
            label="Lanjut ke mobil saya"
            variant="accent"
            size="lg"
            disabled={!valid}
            loading={save.isPending}
            onPress={() => save.mutate(name)}
          />
        </View>
      </ScrollView>
    </KeyboardAvoidingView>
  );
}
