# AnakMobil.id — Mobile Feature Breakdown (React Native)

**Source:** `prd.md` v1.0 (14 Aug 2026)
**Scope of this document:** the mobile application only (Android + iOS), plus the backend surface the mobile app requires.
**Platform decision:** React Native, overriding the PRD's Flutter recommendation (§6.1).
**Status:** Draft for review.

---

## 0. Decisions taken in this breakdown

| # | Decision | Rationale |
|---|---|---|
| D-1 | **React Native + Expo (managed workflow with a custom dev client)** instead of Flutter | The team's existing strength is TypeScript/React. One language across mobile, the Astro landing page, and the Vite+React backoffice means shared types, shared validation schemas (zod), a shared generated API client, and one mental model. Expo removes the Xcode/Gradle maintenance tax; the custom dev client keeps native modules available when needed (camera, notifications, MMKV, Sentry). |
| D-2 | **Expo Router** for navigation | Universal/deep links are a product requirement, not a nice-to-have: the public web uses `/@username`, `/build/:slug`, `/c/:slug`, and those links must open the app. File-based routing gives that mapping declaratively instead of a hand-maintained linking config. |
| D-3 | **TanStack Query** as the server-state layer; **Zustand** only for genuine client state (active vehicle, session, draft forms) | Almost all state in this app is server state. A global store holding server data would mean re-implementing caching, invalidation, and refetch by hand. |
| D-4 | **No offline write queue in v1.** Read-cache only. | Offline mutation with conflict resolution is weeks of work for a use case (logging a service record in a basement parking garage) that a retry banner covers. Revisit if telemetry shows real failure rates. |
| D-5 | **The vehicle catalog is seed data, not user data** | Brand/model/generation/variant must be curated and shipped before launch. This is a content acquisition project running in parallel with engineering — see §8.1. |
| D-6 | **AI responses are streamed and structured** | The PRD (§23) specifies a structured response envelope. Streaming plain text and parsing JSON afterwards gives the worst of both. Recommendation: stream the prose `answer` token-by-token over SSE, then deliver `evidence`, `confidence`, `warnings`, `recommendations` as a final structured frame the UI renders as cards. |

---

## 1. Mobile technical architecture

### 1.1 Dependency set

| Concern | Choice | Note |
|---|---|---|
| Runtime | Expo SDK (latest stable) + EAS Build/Update | Custom dev client from day one |
| Language | TypeScript, `strict: true` | No `any` in domain types |
| Navigation | `expo-router` | Typed routes enabled |
| Server state | `@tanstack/react-query` | With `persistQueryClient` → MMKV for read-cache |
| Client state | `zustand` | Session, active vehicle, composer drafts |
| Forms | `react-hook-form` + `zod` | Schemas shared with backend contract |
| Storage | `react-native-mmkv` (cache), `expo-secure-store` (tokens) | Tokens never in MMKV |
| Lists | `@shopify/flash-list` | Feeds, timelines, community, explore grid |
| Images | `expo-image` | Built-in disk/memory cache, blurhash placeholders |
| Media capture | `expo-image-picker`, `expo-camera`, `expo-av` (problem audio/video) | |
| Animation | `react-native-reanimated` + `react-native-gesture-handler` | Respect `prefers-reduced-motion` equivalent |
| Bottom sheets | `@gorhom/bottom-sheet` | Pickers, evidence detail, filters |
| Notifications | `expo-notifications` + FCM/APNs | Plus local notifications for service reminders |
| i18n | `i18next` + `expo-localization` | **id-ID is the default locale**, en-US secondary |
| Observability | `sentry-expo`, plus a product analytics SDK | Crash + funnel from day one |
| Auth (social) | `expo-auth-session` (Google), `expo-apple-authentication` | Apple Sign-In is **mandatory** on iOS once Google sign-in exists |

### 1.2 Project structure

```text
app/                        # expo-router routes only, thin
  (auth)/
  (tabs)/
    index.tsx               # Home
    garage/
    explore/
    community/
    profile/
  vehicle/[id]/
  build/[id]/
  problem/[id]/
  ai/
src/
  features/
    auth/                   # api, hooks, components, schemas per feature
    vehicle/
    build/
    service/
    problem/
    community/
    explore/
    ai/
    notifications/
  shared/
    api/                    # client, interceptors, error mapping
    ui/                     # design system primitives
    hooks/
    lib/
  types/                    # generated from backend OpenAPI
```

Rule: `app/` files contain routing and layout composition only. All logic lives under `src/features/*`. A screen file longer than ~150 lines is a refactor signal.

### 1.3 Design system prerequisites

Before the first product screen is built, these must exist:

**All decided — see `docs/design.md`.** Graphite `#1D232A` primary, AnakMobil Orange `#ED491C` accent, Inter, core radius 16px, 4px spacing base, semantic colours separate from the accent. Target ratio 85% neutral / 10% orange / 5% semantic.

- Color tokens (light + dark; **dark is the primary theme** for this audience) — semantic names, not raw hex, at call sites.
- Type scale and the Inter stack, with tabular numerals for spec data (`18×8.5 ET40`, `225/40 R18`, `146,120 KM`).
- Spacing scale, radius scale, elevation. Borders before shadows.
- Primitives: `Button`, `Input`, `Select`, `Sheet`, `Card`, `Chip`, `Badge`, `Avatar`, `EmptyState`, `ErrorState`, `Skeleton`, `Toast`.
- Minimum touch target 44×44pt enforced in `Button`/`IconButton` by default.

---

## 2. Navigation map

