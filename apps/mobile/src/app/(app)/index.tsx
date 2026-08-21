import { Redirect } from "expo-router";

/**
 * `/(app)` lands on Beranda.
 *
 * This has to be a real route, not an absence, because `/(app)` is what every
 * entry path targets: `AuthGate` after sign-in, `OnboardingGate` on
 * completion, and `app/index.tsx` on every cold start with a live session.
 *
 * `unstable_settings = { anchor: "home" }` on the layout does NOT do this job,
 * which is the defect this file fixes. The anchor becomes the navigator's
 * `initialRouteName`, and expo-router *prepends* that route while keeping the
 * matched one focused — `/(app)` resolved to `{ index: 1, routes: [home,
 * index] }`, so the first screen after signing in was the AM-14 healthcheck,
 * with its API base URL on display and no tab highlighted (the healthcheck's
 * `tabBarButton` is null). Beranda was one tap away and never what opened.
 * Found in Task 2's review.
 */
export default function AppIndex() {
  return <Redirect href="/home" />;
}
