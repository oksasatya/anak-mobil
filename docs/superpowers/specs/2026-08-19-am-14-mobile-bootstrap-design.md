# AM-14 — Mobile project bootstrap and build pipeline

Ticket: [AM-14](https://oksasatyaa.atlassian.net/browse/AM-14) · Epic: mobile · design approved 2026-08-19

`apps/mobile` is a line in the repository layout and nothing else — "not scaffolded yet". Every mobile story after this one assumes a project that installs, runs on a real phone, fails loudly on bad code, and knows which API it is talking to. This ticket builds exactly that foundation and stops there: no design system, no navigation, no API client. Those are named out of scope on the ticket and stay out of scope here.

The reason to build it now, ahead of more backend, is that the API has no real consumer. Its response envelope, its error codes, its auth cookie flow — all of it is asserted by integration tests and by nothing that behaves like a user. A mobile shell that can reach `/healthz` against the right base URL is the first thing that exercises the contract from the outside, which is where the contract's real gaps show.

This design was stress-tested twice before it was trusted: a cross-model adversarial pass, whose four changes are tagged **[revised]** inline, and a grill against this repository's own documents, whose changes are tagged **[grill]**. The tags let the next reader see the design survived something rather than being the first idea that sounded right.

## The stack, and why it is not Flutter or bare React Native

**React Native with Expo, Expo Router for file-based routing, a custom dev client from the first build.** This is on the ticket already; the open question the owner raised was Flutter versus React Native, and the answer is React Native for one reason that outweighs the rest here: the rest of this codebase is TypeScript and Rust, and the mobile app is meant to consume types generated from the same backend. **[grill]** That shared-type path is a future one — `packages/api-types` is "not scaffolded yet" per the repo layout, so it is a reason to keep the door open, not a benefit collectable today. What is true today is the reverse cost: Flutter is Dart — a second language, a second toolchain, and a hard wall between the app's models and the API's TypeScript — and that wall is present-tense. React Native keeps one type language end to end and lets the generated-types story arrive when its package does. Flutter's advantages (its own render pipeline, tighter control of custom animation) buy nothing a community-and-garage app needs that Expo's native views do not already give.

**Expo over bare React Native** because bare RN means owning the native build config — Xcode project files, Gradle, the CocoaPods lockfile — by hand, from day one, for a solo developer. Expo's prebuild generates all of it from `app.config.ts` and regenerates it when a dependency needs a native change, so the native projects are outputs, not hand-maintained source. This is the "continuous native generation" model, and it is the correct default unless a native requirement forces ejection — none exists here.

**A custom dev client, not Expo Go.** Expo Go is the sandbox app that only runs the SDK's own native modules; the moment this app needs a native module the sandbox does not bundle, Expo Go cannot load it. Building the dev client from the start means the development app is *this* app's real native binary with Metro pointed at it — there is never a migration off Expo Go later, and AC1's "runs on a physical device" means the real thing throughout.

## AC1 is a local native build, and the ticket's own wording hides a trap

AC1: clone, install, run the dev command, and the app opens on a physical Android and iOS device with no error.

**[revised] `expo start --dev-client` does not satisfy this on its own.** That command starts Metro — the JavaScript bundler — and nothing else. It assumes a dev client is *already installed* on the device. On a freshly cloned repo nothing is installed, so "run the dev command and it opens on the device" is false until a native build has happened first. Reading AC1 as a single command is the trap; it is two steps, and the first is a native build.

**The build is local, run by the owner:** `expo run:ios` on the Mac with Xcode, `expo run:android` with Android Studio. These compile the dev client and install it on a connected device or simulator; after that, `expo start --dev-client` is the day-to-day loop. No Expo account and no cloud service is required to build — this is the plainest path to "it runs on my phone" and it depends on nothing external.

**The division of labour is explicit because it has to be.** This scaffold produces a repository where those commands are ready to run. The build-and-install-on-a-physical-device step needs the owner's machine, the owner's Xcode/Android Studio, and the owner's connected phone — it cannot be performed or verified from here. AC1's proof is the owner running `expo run:ios` / `expo run:android` once and seeing the app open. Everything up to that command is what this ticket delivers and verifies.

**[grill] A physical device cannot reach `localhost`, and AC1 says physical device.** The API binds `0.0.0.0:8080` — reachable across the LAN — but `localhost`/`127.0.0.1` on the phone resolves to the phone itself, so a dev build pointed at `localhost` fails on a real device with a connection error that looks like a broken app. The development API URL is therefore the **dev machine's LAN IP** — `http://<machine-lan-ip>:8080` — not `localhost`; a simulator, which shares the host's loopback, may use `localhost`, a physical phone may not. Metro already surfaces the machine's LAN IP for the bundler connection; the API URL needs the same value, and the healthcheck screen showing the resolved URL is what makes a wrong one obvious immediately rather than as a mystery failure.

**Prerequisites on the dev machine**, stated so a fresh clone is not a mystery:

- **Xcode** (iOS) and **Android Studio** (Android).
- **[grill] An Apple signing identity for a physical iPhone.** `expo run:ios` onto a real device needs code signing — a free personal Apple team is enough, but the device must be registered, the app re-signed roughly weekly (the free tier's 7-day certificate), and the developer certificate trusted once on the device under Settings. The iOS *simulator* needs none of this; the physical-device requirement in AC1 is what pulls signing in. Android's local debug build self-signs and needs no account.
- **Node LTS** — see the Bun section for why Node is still required even though the repo runs on Bun.
- The API running and reachable at the LAN IP above (`make be-web`).

## Bun everywhere, with one hard-won caveat

The repository is a Bun workspace and `apps/mobile` joins it. Scaffold with `create-expo-app`, but **[revised] `--no-install`**: letting the scaffolder run its own install writes a nested `package-lock.json` or `yarn.lock` inside `apps/mobile`, which then resolves a different dependency tree than the root `bun.lock`. Scaffold without installing, delete any stray lockfile the template drops, then a single `bun install` from the repository root brings the new workspace into the one lockfile that CI enforces with `--frozen-lockfile`.

**[grill] Bun is the intent, not an assumption — the repository already wrote the fallback and told us to verify.** `CLAUDE.md` names `apps/mobile` as the one place Bun is expected to strain, because Metro and parts of the Expo/EAS CLI have historically assumed npm or yarn, and it pre-authorizes the exact remedy: *"`apps/mobile` alone uses a different package manager, not that the repository reverts."* So this scaffold **verifies Bun before committing to it** — `bun install` at the root, then `expo run:android`/`expo prebuild` and an `eas build --local` dry run must each work. If one breaks *specifically because the tool shells to npm/yarn and cannot be pointed at Bun*, `apps/mobile` alone falls back to Yarn (Expo/EAS's best-supported manager), with a nested `yarn.lock` scoped to that workspace and the root staying Bun. That is a per-workspace decision the repo already blessed, not a reversal — and it is recorded here so the plan treats "does Bun hold for mobile?" as a checkpoint with a known answer for "no", rather than a surprise.

**[revised] Node LTS is a required peer even on Bun.** `expo prebuild` shells out to `npm pack` internally to resolve and stage native template packages; that path assumes a Node/npm binary on `PATH`. Bun runs everything else — installs, scripts, Metro — but the prebuild step is the one place Node must exist. This is documented as a prerequisite rather than discovered when prebuild fails.

**[revised] Metro needs no manual monorepo configuration.** The old advice was to hand-write `watchFolders` and `nodeModulesPaths` in `metro.config.js` so Metro could see the workspace root. Current Expo SDKs auto-detect a Bun/Yarn/pnpm workspace and configure both. The project uses the default `expo/metro-config` untouched; a hand-rolled config here is obsolete maintenance surface that drifts from the SDK's own defaults.

**Pin the SDK, do not float it.** Scaffold against a pinned Expo SDK major (the current stable at scaffold time, recorded in the plan and in `package.json`), never `create-expo-app@latest` with a floating template — a floating scaffold is not reproducible, and two clones a month apart get different code. The exact pinned version is resolved in the plan, not guessed here.

## AC2 is one command that fails on file and line

AC2: a change that breaks lint or type-checking makes the verify command exit non-zero and name the file and line.

A `make mb-check` target, mirroring the repo's existing `make` wrappers, running three steps in order:

1. **Generate typed routes.** Expo Router derives a `.d.ts` of the route tree from the files in `app/`. **[revised] This must run before the type-check**, or `tsc` checks against stale or missing route types and the gate is checking the wrong thing. In CI the generation step comes first for the same reason.
2. **`tsc --noEmit`** against a `tsconfig.json` extending `expo/tsconfig.base` with `strict: true`. The ticket's "no `any` in domain types" is enforced by strictness plus an ESLint rule against explicit `any`, not by hope.
3. **ESLint** with the Expo config. Both `tsc` and ESLint report file and line on failure natively, which is AC2's actual requirement — no custom reporter is needed, only that the command chains them and propagates the first non-zero exit.

The same three steps run in CI (below), so "passes locally" and "passes in CI" mean the same thing.

## AC3 is per-profile env with nothing secret in the bundle

AC3: a build for development, preview, or production points at that profile's API base URL, and no secret is embedded in the app bundle.

**The app reads exactly one value, `EXPO_PUBLIC_API_URL`.** Expo inlines variables prefixed `EXPO_PUBLIC_` into the JavaScript bundle at build time and inlines nothing else. That prefix is the whole mechanism: the base URL is public by nature (it is in every network request the app makes), so it is the only thing that ever carries the prefix, and a secret cannot leak through this path because a secret would never be given an `EXPO_PUBLIC_` name. This is the repo's standing rule for mobile made concrete — nothing secret goes into a mobile environment at all; anything the app must not reveal is fetched from the API against the user's own session, never compiled in.

**Two supply paths, one contract**, because AC1 is a local build and cloud builds come later:

- **Local builds** (the AC1 path) read `EXPO_PUBLIC_API_URL` from a per-profile `.env` file — `.env.development`, `.env.preview`, `.env.production`. **[grill] The profile is selected explicitly, not left to Expo's build mode.** A local `expo run:*` defaults to the development variant, so "run it and the right env loads" only ever proves the development profile unless the profile is chosen deliberately. An `APP_VARIANT` environment variable (`development` | `preview` | `production`), read at the top of `app.config.ts`, selects which `.env.<variant>` file is loaded and drives the per-variant app id and display name; a Makefile target per profile (`mb-run-dev`, `mb-run-preview`, `mb-run-prod`) sets `APP_VARIANT` and runs the matching variant, so all three are provable on the local machine without a cloud build. Only `.env.development` is committed as an example (pointing at the LAN-IP dev API); the others are the developer's own, and `apps/mobile/.gitignore` keeps real env files out of git.
- **[revised] EAS Environments, not a build profile's `env` block**, for cloud builds. The trap the adversarial pass caught: `expo start` and a local run read `.env` files, but an EAS *build profile's* inline `env` does not flow to a local `expo start` — so "set it in `eas.json`" would appear to work in a cloud build and silently do nothing locally, giving a false AC3 proof. EAS Environments define the variable per environment (development/preview/production) and are pullable locally with `eas env:pull`, so the same variable resolves the same way in both worlds. Verify with `eas env:pull` followed by `expo config --type public`, which prints exactly what will be inlined.

**The proof is a screen, not a claim.** `app/index.tsx` is a healthcheck screen that renders the active profile and the resolved `EXPO_PUBLIC_API_URL`, then calls `/healthz` on it and shows the result. AC3 is demonstrated by opening the app on each profile and reading the URL off the screen — the same screen doubles as AC1's "it opens with no error" evidence and the first real exercise of the API contract from a client.

`eas.json` still defines the three build profiles (development/preview/production) so cloud builds are one command away when they are wanted; **[revised] `eas init`** runs once to create the EAS project and write its `projectId` into `app.config.ts`, without which no EAS command resolves. Configuring EAS now costs nothing and removes a setup step from every later mobile ticket; it does not make AC1 depend on the cloud.

## Layout

```
apps/mobile/
  app/
    index.tsx            healthcheck screen — active profile, resolved API URL, /healthz result
    _layout.tsx          root layout (Expo Router requires it; no navigation structure yet)
  app.config.ts          dynamic config — reads EXPO_PUBLIC_API_URL, holds eas projectId, per-profile app id/name
  eas.json               development / preview / production build profiles
  tsconfig.json          extends expo/tsconfig.base, strict: true
  .env.development       committed example → dev API at the machine LAN IP (http://<lan-ip>:8080), not localhost
  .gitignore             ignores .env.preview / .env.production and native build output (ios/, android/)
  package.json           @anakmobil/mobile, pinned Expo SDK
.github/workflows/mobile.yml    path-filtered: bun install --frozen-lockfile → generate routes → mb-check
Makefile                        + mb-check, mb-run-dev / mb-run-preview / mb-run-prod
```

**[grill] The `mb-` prefix is not arbitrary** — the Makefile already namespaces by surface (`be-*` backend, `fe-*` landing, `ds-*` design tokens), so mobile is `mb-*`. `mb-check` joins the `check` aggregate (`check: be-check fe-check mb-check`) so "every gate in the repository" stays true; the aggregate installs the mobile workspace as part of a full `bun install`, which it already runs.

`ios/` and `android/` are prebuild outputs and stay gitignored — they are generated by `expo run:*`, not hand-maintained, and committing them would invite exactly the manual native drift Expo exists to avoid.

## CI

`.github/workflows/mobile.yml`, path-filtered to `apps/mobile/**` and the workflow file itself so a backend commit does not trigger it (the repo's stated reason for having no task runner — affected-only already exists in the path filters). It pins the same Bun version the other JS jobs use, runs `bun install --frozen-lockfile`, generates the router types, then runs `mb-check`. It does **not** build a native binary or an EAS build — CI proves the code is type-clean and lint-clean; the native build is local and the cloud build is on demand. Adding the job in this same change is deliberate: a workspace with no CI job has no gate, reads as passing forever, and the defect hides for months.

## Anti-goals

Named so a later reader can tell a deliberate absence from an oversight.

- **No design system, no theme wiring, no fonts.** `packages/tokens` is not consumed yet. The healthcheck screen is deliberately plain.
- **No navigation structure.** Expo Router is installed and its one required `_layout.tsx` exists; there are no tabs, no stack, no routes beyond the healthcheck index. Navigation is its own story.
- **No API client.** The healthcheck screen calls `/healthz` with a bare `fetch`. No client abstraction, no auth, no interceptors, no generated types wired in — those arrive with the stories that need them.
- **No EAS cloud build in the AC1 path.** `eas.json` is configured, but AC1 is proven by a local `expo run:*`. Cloud builds are available, not required.
- **No app store submission, no signing config beyond what a local dev build needs**, no push notifications, no OTA update channel.
- **No authentication.** The app reaches only the public `/healthz`.

## Verification

- **AC1** — owner runs `expo run:ios` and `expo run:android` once; the app opens on device/simulator and the healthcheck screen renders. (Owner-executed; cannot be verified from the development environment.)
- **AC2** — introduce a deliberate type error and a lint error; `make mb-check` exits non-zero and names the file and line for each. Revert.
- **AC3** — open the app under each profile; the screen shows that profile's `EXPO_PUBLIC_API_URL`. `expo config --type public` confirms no non-`EXPO_PUBLIC_` value is inlined.
- **Gate** — `bun install --frozen-lockfile` from the root leaves `bun.lock` unchanged (no nested lockfile drift); the CI job is green on the branch.
