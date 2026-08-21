import { Link, router } from "expo-router";
import { useMemo, useState } from "react";
import { KeyboardAvoidingView, Platform, ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmBrandLockup } from "@/components/display";
import { AmButton, AmTextField } from "@/components/input";
import { asApiError, isSignInFailure, useRegister } from "@/features/auth/api";
import { registerConflictOf } from "@/features/auth/conflict";
import { setPendingEmail } from "@/features/auth/pendingEmail";
import { ConsentCheckbox } from "@/features/auth/ConsentCheckbox";
import { PasswordStrength } from "@/features/auth/PasswordStrength";
import { fieldErrorsOf, registerSchema, USERNAME_PATTERN } from "@/features/auth/schemas";
import {
  useUsernameAvailability,
  type Availability,
} from "@/features/auth/useUsernameAvailability";
import { resumeSignIn } from "@/shared";
import { useTheme } from "@/theme";

const CONSENT_LABEL = "Saya setuju dengan Syarat Layanan dan Kebijakan Privasi AnakMobil.";

const AVAILABILITY_HINT: Record<Availability, string> = {
  idle: "3–30 karakter. Huruf kecil, angka, titik, dan garis bawah.",
  checking: "Memeriksa ketersediaan…",
  available: "Username ini tersedia.",
  taken: "Username ini sudah dipakai.",
  unknown: "Belum bisa memeriksa sekarang. Kamu tetap bisa lanjut.",
};

