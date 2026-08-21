import type { ExpoConfig } from "expo/config";

// APP_VARIANT selects the app identity (id, name). It is a plain build-time var,
// never inlined into the bundle, set by `make mb-run-<variant>` and by each EAS
// build profile's `env`. A bare `expo run:*`/`expo start` defaults to development.
//
// The bundle's one public value, EXPO_PUBLIC_API_URL, is NOT selected here.
// Babel inlines it from process.env at bundle time, and process.env is populated
// by (a) `@expo/env`, which auto-loads `.env.<NODE_ENV>` — development or
// production, never our `preview` — and (b) the shell, which the mb-run-<variant>
// targets set by sourcing `.env.<variant>` BEFORE Expo starts (`@expo/env` does
// not override an already-set var, so the sourced value wins). Loading the env
// file here too would not change what babel inlines — it would only make
// `expo config` report a value the bundle does not carry — so it is deliberately
// not done. The authoritative per-profile path is `make mb-run-<variant>`.
type Variant = "development" | "preview" | "production";
const VARIANT = (process.env.APP_VARIANT as Variant) ?? "development";

const APP_ID: Record<Variant, string> = {
  development: "id.anakmobil.app.dev",
  preview: "id.anakmobil.app.preview",
  production: "id.anakmobil.app",
};

const APP_NAME: Record<Variant, string> = {
  development: "AnakMobil (Dev)",
  preview: "AnakMobil (Preview)",
  production: "AnakMobil",
};

const config: ExpoConfig = {
  name: APP_NAME[VARIANT],
  slug: "anakmobil",
  scheme: "anakmobil",
  version: "0.1.0",
  orientation: "portrait",
  icon: "./assets/images/icon.png",
  userInterfaceStyle: "automatic",
  ios: {
    bundleIdentifier: APP_ID[VARIANT],
    // No `icon` override here on purpose: it used to point at
    // `./assets/expo.icon`, which is Expo's OWN default icon bundle — literally
    // `expo-symbol.svg` on Expo's blue automatic gradient — so the app shipped
    // with the stock Expo logo on the home screen. Falling through to the root
    // `icon` uses the real brand mark instead.
  },
  android: {
    package: APP_ID[VARIANT],
    adaptiveIcon: {
      // brand.800. The adaptive foreground/background images below are derived
      // from the real brand mark; this is the fallback fill behind them.
      backgroundColor: "#1D232A",
      foregroundImage: "./assets/images/android-icon-foreground.png",
      backgroundImage: "./assets/images/android-icon-background.png",
      monochromeImage: "./assets/images/android-icon-monochrome.png",
    },
    predictiveBackGestureEnabled: false,
  },
  web: {
    output: "static",
    favicon: "./assets/images/favicon.png",
  },
  plugins: [
    "expo-router",
    [
      "expo-splash-screen",
      {
        backgroundColor: "#0F141A",
        // The REAL brand mark, not `splash-icon.png` — that file is a leftover
        // chevron placeholder and is not the AnakMobil logo at all. This is a
        // build-time copy of `@anakmobil/assets/img/favicon-dark.png` (the
        // dark-ground variant, matching the background above): an Expo config
        // plugin resolves asset paths inside the app directory, so it cannot
        // import from the workspace package the way `AmBrandLockup` does.
        // Transparent — the tile is stripped, so the mark floats on the
        // background colour above instead of sitting in a dark square. Same
        // file and same 88pt width the in-app launch view uses, so the mark
        // does not move or change when one hands over to the other.
        image: "./assets/images/brand-mark.png",
        imageWidth: 88,
      },
    ],
    "expo-secure-store",
  ],
  experiments: {
    typedRoutes: true,
    reactCompiler: true,
  },
  extra: {
    appVariant: VARIANT,
    // Written by `eas init` in Task 5. Until then no EAS command resolves,
    // which is fine — AC1 needs no EAS.
    eas: { projectId: "REPLACE_AFTER_EAS_INIT" },
  },
};

export default config;