```text
Tab bar: Home | Garage | Explore | Community | Profile
Global FAB (+): Add modification · Add service · Add problem · Add photo
Persistent AI entry: header action on Home + Vehicle + contextual "Tanya AI" buttons

Home
Garage
  └ Vehicle Detail
      ├ Build tab       → Modification Detail · Add/Edit Modification · Build Timeline
      ├ Service tab     → Service Detail · Add/Edit Service · Reminders
      ├ Problems tab    → Problem Detail · Create Problem · Resolve Problem
      └ Photos / Specs  → Edit Vehicle · Photo Viewer
Explore
  ├ Build Feed (filtered) → Build Detail (other user) → Public Profile
  └ Part Detail
Community
  ├ Discover → Community Detail → Post Detail → Composer
  └ Members
Profile
  ├ My public profile
  └ Settings → Account · Privacy · Notifications · AI usage · Legal · Delete account
AI (modal / dedicated stack)
  ├ Chat (context-bound to a vehicle)
  ├ Conversation list
  └ Evidence sheet
Notification center (from header)
```

**Screen inventory: ~58 distinct screens/sheets for the PRD's stated v1 scope.** That number is the single most important input to the scope discussion in §7.

---

## 3. Epic breakdown

Complexity is expressed in engineering-days for one experienced full-stack developer, mobile side only, excluding backend. `S` = 1–2d, `M` = 3–5d, `L` = 6–10d, `XL` = 10d+.

### E0 — Foundation & app shell

| # | Item | Detail | Size |
|---|---|---|---|
| E0-1 | Project bootstrap | Expo + TS strict, ESLint/Prettier, absolute imports, EAS profiles (dev/preview/prod), env config via `expo-constants` | S |
| E0-2 | Design system | Tokens, theme provider (dark default), primitives listed in §1.3 | L |
| E0-3 | Navigation shell | Tab bar, stack layouts, global FAB with action sheet, header patterns | M |
| E0-4 | API client | Fetch wrapper, auth header injection, refresh-token retry, typed error mapping, request-id propagation | M |
| E0-5 | State foundation | Query client + MMKV persistence, Zustand session store, **active-vehicle context** (cross-cutting; nearly every screen reads it) | M |
| E0-6 | App states | Global error boundary, offline banner, force-update gate, maintenance mode | S |
| E0-7 | Observability | Sentry, analytics events schema, screen tracking | S |
| E0-8 | Deep links | Universal links (iOS) + App Links (Android), route mapping for `/@user`, `/build/:slug`, `/c/:slug`, `/problems/:slug` | M |

**Acceptance:** app builds to a physical device via EAS, cold start under 3s, a deep link opens the correct route while logged out and resumes it after login.

---

### E1 — Authentication & onboarding

**User stories**
- As a new user I can register with email and password so I can own a garage.
- As a returning user I can log in with Google or Apple so I skip typing a password.
- As a new user I am taken straight into adding my first car, because an empty account has no value.

| # | Screen / flow | States & rules | Size |
|---|---|---|---|
| E1-1 | Welcome | Value proposition, "Masuk" / "Daftar", guest-preview link | S |
| E1-2 | Register | Email, password (min 8, strength meter), username availability check (debounced, live), ToS consent | M |
| E1-3 | Login | Email/password, error differentiation (wrong password vs unknown account — **do not leak account existence**), rate-limit feedback | S |
| E1-4 | Social auth | Google + Apple. Apple is mandatory on iOS. Account-linking rule when an email already exists | M |
| E1-5 | Email verification | Deep link back into app, resend with cooldown | S |
| E1-6 | Forgot / reset password | Token expiry handling | S |
| E1-7 | Onboarding: profile | Display name, avatar (optional), username confirm | S |
| E1-8 | Onboarding: add first car | Handoff to E2-1; **cannot be skipped** — skipping produces an empty product | — |
| E1-9 | Aha screen | "Your Honda Civic FD — 837 builds · 184 known issues · 421 parts · 7 communities" + primary CTA "Tanya AnakMobil AI" | M |
| E1-11 | **Account roles: admin and user** | Exactly two platform roles. Self-registration always yields `user`; `admin` is granted by migration or an operational command only. Enforced server-side on every admin request, read fresh so revocation is immediate. The mobile app is identical for both roles. Every admin action is written to an append-only audit trail. **Platform role is a different axis from community membership role** (owner/admin/member) — never the same column | M |
| E1-10 | **Biometric quick unlock** | Face ID / Touch ID / fingerprint. Unlocks a session stored on the device — **not** a second authentication factor against the server. Refresh token held in Keychain/Keystore behind a biometric requirement, bound to the current biometric set so enrolling a new face or finger revokes it automatically. Password fallback always available; disabling it, and logging out, must wipe the stored credential | M |

**Critical note on E1-9:** this screen is the activation moment in the PRD (§7.1) and it *cannot render honestly at launch* — on day one those counters are zero for almost every vehicle. See §8.2.

**Acceptance:** register → verify → add car → aha screen completes in under 90 seconds with no dead ends; session survives app restart; logout clears secure store and query cache.

---

### E2 — Vehicle & My Garage

**User stories**
- As an owner I can add a car by picking brand → model → generation → year → variant.
- As an owner with several cars I can switch which one is "active" everywhere in the app.
- As an owner I can record my plate and VIN without them being visible to anyone else.