export default function Register() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const register = useRegister();

  const [email, setEmail] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [consent, setConsent] = useState(false);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<string | null>(null);

  /**
   * Drop one field's error the moment its value changes.
   *
   * Without this, `errors` was written only by `submit()` and cleared only by
   * another `submit()`, so a server message outlived the value it was about:
   * submit a genuinely taken `budi` while the availability endpoint is down,
   * then type `budi.satya`, and "Username ini sudah dipakai." still sat under
   * the new name — while `hint={usernameError ? undefined : ...}` suppressed
   * the availability hint entirely, so "Username ini tersedia." could never
   * render even once the endpoint recovered. The button stayed enabled
   * throughout, so the screen said the name was taken and let it be submitted.
   * The only thing that cleared it was the very submit the false message was
   * discouraging. Found in Task 3's review.
   */
  const clearError = (field: string) =>
    setErrors((prev) => {
      if (!(field in prev)) return prev;
      const next = { ...prev };
      delete next[field];
      return next;
    });
  const [retrying, setRetrying] = useState(false);
  // AM-59: the account already exists on this email. Cleared the moment the
  // email field changes, so the panel never outlives the address it was about.
  // The address the server actually refused — NOT a boolean.
  //
  // A flag was bound to nothing: the email field stays editable while the POST
  // is in flight (only the button is gated), so correcting a typo mid-request
  // and then receiving the 409 for the OLD address left the panel saying
  // "Email ini sudah punya akun." about an address that has no account — and
  // "Masuk dengan email ini" then seeded login with the new one, which answers
  // "Email atau kata sandi salah." Holding the address makes the panel's own
  // equality check the guard. Found in Task 4's review.
  const [emailTaken, setEmailTaken] = useState<string | null>(null);

  // Mirrors registerSchema's own `.trim().toLowerCase()` on the username
  // field exactly, so the name checked here is always the name that gets
  // submitted — checking the raw (possibly padded/mixed-case, e.g. pasted
  // from a password manager) field text would check a different string than
  // the canonical one the form actually sends.
  const canonicalUsername = username.trim().toLowerCase();
  const shapeOk = USERNAME_PATTERN.test(canonicalUsername);
  const availability = useUsernameAvailability(canonicalUsername, shapeOk);

  const values = useMemo(
    () => ({ email: email.trim(), username, password, consent }),
    [email, username, password, consent],
  );
  const parsed = registerSchema.safeParse(values);

  // The token pair, present only when the POST succeeded and `signIn` then
  // failed — see `SignInFailure` in api.ts.
  //
  // CORRECTED. This used to read `register.data`, on the assumption that the
  // success value survives into the error state. It does not: query-core's
  // reducer does `case "error": return { ...state, data: void 0, ... }`, so
  // `register.data` is ALWAYS undefined when `isError` is true and every
  // branch below was unreachable — a retry that looked implemented and never
  // ran. The pair now rides the error instead.
  const pendingTokens =
    register.error !== null && isSignInFailure(register.error) ? register.error.tokens : null;

  const canSubmit = pendingTokens
    ? true
    : parsed.success && availability !== "taken" && !register.isPending;
  const busy = register.isPending || retrying;

  const submit = () => {
    setFormError(null);

    if (pendingTokens) {
      // The registration already succeeded — `pendingTokens` is the token
      // pair from that 2xx. Getting here means `signIn` (writeSession ->
      // GET /me -> start the cache -> flip to signedIn) failed partway,
      // most likely offline. Retrying MUST resume from `signIn`, never
      // re-submit the form: a second POST /auth/register would be told the
      // person's own thirty-second-old email is already taken, while a
      // valid, unused token pair sits on the device. See the "A CALLER MUST
      // NOT RE-SUBMIT ON ERROR" doc comment above `useRegister` in api.ts.
      setRetrying(true);
      // `resumeSignIn`, NOT `signIn` — `signIn` starts with `writeSession`, and
      // if the failed attempt already triggered a refresh, rewriting the older
      // pair makes the next 401 present a spent refresh token, which the server
      // reads as reuse and answers by revoking every session on every device.
      // See that function's own comment. Found in Task 2's review.
      resumeSignIn(pendingTokens).catch((error: unknown) => {
        setRetrying(false);
        setFormError(asApiError(error).message);
      });
      return;
    }

    if (!parsed.success) {
      setErrors(fieldErrorsOf(parsed.error));
      return;
    }
    setErrors({});
    const { consent: _agreed, ...input } = parsed.data;
    register.mutate(input, {
      onError: (error) => {
        // AM-50 AC3 / AM-59: an email collision gets the "masuk dengan email
        // ini" panel, not a field message — checked before the generic
        // `error.fields` branch so it does not fall through to it.
        const conflict = registerConflictOf(error);
        if (conflict === "email") {
          setEmailTaken(input.email);
          return;
        }
        if (conflict === "username") {
          // The race-loser case the live availability check cannot catch:
          // the name was free when checked and taken by the time of submit.
          setErrors({ username: error.fields?.username ?? "Username ini sudah dipakai." });
          return;
        }
        // A validation error WITH `fields` is field-scoped (AM-57 puts the
        // message under the field that failed) and its top-level
        // `error.message` must NOT also render as a banner — it can be
        // actively misleading (a taken-email 409 carries the generic "Data
        // ini sudah berubah" at the top level and the real text in
        // `fields.email`). Only a validation error with NO `fields` (the
        // server could not attach one — e.g. a malformed request) falls
        // through to the banner, so a bad submit never fails silently.
        if (error.fields) {
          setErrors(error.fields);
          return;
        }
        setFormError(error.message);
      },
    });
  };

  const usernameError = availability === "taken" ? AVAILABILITY_HINT.taken : errors.username;

  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === "ios" ? "padding" : undefined}
      style={{ flex: 1 }}
    >
      <ScrollView
        keyboardShouldPersistTaps="handled"
        contentContainerStyle={{
          // See login.tsx: `flexGrow: 1` gives the footer's `marginTop: "auto"`
          // something to push against.
          flexGrow: 1,
          padding: theme.pagePadding,
          paddingTop: insets.top + theme.space[10],
          paddingBottom: insets.bottom + theme.space[4],
          gap: theme.space[8],
        }}
      >
        <View style={{ gap: theme.space[5] }}>
          <AmBrandLockup variant="header" />
          <Text
            accessibilityRole="header"
            style={[theme.type["body-lg"], { color: theme.color.textSecondary }]}
          >
            Bikin garasi digital kamu. Gratis, selamanya.
          </Text>
        </View>

        {/* No card — same reasoning as login.tsx. */}
        <View style={{ gap: theme.space[4] }}>
          <AmTextField
            label="Email"
            value={email}
            onChangeText={(value) => {
              setEmail(value);
              setEmailTaken(null);
              clearError("email");
            }}
            placeholder="nama@email.com"
            error={errors.email}
            keyboardType="email-address"
            autoCapitalize="none"
            autoCorrect={false}
            autoComplete="email"
            textContentType="emailAddress"
          />

          <AmTextField
            label="Username"
            value={username}
            // Lowercased on the way in, matching the server's canonicaliser.
            // Doing it here rather than only at submit means what the
            // person sees is what the server will store.
            onChangeText={(value) => {
              setUsername(value.toLowerCase());
              clearError("username");
            }}
            placeholder="oksa.satya"
            hint={usernameError ? undefined : AVAILABILITY_HINT[availability]}
            error={usernameError}
            maxLength={30}
            autoCapitalize="none"
            autoCorrect={false}
            autoComplete="username"
            textContentType="username"
          />

          <View style={{ gap: theme.space[2] }}>
            <AmTextField
              label="Kata sandi"
              value={password}
              onChangeText={(value) => {
                setPassword(value);
                clearError("password");
              }}
              error={errors.password}
              secureTextEntry
              autoCapitalize="none"
              autoCorrect={false}
              autoComplete="new-password"
              textContentType="newPassword"
            />
            <PasswordStrength password={password} />
          </View>

          <ConsentCheckbox checked={consent} onChange={setConsent} label={CONSENT_LABEL} />

          {formError ? (
            <Text
              accessibilityLiveRegion="polite"
              accessibilityRole="alert"
              style={[theme.type.caption, { color: theme.color.semanticText.danger }]}
            >
              {formError}
            </Text>
          ) : null}

          <AmButton
            label={pendingTokens ? "Lanjutkan" : "Daftar"}
            onPress={submit}
            size="lg"
            variant="accent"
            loading={busy}
            disabled={!canSubmit || busy}
          />

          {emailTaken !== null && emailTaken === values.email ? (
            <View
              accessibilityLiveRegion="polite"
              style={{ gap: theme.space[3], marginTop: theme.space[2] }}
            >
              <Text style={[theme.type.body, { color: theme.color.textPrimary }]}>
                Email ini sudah punya akun.
              </Text>
              <AmButton
                label="Masuk dengan email ini"
                variant="secondary"
                onPress={() => {
                  // The refused address, not whatever is in the field now,
                  // and NOT through a route param — see `pendingEmail.ts`
                  // for why a query string is the wrong carrier here.
                  setPendingEmail(emailTaken);
                  router.push("/login");
                }}
              />
              {/* AM-54 (password reset) does not exist yet — no endpoint, no
                  screen, no email sender. This stays one honest sentence,
                  never a button: a control whose only behaviour is to say
                  "not available" is a dead end wearing a button, which is
                  what AM-59's definition of done forbids. */}
              <Text style={[theme.type.caption, { color: theme.color.textTertiary }]}>
                Pengaturan ulang kata sandi belum tersedia di aplikasi.
              </Text>
            </View>
          ) : null}
        </View>

        {/* Pinned to the bottom when the form leaves room; flows after it when
            it does not. */}
        <View style={{ marginTop: "auto", gap: theme.space[1], alignItems: "center" }}>
          <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
            Sudah punya akun?
          </Text>
          {/* Vertical padding, not hitSlop: the label type is 13px/18px, so the
              tap target is ~18pt against this plan's 44pt floor. The Am*
              primitives apply minHeight after the caller's style so it cannot be
              defeated, but a bare Link is not one of them. expo-router's Link
              does NOT accept hitSlop (not on LinkProps); padding on the
              underlying Text grows the touchable region instead. space[4] is 16,
              so 18 + 32 = 50. Found in Task 2's review. */}
          <Link
            href="/login"
            style={[
              theme.type.label,
              { color: theme.color.accentText, paddingVertical: theme.space[4] },
            ]}
          >
            Masuk
          </Link>
        </View>
      </ScrollView>
    </KeyboardAvoidingView>
  );
}
