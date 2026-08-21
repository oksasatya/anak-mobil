import { Link } from "expo-router";
import { useState } from "react";
import { KeyboardAvoidingView, Platform, ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { AmBrandLockup } from "@/components/display";
import { AmButton, AmTextField } from "@/components/input";
import { asApiError, isSignInFailure, useLogin } from "@/features/auth/api";
import { takePendingEmail } from "@/features/auth/pendingEmail";
import { fieldErrorsOf, loginSchema } from "@/features/auth/schemas";
import { formatCountdown, useCountdown } from "@/features/auth/useCountdown";
import { resumeSignIn } from "@/shared";
import { useTheme } from "@/theme";

/**
 * The one string for a refused credential.
 *
 * A constant rather than two literals, because AM-51 AC2 is a security
 * measure: an unknown email and a wrong password must be indistinguishable,
 * and two literals in two branches is how they quietly stop being. This is a
 * deliberate client-owned copy rather than a pass-through of `error.message`
 * — the server already answers both cases with one identical 401 message
 * (`"Email atau password salah."`, verified against the running API), but
 * pinning the client's own string means this screen stays indistinguishable
 * even if a future server change ever let the two paths diverge.
 */
const CREDENTIALS_REFUSED = "Email atau kata sandi salah.";
// Shown only when the login itself worked and the session could not be
// finished. It must not read like a failure: the account is fine and the
// credentials are already on the device.
const SIGN_IN_INTERRUPTED = "Koneksi terputus sebelum selesai. Tap lanjutkan untuk meneruskan.";

export default function Login() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const login = useLogin();
  const countdown = useCountdown();
  // AM-59: seeded once from a register-screen handoff (AM-50 AC3's
  // already-registered path). `useState`'s initialiser runs exactly once, so
  // the param fills the field and the person owns it from there — an effect
  // that re-synced the param into state would overwrite what they typed on
  // every re-render.

  const [email, setEmail] = useState(takePendingEmail);
  const [password, setPassword] = useState("");
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [retrying, setRetrying] = useState(false);

  // The token pair from a login whose POST succeeded but whose `signIn` then
  // failed. Non-null means: do NOT submit again, resume instead.
  //
  // DERIVED from the mutation, not stored. It used to be `useState`, written
  // once and never cleared, so a resume that could never succeed — a Keystore
  // invalidated by a biometric re-enrolment, say — left the button reading
  // "Lanjutkan" forever with no path back to an ordinary login, and typing a
  // different password was silently ignored. `register.tsx` derived it from
  // the outset; this screen was the outlier. Found in Task 2's review.
  const pendingTokens =
    login.error !== null && isSignInFailure(login.error) ? login.error.tokens : null;

  const waiting = countdown.remaining > 0;
  const busy = login.isPending || retrying;

  const submit = () => {
    setFormError(null);

    if (pendingTokens !== null) {
      // The login already succeeded — this pair came back with the 2xx. Only
      // `signIn` (writeSession -> GET /me -> start the cache -> flip to
      // signedIn) failed, most likely offline. Resuming from `signIn` is the
      // only correct retry: a second POST /auth/login would mint a second
      // server session that lives to the refresh token's full TTL, for a
      // person who is already holding valid credentials on this device.
      setRetrying(true);
      // `resumeSignIn`, NOT `signIn`. `signIn` starts with `writeSession`, and
      // if the failed attempt already triggered a refresh, the disk holds a
      // NEWER pair than the one being held here — rewriting the old one makes
      // the next 401 present a spent refresh token, which the server reads as
      // reuse and answers by revoking every session on every device. See that
      // function's own comment for the full chain. Found in Task 2's review.
      resumeSignIn(pendingTokens)
        .catch((error: unknown) => {
          // `asApiError`, not a cast: `signIn`'s Keystore and MMKV steps reject
          // with real `Error`s, whose `message` a bare spread or cast loses,
          // and the banner then renders nothing at all.
          setFormError(asApiError(error).message);
        })
        .finally(() => setRetrying(false));
      return;
    }

    setErrors({});
    const parsed = loginSchema.safeParse({ email: email.trim(), password });
    if (!parsed.success) {
      setErrors(fieldErrorsOf(parsed.error));
      return;
    }
    login.mutate(parsed.data, {
      onError: (error) => {
        if (error.kind === "rateLimited") {
          countdown.start(error.retryAfterSeconds);
          return;
        }
        // A 422 with no envelope (a malformed body axum rejects before the
        // handler runs) arrives as `kind: "validation"` with `fields`
        // undefined — client-side parsing should make this unreachable, but
        // the fallback below still exists so a submit never silently does
        // nothing.
        if (error.kind === "validation" && error.fields) {
          setErrors(error.fields);
          return;
        }
        // The POST succeeded and only `signIn` failed. `mutation.data` cannot
        // detect this — query-core's reducer does `case "error": return
        // { ...state, data: void 0, ... }`, so it is always undefined
        // alongside an error, which made the first version of this fix dead
        // code. The pair rides the error instead; see `SignInFailure` in
        // api.ts. Holding it here turns the next tap into a resume rather
        // than a second POST, which would mint a duplicate server session.
        if (isSignInFailure(error)) {
          // The pair itself is derived from `login.error` above; this branch
          // only picks the copy.
          setFormError(SIGN_IN_INTERRUPTED);
          return;
        }
        // Every other failure gets one line. The 401 branch is not derived
        // from `error.message` on purpose — see CREDENTIALS_REFUSED.
        setFormError(error.kind === "unauthorized" ? CREDENTIALS_REFUSED : error.message);
      },
    });
  };

  const buttonLabel = waiting
    ? `Coba lagi dalam ${formatCountdown(countdown.remaining)}`
    : pendingTokens !== null
      ? "Lanjutkan"
      : "Masuk";

  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === "ios" ? "padding" : undefined}
      style={{ flex: 1 }}
    >
      <ScrollView
        keyboardShouldPersistTaps="handled"
        contentContainerStyle={{
          // `flexGrow: 1` so the footer's `marginTop: "auto"` has room to push
          // against. Without it the container is only as tall as its content
          // and the link sits directly under the button with a screen of dead
          // space below — which is how this screen looked before.
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
            Masuk ke garasi kamu.
          </Text>
        </View>

        {/* No card. Its border is `#29313A` on a `#0F141A` ground — barely
            visible, and it indented the fields away from the page gutter for
            no structure in return. The fields carry their own borders; the
            screen reads calmer without a frame around the only thing on it. */}
        <View style={{ gap: theme.space[4] }}>
          <AmTextField
            label="Email"
            value={email}
            onChangeText={setEmail}
            placeholder="nama@email.com"
            error={errors.email}
            keyboardType="email-address"
            autoCapitalize="none"
            autoCorrect={false}
            autoComplete="email"
            textContentType="emailAddress"
          />
          <AmTextField
            label="Kata sandi"
            value={password}
            onChangeText={setPassword}
            error={errors.password}
            secureTextEntry
            autoCapitalize="none"
            autoCorrect={false}
            autoComplete="current-password"
            textContentType="password"
          />
          {formError ? (
            <Text
              accessibilityLiveRegion="polite"
              accessibilityRole="alert"
              style={[theme.type.caption, { color: theme.color.semanticText.danger }]}
            >
              {formError}
            </Text>
          ) : null}
          {waiting ? (
            <Text
              accessibilityLiveRegion="polite"
              style={[theme.type.caption, { color: theme.color.semanticText.warning }]}
            >
              Terlalu banyak percobaan. Coba lagi dalam {formatCountdown(countdown.remaining)}.
            </Text>
          ) : null}
          <AmButton
            // `accent`, not the graphite default. The design system reserves
            // orange for "the strongest brand CTA, used selectively" — and a
            // screen whose entire job is one action is exactly that. It also
            // fixes a first impression: on an empty form the primary button
            // renders disabled, and in graphite it read as a dead control.
            variant="accent"
            label={buttonLabel}
            onPress={submit}
            size="lg"
            loading={busy}
            disabled={waiting}
          />
        </View>

        {/* `marginTop: "auto"` pins this to the bottom on a tall screen and
            lets it flow right after the form on a short one or at a large font
            size. The thumb zone is where a secondary escape belongs. */}
        <View style={{ marginTop: "auto", gap: theme.space[1], alignItems: "center" }}>
          <Text style={[theme.type.body, { color: theme.color.textSecondary }]}>
            Belum punya akun?
          </Text>
          {/* Vertical padding, not hitSlop: the label type is 13px/18px, so the
              tap target is ~18pt against this plan's 44pt floor. The Am*
              primitives apply minHeight after the caller's style so it cannot be
              defeated, but a bare Link is not one of them. expo-router's Link
              does NOT accept hitSlop (not on LinkProps); padding on the
              underlying Text grows the touchable region instead. space[4] is 16,
              so 18 + 32 = 50. Found in Task 2's review. */}
          <Link
            href="/register"
            style={[
              theme.type.label,
              { color: theme.color.accentText, paddingVertical: theme.space[4] },
            ]}
          >
            Daftar sekarang
          </Link>
        </View>
      </ScrollView>
    </KeyboardAvoidingView>
  );
}