| # | Item | Detail | Size |
|---|---|---|---|
| E2-1 | Add-vehicle wizard | 6 steps, back-navigable, progress indicator, draft persisted across app kill. Each step is a searchable picker sheet fed by the catalog API with an offline cache | L |
| E2-2 | Catalog pickers | Search-as-you-type, recent/popular first, "model saya tidak ada" escape hatch → free-text submission queued for moderation | M |
| E2-3 | Garage list | Multi-vehicle cards (photo, name, year, mileage, build value if visible), reorder, set active, add another | M |
| E2-4 | Vehicle detail | Hero photo, identity header, spec grid, tab bar (Build / Service / Problems / Photos), quick actions, "Tanya AI tentang mobil ini" | L |
| E2-5 | Edit vehicle | All optional fields (engine, engine code, transmission, color, drive type, power, torque, purchase date) | M |
| E2-6 | Private fields | Plate, VIN, purchase price. Masked by default, revealed by explicit tap, **never included in any public payload**, excluded from share sheets and screenshots of the public preview | M |
| E2-7 | Mileage log | Odometer entries with date. Powers "current mileage", service intervals, and the AI maintenance assistant. Validation: mileage cannot decrease (warn, allow with confirmation for odometer swaps) | M |
| E2-8 | Photos | Multi-select upload, cover selection, reorder, delete, full-screen viewer with pinch-zoom | M |
| E2-9 | Delete vehicle | Consequences explained (build, service, problems), typed confirmation, soft delete server-side | S |

**Acceptance:** a user can add a second vehicle and every screen (Home, AI context, FAB actions, Explore defaults) follows the active-vehicle switch with no stale data.

---

### E3 — Build & modification

**User stories**
- As an owner I can record a modification with the specs that actually matter for its category.
- As an owner I can see my build as a timeline and know what it cost me.
- As an owner I control who sees my costs.

| # | Item | Detail | Size |
|---|---|---|---|
| E3-1 | Category picker | 15 categories from PRD §10 with icons | S |
| E3-2 | **Dynamic mod form** | Fields vary by category — this is the core of the structured-data promise. Wheels: diameter, width, offset (ET), PCD, center bore. Tyres: width, aspect ratio, rim diameter, compound. Suspension: type (coilover/lowering spring/air), brand, spring rate. Others: brand, product, free-form spec. Common to all: install date, mileage at install, cost, garage, notes, photos | L |
| E3-3 | Part search | Autocomplete against the parts DB; free-text fallback creates a pending part record. **Without the fallback, contribution dies at the first missing part; without moderation, the parts DB fills with duplicates.** Both are required | M |
| E3-4 | Build summary | Grouped by category, current setup at a glance, empty state that teaches | M |
| E3-5 | Build timeline | Chronological, grouped by month, cost per entry, running total | M |
| E3-6 | Cost visibility | Per-vehicle setting: private / community / public. Applied server-side, not just hidden in the client | S |
| E3-7 | Edit / remove mod | Removing sets `removed_at` — history is preserved and still counts as fitment evidence with a "no longer installed" flag | M |
| E3-8 | Share build | Native share sheet → public web URL, OG preview | S |

**Acceptance:** a wheel modification records offset and PCD as structured numbers queryable by the fitment engine, not as a text blob.

---

### E4 — Service history

| # | Item | Detail | Size |
|---|---|---|---|
| E4-1 | Add service record | Date, mileage, category (taxonomy: engine oil, transmission oil, brakes, suspension, tune-up, air conditioning, electrical, bodywork, other), parts replaced (multi), garage, cost, notes, invoice photo | M |
| E4-2 | Service timeline | Grouped by year, mileage markers, cost column | M |
| E4-3 | Service detail | Full record, invoice viewer, edit, delete | S |
| E4-4 | Next-service rules | Per record: next date and/or next mileage. Derived "upcoming service" card on Home | M |
| E4-5 | Reminders | Local notification for date-based; push for mileage-based (server needs a mileage-update trigger). Snooze, mark done → prefills a new service record | M |

**Acceptance:** logging an oil change at 140,200 km with a 5,000 km interval surfaces a reminder on Home once the odometer log passes 145,000 km.

---

### E5 — Problem & repair knowledge

This epic produces the data that makes the AI worth anything. Treat it as a first-class capture flow, not a support ticket form.

| # | Item | Detail | Size |
|---|---|---|---|
| E5-1 | Create problem | Title, description, vehicle (defaults to active), mileage, symptom tags (when it happens: cold start / turning / braking / over 80 km/h / idle), error code (OBD), media: photo, **audio** (engine noise — genuinely useful and rarely offered), video | L |
| E5-2 | Problem status machine | `open → diagnosing → solved` and `open → closed_unresolved`. Status changes are timestamped | S |
| E5-3 | Resolve flow | Diagnosis, final solution, parts changed, garage, cost, "would you recommend this garage" | M |
| E5-4 | Problem detail | Timeline of updates, community replies, similar-case rail, "punya masalah sama" reaction that increments an occurrence counter | M |
| E5-5 | Similar problems | Same generation first, then same model, then same brand. Ranked by symptom-tag overlap | M |
| E5-6 | My problems list | Filter by status and vehicle | S |
| E5-7 | Nudge to resolve | An open problem older than 14 days prompts "sudah beres? bantu owner lain" — this is the mechanism that actually populates the knowledge base | S |

**Acceptance:** a solved problem yields a structured record with diagnosis, solution, parts, cost, and garage that the retrieval layer can index.

---

### E6 — Community

