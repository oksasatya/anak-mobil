# @anakmobil/mobile

The AnakMobil.id mobile app — React Native via Expo, in the repository's Bun workspace. This is the **bootstrap** (AM-14): it installs, type/lint-gates, runs on a real device against a per-profile API URL, and has its own CI job. There is no design system, no navigation, and no API client yet — each is its own story.

## Stack

- **Expo SDK 57** · **Expo Router** (file-based, typed routes) · React Native 0.86 / React 19
- **Custom dev client** from the first build (not Expo Go) — `expo-dev-client`
- **TypeScript strict**, no explicit `any`
- **Bun** (workspace member; the repo has one root `bun.lock`)
- **EAS** configured (`eas.json`) for cloud builds — not required for local development

## Prerequisites

- **Xcode** (iOS) and/or **Android Studio** (Android)
- **Node LTS** on `PATH` — Bun runs everything else, but `expo prebuild` shells to `npm pack` internally
- For a **physical iPhone**: an Apple signing identity (a free personal team works — the device is registered, the app re-signed ~weekly on the free 7-day certificate, and the developer certificate trusted once under Settings). The iOS **simulator** needs none of this; Android debug builds self-sign.
- The API running and reachable — `make be-web` (binds `0.0.0.0:8080`)

## Commands

All `make` targets run from the **repository root**.

```bash
make mb-check                 # gate: prettier --check → typed routes → tsc --noEmit → expo lint
make mb-run-dev p=ios         # build+install the dev client on a device/simulator (p=ios|android)
make mb-run-preview p=ios     # the preview profile
make mb-run-prod p=ios        # the production profile
make mb-reverse               # bridge localhost:8080 into an attached Android device
```

**Two steps, and the split is the point.** `mb-run-*` performs a **native build** — Gradle or Xcode, minutes on a first run — and only has to happen once, then again when a native dependency changes. Everything after that is JavaScript, served by Metro.

So the daily loop is the repo-wide dev command, which starts the API, the landing site, and Metro together:

```bash
make dev              # API + landing + Metro + the app on the iOS simulator
make dev m=none       # …without opening the app, for landing-only work
```

**iOS is what `make dev` opens; Android is a manual path.** Android still has to work — AM-24 asks for the app running on a physical device of each platform — but booting an emulator on every `make dev` costs more than it returns when the day's work is iOS. To run Android: `make mb-run-dev p=android`, start the device, then `make mb-reverse`.

`make dev` opens an **already-installed** dev client and never builds one. Run `make mb-run-dev p=ios` once first, or there is nothing to open — the command says so rather than failing silently.

Metro's interactive keys (`i`, `a`, `r`) do **not** work under `make dev` — its output is piped through `awk` for the log prefix, which costs the TTY. That is exactly why `m=` exists: the platform is chosen at startup instead of by keypress. For the interactive Metro, run it alone:

```bash
bun run --filter @anakmobil/mobile start
```

## Environment profiles

The app carries exactly one build-time value: **`EXPO_PUBLIC_API_URL`** — the API base URL. It is the only variable prefixed `EXPO_PUBLIC_`, because that prefix is the only thing Expo inlines into the bundle. Nothing secret is ever put here; a secret would never be given that prefix, and anything the app must not reveal is fetched from the API against the user's own session, never compiled in.

```mermaid
flowchart LR
  V["APP_VARIANT<br/>(dev / preview / prod)"] --> C["app.config.ts"]
  E[".env.&lt;variant&gt;<br/>(local)"] --> C
  X["EAS Environments<br/>(cloud builds)"] --> C
  C --> ID["app id + name<br/>id.anakmobil.app[.dev|.preview]"]
  C --> U["EXPO_PUBLIC_API_URL<br/>→ inlined into the bundle"]
  U --> S["healthcheck screen<br/>shows profile + URL + /healthz"]
```

- **`.env.development`** is committed and points at `http://localhost:8080`. `.env.preview` / `.env.production` are the developer's own, or pulled with `eas env:pull`, and stay gitignored.
- **`localhost` is not the same word on every target**, and this is the one thing worth reading twice:

  | Target | Does `localhost:8080` reach the API? |
  |---|---|
  | iOS simulator | Yes — it shares the host's network stack |
  | Android emulator | Only with `adb reverse` — otherwise `localhost` is the emulator |
  | Android phone over USB | Only with `adb reverse` — same reason |
  | **Physical iPhone** | **No.** There is no adb; point the URL at the Mac's LAN IP |

  Run `make mb-reverse` once the Android device or emulator is up, and again after any reconnect or emulator restart. For a physical iPhone, set `EXPO_PUBLIC_API_URL` to `http://<lan-ip>:8080` (`ipconfig getifaddr en0`) — the API binds `0.0.0.0:8080`, so it is reachable.
- The `mb-run-<variant>` targets set `APP_VARIANT` and source the matching env file so the right URL is inlined; the healthcheck screen (`src/app/index.tsx`) prints the resolved profile and URL so a wrong one is obvious on first launch.

## Structure

```
app.config.ts        dynamic config — APP_VARIANT → app id/name, env file, EAS projectId
eas.json             development / preview / production build profiles
src/app/_layout.tsx  root layout (bare Stack — no navigation yet)
src/app/index.tsx    healthcheck screen
.env.development      committed example (LAN IP)
```

CI: `.github/workflows/mobile.yml` runs the same checks (format-clean + type-clean + lint-clean) on every change under `apps/mobile/`. It does not build a native binary — that is local (`expo run:*`) or on-demand in the cloud (`eas build`).
