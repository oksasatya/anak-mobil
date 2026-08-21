import { Redirect } from "expo-router";

/**
 * The group's entry route sends people straight to the sign-in screen.
 *
 * There used to be a welcome screen here — a wordmark, a value proposition,
 * and "Daftar" / "Masuk" buttons. It was removed on the owner's call, and the
 * reason is worth keeping: it asked for a tap without answering anything. A
 * person opening the app either has an account or does not, and both need the
 * same next screen — sign in, with a register link on it. The value
 * proposition was also the least honest surface in the app, since it described
 * features that are not built yet.
 *
 * A `Redirect` rather than putting the sign-in screen at this path: it keeps
 * `/login` and `/register` as real, deep-linkable routes with their own names,
 * and it costs one frame that the router never paints.
 */
export default function AuthIndex() {
  return <Redirect href="/login" />;
}