| # | Item | Detail | Size |
|---|---|---|---|
| E6-1 | Discovery | Recommended by the user's vehicles (generation → model → brand → region), search, browse by type | M |
| E6-2 | Community detail | Header, description, member count, rules, join/leave, post list | M |
| E6-3 | Post types | Discussion, question, problem share, build share, garage recommendation. Composer adapts to type; sharing a problem or build embeds the structured object rather than copying text | L |
| E6-4 | Post detail + comments | Threaded one level, reactions, sort by newest/top | M |
| E6-5 | Members | List, roles (owner/admin/member) | S |
| E6-6 | Moderation, minimum viable | Report post/comment/user, block user, admin delete. **Not optional — an unmoderated Indonesian community app becomes a spam and jual-beli board within weeks** | M |
| E6-7 | Community notifications | Reply, mention, admin announcement | S |

Admin tooling beyond delete/pin is Phase 2 per PRD §42.

---

### E7 — Explore

| # | Item | Detail | Size |
|---|---|---|---|
| E7-1 | Explore home | Default scope = same generation as active vehicle. Sections: popular builds, recent builds, common setups, known issues | M |
| E7-2 | Build feed + filters | Filters: wheel diameter/width/offset range, suspension type, style tag, engine. Grid of build cards | L |
| E7-3 | Build detail (other user) | Read-only build, parts list with "used by N builds", owner link, save/bookmark | M |
| E7-4 | Part detail | Specs, used-by count, most common pairing (e.g. tyre size), builds using it, price range if available | M |
| E7-5 | Public user profile | Garage showcase, public builds, joined communities, solved-problem count | M |

---

### E8 — AnakMobil AI

The largest and highest-risk epic on both client and server.

| # | Item | Detail | Size |
|---|---|---|---|
| E8-1 | Chat screen | Streaming message list, composer, stop-generation, retry, copy | L |
| E8-2 | **Vehicle context chip** | Always-visible indicator of which car the question is about, tappable to switch. Without it, every answer is ambiguous in a multi-car garage | M |
| E8-3 | Suggested prompts | Contextual per entry point: from a problem page → "apa kemungkinan penyebabnya?"; from a build page → "upgrade apa selanjutnya?" | S |
| E8-4 | Structured response rendering | `answer` prose + confidence badge (Verified / High / Medium / Low / Insufficient data) + evidence cards (builds, problems, parts, garages, service records) + warnings + action buttons | L |
| E8-5 | Evidence sheet | Tapping an evidence card opens the real object — this is what keeps the AI a router to community content rather than a replacement for it (PRD §21) | M |
| E8-6 | Entry points | Home, vehicle, build, problem, part. Each seeds the conversation with its object as context | M |
| E8-7 | Conversation history | List, resume, rename, delete | M |
| E8-8 | Quota | 20 questions/month free. Usage meter, warning at 80%, graceful exhausted state that does **not** feel like a broken app | M |
| E8-9 | Feedback loop | Helpful / not helpful, regenerate, correction capture. Feeds PRD §55 metrics | S |
| E8-10 | Safety presentation | Safety-critical categories (brake, steering, structural, fuel leak, electrical, overheat) render a prominent inspection warning and suppress confident phrasing | M |
| E8-11 | Failure states | Timeout, no evidence found, model error, offline. "Insufficient data" must be a designed answer, not an error toast | M |

**Acceptance:** asking "velg yang cocok buat mobil gue apa?" with no further input returns an answer that names the user's actual car, cites at least one real evidence object, and states a confidence level.

---

### E9 — Home

| # | Item | Detail | Size |
|---|---|---|---|
| E9-1 | Contextual header | Time-based greeting, active vehicle card with photo and mileage | S |
| E9-2 | AI prompt entry | Prominent, above the fold | S |
| E9-3 | "For your car" sections | Known issues, popular builds, community updates, upcoming service, events (Phase 2) | L |
| E9-4 | Empty/cold states | A new user with a rare car sees an almost empty Home. Each section needs a designed low-data state that offers a contribution action instead of a blank card | M |

---

### E10 — Profile, settings, compliance

| # | Item | Detail | Size |
|---|---|---|---|
| E10-1 | My profile | Public preview, garage showcase, stats | M |
| E10-2 | Edit profile | Name, username, bio, avatar, location (city-level only) | S |
| E10-3 | Privacy settings | Cost visibility default, profile visibility, garage visibility per vehicle | M |
| E10-4 | Notification settings | Per-category toggles, quiet hours | S |
| E10-5 | AI usage | Quota consumed, history | S |
| E10-6 | Legal | ToS, privacy policy, licenses | S |
| E10-7 | **Account deletion** | In-app, self-service. Required by both App Store and Play Store when in-app registration exists. Export-before-delete is a nice-to-have | M |

---

### E11 — Notifications

| # | Item | Detail | Size |
|---|---|---|---|
| E11-1 | Push registration | Token lifecycle, permission priming screen before the OS prompt (asking cold burns the one chance you get) | M |
| E11-2 | Notification center | In-app list, read/unread, deep link per type | M |
| E11-3 | Categories v1 | Community activity, comment/reply, build interaction, service reminder, AI-generated reminder | M |

---

### E12 — Cross-cutting

| # | Item | Detail | Size |
|---|---|---|---|
| E12-1 | **Media pipeline** | Pick → client-side resize/compress → presigned upload → progress → retry → attach. Used by vehicle photos, mod photos, service invoices, problem media, avatars, post images. Build once, properly | L |
| E12-2 | i18n | id-ID default, string extraction discipline from the first screen (retrofitting is 3× the work) | M |
| E12-3 | Empty / error / loading | Every list needs all three, designed. Skeletons over spinners | M |
| E12-4 | Accessibility | 44pt targets, labels on icon buttons, dynamic type tolerance, contrast AA | M |
| E12-5 | Performance | FlashList everywhere, image cache policy, screen-level code splitting, cold-start budget | M |
| E12-6 | Release engineering | EAS build + submit, OTA update channel, staged rollout, store listings, screenshots | M |

---

### E13 — Backoffice admin (web)

A third surface, not part of the mobile app. It exists because three flows already designed — catalog suggestions (E2-2), part suggestions (E3-3), and content reports (E6-6) — all push work into a queue that currently has nobody to work it.

| # | Item | Detail | Size |
|---|---|---|---|
| E13-1 | Admin-only login | Route guards plus server-side rejection. Hiding the menu is not access control. Shorter idle timeout than the mobile session | M |
| E13-2 | Dashboard shell | Layout, navigation, and the reusable dense-table component: sorting, filters, pagination, bulk selection, empty/loading/error. Home page shows pending counts per queue | L |
| E13-3 | Content moderation queue | Reports grouped per object with reporter counts; decisions are delete / suspend / dismiss, each closing every report on that object. **Soft delete with undo** — moderation is sometimes wrong, and permanent deletion makes that unfixable | L |
| E13-4 | Vehicle catalog curation | Work the suggestion queue; approving one repoints the suggester's own vehicle so they never re-enter data. Edit entries; entries in use cannot be deleted, only merged | L |
| E13-5 | Part curation | Approve suggested parts, merge duplicates without losing evidence counts, complete structured specs (PCD, offset, center bore) so the fitment engine gets numbers. Merge is undoable | L |
| E13-6 | User management | Search, suspend and restore with a reason. **Plate, VIN, and purchase price are excluded from admin responses entirely** — admin access is not a reason to break a privacy promise made to the user | M |
| E13-7 | AI usage and cost monitoring | Questions/day, unique users, real cost/day and per question, top consumers, and a filter for low-confidence or unhelpful answers. Reading user conversations is audit-logged and limited to flagged answers | M |

**Out of scope:** garage and parts-seller dashboards. Both are separate products for a later phase.

**Design note:** `docs/design.md` covers mobile and public web — card-based, vehicle photography. It does not cover dense data tables. Backoffice keeps the same colour, type, and radius tokens but needs its own density decisions.

---

## 4. Backend surface required by the mobile app

Grouped by module (PRD §48 modular monolith).

```text
auth        POST /register · /login · /refresh · /logout · /verify-email
            /forgot-password · /reset-password · /oauth/{google,apple}
users       GET|PATCH /me · GET /users/:username · /me/settings
            DELETE /me
catalog     GET /brands · /models?brand · /generations?model
            /variants?generation · /engines · POST /catalog-suggestions
vehicles    CRUD /vehicles · POST /vehicles/:id/photos
            POST /vehicles/:id/mileage · GET /vehicles/:id/stats
builds      GET /vehicles/:id/build · CRUD /modifications
            GET /builds/:slug (public) · GET /builds?filters (explore)
parts       GET /parts?q · GET /parts/:id · GET /parts/:id/evidence
            POST /parts/suggestions
services    CRUD /service-records · GET /vehicles/:id/upcoming-service
problems    CRUD /problems · POST /problems/:id/resolve
            GET /problems/similar?vehicle&symptoms · POST /problems/:id/me-too
communities GET /communities?recommended · CRUD membership
            CRUD /posts · /comments · /reactions · POST /reports
ai          POST /ai/conversations · POST /ai/messages (SSE stream)
            GET /ai/conversations · GET /ai/usage · POST /ai/feedback
media       POST /uploads/presign · POST /uploads/complete
notif       POST /devices · GET /notifications · PATCH /notifications/read
```

**Contract discipline:** generate the mobile TypeScript types from the backend OpenAPI spec. Hand-written client types drift within a sprint.

---

## 5. Data model — mobile-relevant view

```text
User ──< Vehicle ──< Modification >── Part
                 │
                 ├──< ServiceRecord >── Garage
                 ├──< ProblemCase ──< RepairSolution >── Garage
                 ├──< MileageLog
                 ├──< VehiclePhoto
                 └──< CommunityMember >── Community ──< Post ──< Comment

Vehicle ── VehicleVariant ── VehicleGeneration ── VehicleModel ── Brand
                                                       (catalog: seeded, read-only)

AIConversation ──< AIMessage ──< AIResponseEvidence >── (Build|Problem|Part|Service|Garage)
```

Fields the mobile client must treat as **never leaving the device unmasked**: `plate`, `vin`, `purchase_price`, exact coordinates.

---

## 6. Cold-start / low-data behaviour

Every surface below needs an explicit designed state for "the community has no data about this car yet":

| Surface | Low-data behaviour |
|---|---|
| Aha screen (E1-9) | Drop the counters; lead with "jadi yang pertama" and a contribution CTA |
| Home "for your car" | Fall back from generation → model → brand → general |
| Explore | Widen scope automatically, label the widening honestly ("belum ada build Civic FD — ini build Civic lain") |
| AI | Answer from general automotive knowledge with confidence `Low` and say plainly that no community evidence exists |
| Similar problems | Show none rather than irrelevant ones |

---

## 7. Phasing

The PRD's §40 "MVP" is roughly 58 screens and 12 epics. That is a 6–9 month build for one developer, and it puts the riskiest component (grounded AI) behind the longest possible feedback delay. Recommended re-cut:

### Slice 1 — "Garage yang berguna" (~6 weeks)
E0, E1, E2, E3, E4, plus E12-1 (media) and E12-3 (states).
Ships: register, add car, digital garage, build with structured mods, service history with reminders.
**Testable claim:** is a digital garage alone worth opening weekly?

### Slice 2 — "Otak komunitas" (~5 weeks)
E5 (problems), E7 (explore), E9 (home), plus the low-data behaviours in §6.
Ships: problem capture and resolution, explore other builds, contextual home.
**Testable claim:** do owners recognise themselves in other owners' cars?

### Slice 3 — "AI yang tahu mobil lo" (~6 weeks)
E8 full, backend retrieval layer, cost controls.
Ships: the differentiator. Deliberately last, because it is only as good as the data Slices 1–2 collected.
**Testable claim:** does the AI answer better than ChatGPT because it knows this specific car?

### Slice 4 — "Komunitas" (~5 weeks)
E6, E11, E10-3/4/7.
Ships: communities, notifications, privacy and compliance.

### Slice 5 — Store readiness (~2 weeks)
E12-6, store assets, review-response prep, legal pages, beta feedback pass.

**Total to public launch: ~24 weeks solo**, versus ~34 weeks for a big-bang v1, with the first real user signal arriving in week 6 instead of week 30.

---

## 8. Risks and hard problems

### 8.1 The vehicle catalog is a data project, not a feature
Brand → model → generation → variant → engine for the Indonesian market, including local naming that no international dataset carries (*Yaris Bakpao*, *Civic FD1/FD2*, *Avanza* generations). Nothing in the app works without it, and it cannot be crowdsourced at launch because there are no users. Options: license a dataset, scrape and curate, or hand-build the top ~200 enthusiast models and let free-text suggestions cover the tail. **Recommendation: hand-build the top 200 and ship the escape hatch.** Budget 3–4 weeks of non-engineering effort.

### 8.2 The aha screen lies on day one
"837 builds · 184 known issues" is the activation moment, and it reads `0 · 0 · 0` for every user until the platform has thousands of cars. Seeding options: import from a partner community, run a pre-launch build-submission campaign, or redesign the screen so that a rare car reads as an opportunity rather than an empty room. **This needs a product decision before E1-9 is built.**

### 8.3 Fitment data has no obvious source
The fitment advisor (PRD §16) needs OEM specs: PCD, center bore, stock offset ranges, brake clearance. Community evidence covers popular cars slowly and rare cars never. Until that exists, the fitment feature must lean entirely on the `Insufficient Data` confidence tier — which is honest but unimpressive in a demo. Plan for a curated seed of the top 50 enthusiast models.

### 8.4 AI cost is a product constraint, not an infra detail
20 free questions/month × retrieval + generation is real money at scale, and the PRD's own §50 controls (caching, routing, summarisation) must exist in the first AI commit, not in a later hardening pass. Instrument cost-per-request from day one.

### 8.5 Moderation is not deferrable
Indonesian automotive groups attract jual-beli spam quickly. Report, block, and admin-delete ship with communities or communities do not ship.

### 8.6 React Native specifics worth planning for
- A photo-heavy app needs an image cache eviction policy or it grows unbounded on device.
- Audio recording for problem capture requires permission priming and a real file-size budget.
- Deep links must work from a cold start while logged out, then resume after login — the single most commonly broken flow in RN apps.
- Apple review will scrutinise: account deletion, Sign in with Apple, and any AI feature that could be read as giving safety advice. The safety-warning presentation in E8-10 is partly a store-review requirement.

---

## 9. Estimation summary (mobile only, one developer)

| Epic | Size |
|---|---:|
| E0 Foundation | 18d |
| E1 Auth & onboarding | 12d |
| E2 Vehicle & garage | 22d |
| E3 Build & modification | 20d |
| E4 Service history | 12d |
| E5 Problems | 16d |
| E6 Community | 22d |
| E7 Explore | 16d |
| E8 AI (client) | 26d |
| E9 Home | 10d |
| E10 Profile & settings | 10d |
| E11 Notifications | 8d |
| E12 Cross-cutting | 22d |
| **Total mobile** | **~214d ≈ 43 weeks solo** |
| E1-11 Account roles (RBAC) | 5d |
| E13 Backoffice admin (web, separate surface) | 40d |
| **Total including backoffice** | **~259d ≈ 52 weeks solo** |
| E14 Subscription tiers (off the first release path) | 18d |
| E15 Landing page (public web) | 8d |
| E16 Backend (Rust + axum, 18 stories) | ~160d |
| **Total all surfaces** | **~445d ≈ 89 weeks solo** |

The E16 figure includes the Rust learning curve, which is real: Rust is not among the owner's listed strengths and was chosen partly as a learning goal (see §11). Treat ~160d as a mid estimate, not a floor. The earlier "backend is roughly the same magnitude again" line in §9 was a guess made before E16 existed; this number replaces it.

Backend is roughly the same magnitude again, and the AI retrieval layer alone is 4–6 weeks. The §7 slicing does not reduce this total; it changes the order so that the first shippable, learnable product arrives in week 6.

---

## 10. Open decisions

1. **Scope:** ship the sliced plan (§7) or the PRD's full v1?
2. **Backend language:** Go per PRD §48, or Node/TypeScript for one language across the whole stack?
3. **Catalog seeding:** licence, scrape, or hand-build top 200?
4. **Cold-start strategy:** how does the aha screen behave before there is a community?
5. ~~Design direction~~ — **resolved.** `docs/design.md` fixes the palette, typography, radius, and spacing. The doc's §68 Flutter theme example is stale against the React Native decision; the centralised-token concept it describes still holds.
6. **Backoffice timing:** the moderation and curation queues have no operator until E13 ships. Either E13 lands before the community opens, or those queues stay unworked at launch.
7. **Subscription pricing:** what Pro and Max cost. PRD §1773 leaves it out of the MVP, and no ticket in E14 hardcodes a number.
8. **Subscription timing:** E14 is scoped but unscheduled. Building billing before anyone has hit the free quota is work that may never pay for itself — the counter-argument is that retrofitting entitlement checks into shipped features costs more than building them alongside.
9. **Max at launch or later:** ship Max alongside Pro, or wait for evidence that people are hitting the Pro ceiling.

---

## 11. Jira tracker

The backlog lives in Jira project **AM** (`Anak Mobil`) at `oksasatyaa.atlassian.net`. Hierarchy is Epic → Story → Subtask. Stories carry a user story plus Given/When/Then acceptance criteria **written in Bahasa Indonesia**, a short technical-notes block, and an explicit out-of-scope line. Subtasks carry a one-paragraph description and a "Selesai ketika" definition of done — no G/W/T at that level.

**368 issues: 17 epics, 106 stories, 245 subtasks.** E16's subtasks are deliberately not written yet — see the E16 note below.

| Epic | Key | Stories | Subtasks |
|---|---|---|---|
| E0 Fondasi & App Shell | AM-1 | AM-14..AM-20 | AM-21..AM-49 |
| E1 Autentikasi & Onboarding | AM-2 | AM-50..AM-56, AM-77, AM-83 | AM-57..AM-76, AM-78..AM-81, AM-91..AM-93 |
| E2 Kendaraan & My Garage | AM-3 | AM-113..AM-120 | AM-121..AM-143 |
| E3 Build & Modifikasi | AM-4 | AM-144..AM-150 | AM-151..AM-167 |
| E4 Riwayat Servis | AM-5 | AM-168..AM-171 | AM-178..AM-188 |
| E5 Problem & Pengetahuan | AM-6 | AM-172..AM-177 | AM-189..AM-202 |
| E6 Komunitas | AM-7 | AM-203..AM-207 | AM-213..AM-222 |
| E7 Explore | AM-8 | AM-208..AM-212 | AM-223..AM-230 |
| E8 AnakMobil AI | AM-9 | AM-231..AM-238 | AM-239..AM-257 |
| E9 Home Kontekstual | AM-10 | AM-258..AM-260 | AM-276..AM-280 |
| E10 Profil, Setelan & Kepatuhan | AM-11 | AM-261..AM-266 | AM-286..AM-296 |
| E11 Notifikasi | AM-12 | AM-267..AM-269 | AM-281..AM-285 |
| E12 Fondasi Lintas-Fitur | AM-13 | AM-270..AM-275 | AM-297..AM-306 |
| E13 Backoffice Admin (web) | AM-82 | AM-84..AM-90 | AM-94..AM-112 |
| E14 Langganan & Monetisasi | AM-307 | AM-308..AM-317 | AM-318..AM-339 |
| E15 Landing Page (web publik) | AM-340 | AM-341 | AM-342..AM-348 |
| E16 Backend (Rust + axum) | AM-349 | AM-350..AM-367 | *not yet written* |

**Cross-cutting rules encoded as acceptance criteria rather than left implicit:**

- Private-data filtering is always **server-side** — plate/VIN/price (AM-135), service costs (AM-162), admin responses (AM-110), profile visibility (AM-287), and every read endpoint (AM-291). The client never receives private data and hides it.
- Every empty state carries an action (AM-272), and the low-data behaviour is a designed primary state, not a fallback branch (AM-260, AM-280, AM-199, AM-224, AM-229).
- Two platform roles only — `admin` and `user` (AM-83). Platform role is separate from community membership role.
- Account deletion (AM-266) is a store-review release blocker, not a nice-to-have.
- Community contribution is **never** paywalled (AM-308 AC3, AM-315 AC3) — the direct application of the PRD §57 closing line, "Do not gate basic community contribution behind payment". Written as acceptance criteria so it cannot be quietly reversed later.

### E14 — Subscription tiers

Added on request after the backlog was complete. The tier contents are derived from the PRD §57 "AI Pro" list, not invented: higher AI limits, advanced build planning, comparison, deep service analysis, image analysis, export vehicle report.

| Tier | Contents |
|---|---|
| **Free** | 20 AI questions/month. Garage, builds, service history, problems, and community fully open. |
| **Pro** | Much larger AI quota, deep service-history analysis, build comparison, vehicle report export. |
| **Max** | Highest AI quota, photo analysis for damage and parts, staged advanced build planning, queue priority during load. |

The ladder lives in **one** ticket — the entitlement matrix (AM-308). Changing the ladder is one edit, not ten.

Four hard rules encoded in the epic:

1. Community contribution is never gated (above).
2. Entitlement is decided **server-side, always** — a rooted device can fake anything the client decides (AM-308 AC5).
3. Digital goods must go through store IAP. Using external payment for paid AI features gets the app rejected — a store rule, not an architecture preference (AM-310).
4. **Downgrade never deletes data** (AM-313 AC5, AM-330). Features lock, data stays. Deleting on expiry turns disappointment into permanent loss with no way back.

Pricing is deliberately absent from every ticket — PRD §1773 states exact pricing is not part of the MVP. It is an open decision, not an oversight.

E14 is **not on the first release path** unless decided otherwise. The foundation already exists: AM-253 enforces quota server-side and the limit is already configurable without an app release.

### E15 — Landing page, and the current work order

Added on request as the **first surface to build**, ahead of the mobile app. It is a separate surface from both the app (E0–E12) and the backoffice (E13): public web, search-indexed, so the SEO gate is mandatory here and skipped on the behind-auth dashboard.

Two reasons it earns first position rather than being marketing decoration:

- **It ships in days, not months**, and works before the first line of app code is finished.
- **It is a direct instrument against the §8.2 cold-start problem.** Waitlist signups collected now are people who can be called on launch day, instead of an empty app waiting to be discovered. AM-341 AC3 makes waitlist mode the primary behaviour, not a fallback.

AM-87 (vehicle catalog curation) additionally carries `seed-katalog`. It is the tool that makes the §8.1 catalog data project workable — hand-building the top ~200 models needs an editor, and that editor is AM-87.

### E16 — Backend, and the committed stack

Added 2026-08-15 after the architecture was settled. Until then the backend — roughly half the total work — had **zero tickets**, while mobile, backoffice, subscriptions, and landing all had full breakdowns.

| Surface | Stack |
|---|---|
| Backend | Rust + axum, modular monolith, one codebase, two process roles (`web` / `worker`) |
| Database | Postgres + pgvector. No separate vector DB. |
| Redis | Sessions (revocable JWT), rate limiting, read cache |
| Job queue | Postgres — `FOR UPDATE SKIP LOCKED`, lease, backoff, dead-letter |
| Landing (E15) | Astro, React island for the waitlist form only |
| Backoffice (E13) | Vite + React + shadcn/ui + TanStack (Query, Table, Router, Form, Virtual — **not** TanStack Start, which is SSR) |
| Mobile (E0–E12) | React Native + Expo |

Six domain crates: `identity`, `garage`, `build`, `knowledge`, `ai`, `waitlist`. `admin` and `notification` are adapters. `billing` is not built until monetisation is scheduled.

**The boundary is enforced by the compiler, not by lint.** An earlier draft of this design claimed `#![forbid(unsafe_code)]` + clippy enforced domain purity. That was wrong — `forbid(unsafe_code)` forbids `unsafe` blocks, not imports. The real enforcement is a domain crate whose `Cargo.toml` carries no adapter dependencies, so `use axum` in the domain fails to build.

Quality gate is **not Sonar**: `cargo fmt --check` → `cargo clippy --all-targets --all-features -- -D warnings` → `cargo nextest run` → `cargo llvm-cov` (≥90% new code) → `cargo audit` → `cargo deny check`.

E16's **subtasks are deliberately unwritten**. The 18 stories already carry the Codex findings; subtasks are the layer most likely to shift once the architecture spec lands, and writing them now would mean writing them twice.

### Cross-model review — ten findings, all absorbed

The design was adversarially reviewed by a second frontier model before any ticket was written. Verdict was *reject as-is*; the monolith shape survived, the operational and AI contracts did not. All ten are now encoded in tickets rather than left as notes:

| # | Finding | Where it landed |
|---|---|---|
| 1 | SSE final-frame loses the **safety warning** on a dropped connection | AM-231, AM-244, AM-364 — persist-first, SSE is transport |
| 2 | Quota can be exceeded by parallel requests | AM-237, AM-253 — atomic reservation |
| 3 | No write idempotency → duplicate service records on retry | AM-357 (server), AM-368 (client outbox) |
| 4 | Web + worker in one process → CPU spikes starve SSE | AM-350 — two process roles, one binary |
| 5 | "Worker picks it up" is not a queue contract | AM-358 — lease, backoff, DLQ |
| 6 | Retrieval can return confidently wrong **fitment** | AM-233, AM-363 — filter vehicle *before* ANN |
| 7 | pgvector has no exit criteria | AM-363 — measured recall, p95, candidate count |
| 8 | 12 modules is ceremony; lint does not enforce boundaries | AM-349, AM-350 — six crates, compiler-enforced |
| 9 | No recovery or observability design | AM-367 — PITR drill, alerts, runbook |
| 10 | Community content has no trust boundary before AI indexing | AM-234, AM-247, AM-365 |

Finding 1 is the most serious: in an app that answers questions about brakes and fuel leaks, an answer that arrives without its safety warning is a safety defect, not a UX one.

### Current sprint

Six items, marked `sprint-1` in Jira. Four are the sequential backend critical path; two run in parallel on different stacks.

| Order | Key | Story | Status |
|---|---|---|---|
| 1 | AM-350 | Bootstrap workspace, crate boundaries, process roles | **In Progress** |
| 2 | AM-352 | Config, structured logging, health probes | To Do |
| 3 | AM-351 | Response envelope + single error choke point | To Do |
| 4 | AM-353 | Database schema + migrations | To Do |
| — | AM-15 | Design system and tokens (unblocks all three frontends) | To Do |
| — | AM-341 | Public landing page (Astro, ships fastest, starts the waitlist) | To Do |

Deliberately **not** pulled: E13 backoffice (blocked on auth AM-354 and admin API AM-366), mobile feature stories E2–E12 (blocked on their APIs), and E14 subscriptions (off the first release path).

**Not in Jira:** Confluence pages per Epic/Story. The Atlassian connector on this site has Jira write access but Confluence read-only, so the standing "one Confluence page per Epic and Story from the template" rule could not be fulfilled from here. Either grant Confluence write scope or create those pages manually from the Jira issues' **Confluence content** panel.
