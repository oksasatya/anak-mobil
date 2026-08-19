# AnakMobil.id — Design System & UI Guidelines

**Version:** 1.0  
**Status:** Draft  
**Brand:** AnakMobil.id  
**Product:** Automotive Community Platform + Digital Garage + AnakMobil AI  
**Platforms:** Flutter Mobile App + Next.js Public Web  
**Last Updated:** 14 August 2026

---

# 1. Design Direction

AnakMobil.id harus terasa seperti **produk otomotif modern**, bukan seperti forum lama, marketplace sparepart, atau aplikasi bengkel tradisional.

Karakter visual utama:

- Modern
- Automotive
- Confident
- Clean
- Community-driven
- Technical, tetapi tetap mudah digunakan
- Premium tanpa terlihat mewah berlebihan
- Energetic melalui accent orange-red
- Dark/graphite sebagai identitas utama

Brand harus terasa cocok untuk:

- Daily driver owner
- Car enthusiast
- Builder
- Community member
- Workshop / garage
- Event organizer

### Core Visual Statement

> **Digital garage yang modern, cepat, dan hidup.**

---

# 2. Brand Concept

Logo AnakMobil.id menggabungkan tiga ide utama:

## 2.1 Letterform

Simbol utama menggunakan bentuk geometris yang mengarah ke:

- Huruf **A**
- Nuansa monogram **AM**
- Identitas AnakMobil

## 2.2 Garage / Shelter

Outline pada logo memiliki karakter seperti:

- Garage
- Roof
- Shelter

Maknanya:

> AnakMobil.id adalah rumah digital untuk mobil user.

## 2.3 Road / Motion

Garis orange-red menyerupai:

- Jalan
- Lane
- Racing line
- Movement

Maknanya:

- Driving
- Journey
- Modification journey
- Community movement
- Progression

---

# 3. Brand Personality

Gunakan prinsip berikut ketika membuat UI baru.

| Trait | Direction |
|---|---|
| Modern | Clean surfaces, strong typography, minimal ornament |
| Automotive | Technical specs, strong imagery, structured data |
| Energetic | Accent orange-red digunakan secara terkontrol |
| Trustworthy | Hindari UI gimmicky dan klaim AI berlebihan |
| Community | Owner/build/community selalu terasa terhubung |
| Premium | Spacing lega, hierarchy kuat, visual tidak ramai |

AnakMobil.id **bukan**:

- Racing game UI
- Cyberpunk dashboard
- Marketplace penuh banner
- Forum 2008
- Social media clone
- Bengkel POS/dashboard corporate

---

# 4. Logo System

Logo terdiri dari:

1. **Primary Horizontal Logo**
   - Icon + `AnakMobil.id`
   - Untuk website header, deck, media kit, email, documentation.

2. **Symbol / Favicon**
   - Icon saja.
   - Untuk browser favicon, compact navigation, social avatar.

3. **Mobile App Icon**
   - Symbol putih + orange-red pada dark graphite background.
   - Untuk iOS / Android app icon.

4. **Vertical Lockup**
   - Symbol di atas.
   - Wordmark di bawah.
   - Untuk splash screen, onboarding, poster, social assets.

---

# 5. Logo Usage Rules

## 5.1 Clear Space

Minimal clear space di sekitar logo:

```text
X = tinggi crossbar / inner accent pada simbol
```

Gunakan minimal:

```text
Top    : 2X
Right  : 2X
Bottom : 2X
Left   : 2X
```

Jangan meletakkan logo terlalu dekat dengan:

- edge screen
- image
- button
- border
- headline

---

## 5.2 Minimum Size

### Horizontal logo

Digital minimum:

```text
160 px width
```

### Symbol

Minimum:

```text
24 × 24 px
```

Ideal UI size:

```text
32 × 32
40 × 40
48 × 48
```

### App Icon

Master asset:

```text
1024 × 1024
```

---

## 5.3 Logo Background

Preferred:

```text
Light:
#FFFFFF
#F7F8FA

Dark:
#11161C
#181E25
```

Hindari background:

- sangat ramai
- low contrast
- gradient berlebihan
- warna yang bertabrakan dengan orange-red

---

## 5.4 Do Not

Jangan:

- Stretch logo
- Rotate logo
- Tambahkan shadow berat
- Tambahkan outline sembarang
- Ubah orange menjadi warna lain tanpa alasan
- Masukkan gambar mobil ke dalam logo
- Gunakan logo sebagai repeating background pattern
- Gunakan icon detail pada ukuran favicon kecil

---

# 6. Primary Color Palette

Palet berasal dari karakter logo.

## 6.1 Brand Graphite

```css
--brand-950: #0F141A;
--brand-900: #151B22;
--brand-800: #1D232A;
--brand-700: #2B323B;
--brand-600: #3C4550;
```

**Primary recommendation:**

```text
Graphite 800
#1D232A
RGB 29, 35, 42
```

Gunakan untuk:

- Main text
- Dark navigation
- Dark surfaces
- Logo
- Headings
- Primary shell

---

## 6.2 AnakMobil Orange

Logo accent utama.

```css
--orange-700: #C93413;
--orange-600: #DC3E17;
--orange-500: #ED491C;
--orange-400: #F45C32;
--orange-300: #FF805E;
```

**Primary recommendation:**

```text
AnakMobil Orange
#ED491C
RGB 237, 73, 28
```

Gunakan untuk:

- Primary CTA tertentu
- Selected state
- AI highlight
- Live/action indicator
- Important automotive accent
- Brand link
- `.id` wordmark

Jangan gunakan orange untuk seluruh halaman.

Target visual:

```text
80–90% neutral
10–20% brand accent
```

---

# 7. Neutral Palette

## Light Theme

```css
--background:        #F7F8FA;
--surface:           #FFFFFF;
--surface-subtle:    #F1F3F5;
--surface-raised:    #FFFFFF;

--text-primary:      #171C22;
--text-secondary:    #5D6670;
--text-tertiary:     #616A74;

--border:            #E3E6E9;
--border-strong:     #CDD2D7;
```

## Dark Theme

```css
--background-dark:      #0E1217;
--surface-dark:         #151A20;
--surface-dark-raised:  #1B2128;
--surface-dark-subtle:  #202730;

--text-dark-primary:    #F5F7F8;
--text-dark-secondary:  #AAB2BA;
--text-dark-tertiary:   #8E98A2;

--border-dark:          #29313A;
```

`--text-tertiary` and `--text-dark-tertiary` were repaired 2026-08-19 (AM-15): the
previous values failed WCAG AA — 3.12:1 on `--surface` and 2.80:1 on
`--surface-subtle` (light, below even the 3:1 large-text floor), 4.18:1 on
`--surface-dark` and 3.87:1 on `--surface-dark-raised` (dark). The repaired values
reach 5.49:1 / 4.94:1 (light) and 5.97:1 / 5.53:1 (dark) on the same pairs.
`packages/tokens/src/tokens.js` is the source of truth; these match it.

---

# 8. Semantic Colors

Brand orange tidak boleh digunakan untuk semua status.

```css
--success: #168A52;
--warning: #D58A00;
--danger:  #D63B3B;
--info:    #2678D9;
```

These four values are **fills, borders, and icons only — never text.** As text on
their own theme's surface they fail AA: success is 3.99:1 on the dark surface, warning
is 2.81:1 on white, danger is 3.79:1 on dark, info is 3.97:1 on dark. Where a status
needs a label, use Semantic Text below.

Usage:

| State | Color |
|---|---|
| Compatible / Verified | Success |
| Needs Check | Warning |
| Not Compatible | Danger |
| Informational | Info |
| Brand / CTA | AnakMobil Orange |

## Semantic Text

A separate pair per theme, for when a status carries words rather than a fill, border,
or icon. A component that needs both uses `semantic` above for the border/icon and
one of these for the label — which also satisfies §61's "do not communicate status by
colour alone" for free.

| Role | Light | Ratio (white / surface-subtle) | Dark | Ratio (surface / surface-subtle) |
|---|---|---|---|---|
| Success | `#137747` | 5.59 / 5.02 | `#1FA463` | 5.45 / 4.70 |
| Warning | `#8F5C00` | 5.68 / 5.10 | `#D58A00` | 6.22 / 5.35 |
| Danger | `#C22C2C` | 5.69 / 5.12 | `#EC6363` | 5.44 / 4.68 |
| Info | `#1F63B5` | 5.98 / 5.38 | `#4A93E8` | 5.52 / 4.75 |

Not emitted to CSS — mobile-only, per `packages/tokens/src/tokens.js`'s `semanticText`
group.

---

# 9. AI Confidence Colors

AnakMobil AI membutuhkan language visual khusus.

```text
Verified          → Success
High Confidence   → Success subtle
Medium Confidence → Warning
Low Confidence    → Neutral / Warning subtle
Insufficient Data → Neutral
Safety Warning    → Danger
```

Jangan menggunakan warna hijau untuk hasil AI yang belum benar-benar verified.

Label colours use `semanticText` (§8), never the `semantic` fill/border/icon values
directly — the latter fail AA as text. Per §61, a confidence badge is always solid
(`working` role, §77) and carries a label and a shape as well as a colour; colour
alone is never the signal.

---

# 10. Typography

## 10.1 UI Typeface

Recommended:

> **Inter**

Fallback:

```css
font-family:
  Inter,
  ui-sans-serif,
  system-ui,
  -apple-system,
  BlinkMacSystemFont,
  "Segoe UI",
  sans-serif;
```

Alasan:

- Clear pada mobile
- Bagus untuk numeric/spec data
- Dense information tetap readable
- Support web/mobile consistency

---

## 10.2 Wordmark

Wordmark logo adalah asset brand.

Jangan recreate wordmark dengan text biasa di UI jika logo asset tersedia.

---

# 11. Type Scale

## Mobile

```text
Display      32 / 38 / 700
H1           28 / 34 / 700
H2           24 / 30 / 700
H3           20 / 26 / 650
Title        18 / 24 / 650
Body Large   16 / 24 / 400
Body         14 / 21 / 400
Label        13 / 18 / 600
Caption      12 / 17 / 400
Micro        11 / 14 / 500
```

In the app, the 650 weight above renders at **600**: React Native's `fontWeight` has no
650 and the shipped Inter cuts are static. The desktop scale below keeps 650 and 750,
where the web loads the variable cut.

## Desktop Web

```text
Hero        56 / 64 / 750
H1          44 / 52 / 700
H2          36 / 44 / 700
H3          28 / 36 / 650
Title       20 / 28 / 650
Body        16 / 25 / 400
Small       14 / 21 / 400
```

---

# 12. Automotive Numeric Typography

Untuk data seperti:

```text
18×8.5
ET40
225/40 R18
146,120 KM
148 WHP
Rp 14.500.000
```

Gunakan:

- Tabular numbers jika tersedia
- Medium / semi-bold
- High contrast
- Hindari font decorative

Contoh:

```text
18×8.5 ET40

225/40 R18

146,120 KM
```

Specs harus mudah discan.

---

# 13. Spacing System

Base unit:

```text
4 px
```

Tokens:

```text
4
8
12
16
20
24
32
40
48
64
80
96
```

Mobile page padding:

```text
16 px
```

Large mobile:

```text
20 px
```

Desktop container:

```text
max-width: 1200–1280 px
```

---

# 14. Border Radius

Recommended:

```text
XS      6 px
SM      8 px
MD     12 px
LG     16 px
XL     20 px
2XL    28 px
Pill   999 px
```

Core cards:

```text
16 px
```

Hero/car imagery:

```text
20–24 px
```

Buttons:

```text
12 px
```

Do not make every component pill-shaped.

---

# 15. Elevation & Material

Use borders before shadows. **On mobile this is superseded for the `chrome` and
`surface` glass roles, and survives intact for `working`** (§77 Material System): a
translucent surface gets its form from its edge, not from a border, while a solid
data surface needs no edge at all.

## Card Default

```text
border: 1px solid neutral-border
shadow: none / very subtle
```

## Floating Elements

Examples:

- Bottom sheet
- AI composer
- Floating action
- Dropdown

Use soft shadow only.

Avoid:

- black heavy shadow
- neumorphism
- glossy dashboard UI — a warning against **gloss** (shiny gradient chrome, specular
  sweeps), not against the glass material defined in §77

---

# 16. Layout Philosophy

UI should prioritize:

1. Vehicle
2. Context
3. Action
4. Evidence
5. Social interaction

Never prioritize:

1. Ads
2. Random feed
3. Engagement bait

---

# 17. Mobile Navigation

Recommended bottom navigation:

```text
Home
Garage
Explore
Community
Profile
```

Icon labels always visible.

Suggested structure:

```text
Home       house
Garage     car / garage
Explore    compass
Community  users
Profile    user
```

Active state:

```text
Icon: AnakMobil Orange
Label: Graphite / light text
```

---

# 18. Global AI Entry Point

AI harus mudah diakses tanpa mendominasi seluruh app.

Recommended:

```text
Floating / contextual "Ask AI"
```

or home card:

```text
┌──────────────────────────────┐
│ AnakMobil AI                 │
│ Tanya apa aja soal Civic lo  │
│                              │
│ [ Ask AnakMobil AI ]         │
└──────────────────────────────┘
```

AI icon dapat menggunakan:

- Spark / intelligence shape
- Logo symbol + small spark
- Tidak menggunakan robot head cliché

---

# 19. Home Screen

Home bukan infinite social feed.

Recommended hierarchy:

```text
Header
Selected Vehicle
AI Entry
Vehicle Insights
Upcoming Maintenance
Relevant Builds
Community Updates
Events
```

Example:

```text
Good evening

Honda Civic FD1
2008 · R18A

[ Ask AnakMobil AI ]

Known Issues
12 relevant to your model

Popular Setups
18×8.5 ET40

Service
Oil due in ±1,100 km

Community
Civic FD Indonesia
```

---

# 20. Selected Vehicle Card

Ini adalah component penting.

Structure:

```text
┌────────────────────────────────┐
│ [Vehicle Photo]                │
│                                │
│ HONDA                          │
│ Civic FD1                      │
│ 2008 · R18A · Automatic        │
│                                │
│ 12 Mods   8 Services   3 Issues│
└────────────────────────────────┘
```

Rules:

- Vehicle photo dominan.
- Brand kecil.
- Model/generation besar.
- Technical metadata secondary.
- Metrics jangan terlalu banyak.

---

# 21. My Garage Screen

Visual direction:

```text
MY GARAGE

[ Civic FD1 card ]

[ BMW E46 card ]

+ Add Vehicle
```

Avoid grid terlalu padat.

Mobile:

```text
1 column
```

Tablet/Desktop:

```text
2–3 columns
```

---

# 22. Vehicle Detail

Recommended tabs:

```text
Overview
Build
Service
Problems
Specs
```

Header:

- Hero vehicle photo
- Vehicle name
- Generation
- Build summary
- Owner privacy controls

---

# 23. Build UI

Build harus terasa seperti technical profile, bukan shopping list.

Sections:

```text
Wheels & Tyres
Suspension
Brakes
Engine
Intake
Exhaust
ECU
Exterior
Interior
Audio
```

Each part row:

```text
WHEELS

Enkei RPF1
18×8.5 · ET40
Installed Feb 2026

[photo]
```

---

# 24. Build Timeline

Timeline style:

```text
2026

FEB
● Enkei RPF1
  18×8.5 ET40
  Rp14.500.000

MAR
● Michelin PS5
  225/40 R18

MAY
● Tein Flex Z
```

Orange marker only for significant milestones or selected item.

Do not make every timeline line orange.

---

# 25. Fitment Card

Fitment is one of the signature AnakMobil components.

Example:

```text
FITMENT CHECK

Enkei RPF1
18×8.5 ET35

Likely Compatible
HIGH CONFIDENCE

42 similar Civic FD builds

Things to check
• Fender clearance
• Ride height
• Front wheel poke

Common setup
18×8.5 ET38–40
225/40 R18

[ Ask AI ] [ See 42 Builds ]
```

Visual priority:

1. Compatibility
2. Confidence
3. Community evidence
4. Warnings
5. Action

---

# 26. AI Chat Design

AI chat should feel like **automotive expert workspace**, not generic messenger.

Header:

```text
AnakMobil AI

Context:
Honda Civic FD1
```

Composer:

```text
Ask about your Civic...
```

Quick prompts:

```text
Fitment
Problem
Maintenance
Build Plan
Compare Parts
```

---

# 27. AI Answer Structure

Avoid wall-of-text.

Preferred:

```text
Short Answer

Confidence

Why

Community Evidence

Things to Check

Recommended Next Step
```

Example:

```text
Likely compatible.

HIGH CONFIDENCE

42 Civic FD builds use a similar setup.

Watch for:
• Front fender clearance
• Lowered ride height

Common:
18×8.5 ET38–40

[ View Evidence ]
```

---

# 28. AI Evidence

Every grounded AI answer should expose evidence when possible.

Evidence component:

```text
Based on

42 Builds
18 Repair Cases
7 Service Records
```

Tap:

```text
View Sources
```

Do not display fake academic citation style.

Use product-native evidence.

---

# 29. AI Warning Component

Safety-critical response:

```text
⚠ Safety Check

Steering-related symptoms can affect vehicle control.

Avoid continued driving if steering feels loose,
locks, or becomes difficult.

Have the vehicle inspected by a qualified technician.
```

Use danger color only on the warning section, not entire screen.

---

# 30. Problem Card

Example:

```text
Bunyi tek-tek saat belok

Honda Civic FD
143,000 KM

SOLVED

Diagnosis
CV Joint

Solution
Replace CV Joint

Rp1.200.000
```

Status:

```text
Open
Investigating
Solved
```

---

# 31. Repair Intelligence

Example:

```text
Similar Civic FD Cases

CV Joint          51%
Rack End          22%
Tie Rod           14%
Other             13%

Based on 63 solved cases
```

Data visualization:

- Horizontal bar
- Never pie chart by default
- Always show sample size

---

# 32. Service History UI

Timeline-oriented.

Example:

```text
146,100 KM
Today

Engine Oil
Oil Filter

XYZ Garage
Rp850.000
```

Upcoming:

```text
Next Suggested
150,000 KM
```

Use subtle orange highlight for upcoming action.

---

# 33. Community UI

Community page structure:

```text
Community Header
Vehicle relevance
Join status

Tabs:
Overview
Discussions
Builds
Problems
Events
```

Community identity should be visible but never overpower vehicle context.

---

# 34. Community Post Types

Different post types need visible labels:

```text
QUESTION
PROBLEM
BUILD
GARAGE
EVENT
DISCUSSION
```

Use neutral label backgrounds.

Orange reserved for:

- important selected state
- action
- live/event highlight

---

# 35. Explore Screen

Explore should begin from structured discovery.

Recommended filters:

```text
Same Car
Builds
Wheels
Suspension
Problems
Communities
```

Search placeholder:

```text
Search cars, builds, parts, problems...
```

---

# 36. Search Result Cards

Each result must show entity type.

Example:

```text
BUILD

Honda Civic FD1
Enkei RPF1 · Tein Flex Z

by @username
```

or:

```text
PROBLEM

Rack steer noise
Honda Civic FD

31 solved cases
```

---

# 37. Event Design — Future

Event card:

```text
CIVIC FD SUNDAY MEET

24 Aug · 08:00

Senayan, Jakarta

74 cars going

[ Join ]
```

Hero can use automotive photography.

Avoid nightclub-event visual language.

---

# 38. Garage / Workshop Design — Future

Garage profile must emphasize evidence.

Example:

```text
XYZ GARAGE

Honda Specialist

184 Civic FD jobs
92 Jazz GK jobs

4.9
83 structured reviews

Common jobs
Rack steer
CV joint
Engine mounting
```

Do not make Google Maps rating the only trust signal.

---

# 39. Price Intelligence — Future

Example:

```text
Rack Steer Repair
Honda Civic FD

LOW
Rp1.200.000

MEDIAN
Rp1.700.000

HIGH
Rp2.400.000

83 records
```

Use range visualization.

Never communicate:

> "Harga seharusnya Rp1.7jt"

Preferred:

> "Community median: Rp1.7jt"

---

# 40. Dark Mode

Dark mode is important because brand naturally supports it.

Primary dark shell:

```text
#0E1217
```

Cards:

```text
#151A20
#1B2128
```

Orange:

```text
#ED491C
```

Do not increase saturation excessively in dark mode.

Automotive photography looks particularly strong on dark mode.

## Glass Recipe

Three roles, distinguished by coverage, not blur radius (§77 Material System):

```text
chrome:   tint #0E1217 at 80%  ->  composite #0E1217
surface:  tint #151A20 at 92%  ->  composite #14191F
working:  #151A20, solid, zero transparency
```

The composited colour is the contract, not the tint or the coverage percentage.

---

# 41. Light Mode

Light mode should feel:

- crisp
- modern
- editorial
- technical

Background:

```text
#F7F8FA
```

Cards:

```text
#FFFFFF
```

Avoid pure-white-everywhere appearance.

## Glass Recipe

```text
chrome:   tint #FBFCFD at 80%  ->  composite #FAFBFC
surface:  tint #FCFDFD at 92%  ->  composite #FCFDFD
working:  #FFFFFF, solid, zero transparency
```

`surface` tints off-white (`#FCFDFD`) rather than the subtle grey (`#F1F3F5`) used
elsewhere in this document: at 92% coverage that grey gives secondary text only
4.40:1 over a dark backdrop — a fail. What keeps "avoid pure-white-everywhere" true
without breaking the contrast contract is that **the edge, not the fill, is what
separates a surface from the page** (§77) — the composite stays near-white on purpose.

---

# 42. Buttons

## Primary

Recommended:

```text
Background: #1D232A
Text: #FFFFFF
```

For strongest brand CTA:

```text
Background: #ED491C
Text: #0F141A
```

White on `#ED491C` is 3.77:1 and fails AA; graphite-950 (`#0F141A`) reaches 4.91:1.
The orange fill is unchanged — only the label colour repairs the contrast.

Orange CTA should be used selectively.

## Secondary

```text
Background: transparent / white
Border: #CDD2D7
Text: #1D232A
```

## Destructive

Use semantic danger, not orange.

---

# 43. Button Heights

Mobile:

```text
Small   36 px
Medium  44 px
Large   52 px
```

Primary onboarding/action:

```text
52 px
```

Radius:

```text
12 px
```

---

# 44. Input Fields

Recommended:

```text
Height: 48–52 px
Radius: 12 px
Border: neutral
Focus border: graphite / orange accent
```

Use orange focus sparingly.

Form labels always visible for structured automotive data.

Do not rely only on placeholder.

---

# 45. Selection Controls

Vehicle specs need fast selection.

Use:

- Chips
- Segmented controls
- Search select
- Bottom sheet picker

Examples:

```text
Manual
Automatic
CVT
DCT
```

or:

```text
Daily
Track
Stance
Touring
Show
```

---

# 46. Cards

Default card is the `surface` glass role (§77 Material System). Border is superseded
by the role's own edge treatment (§15):

```text
Background: surface (glass role, composited ~88-92%)
Edge: top highlight + bottom inset shadow — no border
Radius: 16px
Padding: 16px
```

**Exception — `working` role, solid, zero transparency:** service history, fitment
results, forms, AI evidence, AI warnings, confidence badges, and the eight §73
signature components. These are read to make a decision, often outdoors, and a
surface whose contrast varies with whatever is behind it is the wrong material for a
service record.

Card density should vary:

- Vehicle Card → visual
- Spec Card → compact
- AI Card → structured
- Community Card → human
- Service Card → timeline

---

# 47. Image Treatment

Automotive photography is important.

Recommended:

- Natural car photography
- Avoid excessive filters
- Preserve vehicle color
- Strong crop
- Low-angle shots allowed
- Detail shots for parts

Ratios:

```text
Vehicle Hero     16:9 / 4:3
Build Card        4:3
Community Avatar  1:1
Part Image        1:1
Event Hero       16:9
```

The ground behind these photos (§77 Material System) is a **tint, not a filtered
photo** — a blurred, scrimmed photo would violate both "preserve vehicle color" and
"avoid excessive filters" above, and costs roughly 46 MiB per decoded 4000×3000
image, approaching a gigabyte across twenty cached vehicles. The tint keeps what the
photo contributes — this garage belongs to *this* car — without the cost or the
filter.

---

# 48. Placeholder Imagery

If no vehicle photo:

Use:

```text
Neutral silhouette / simplified vehicle placeholder
```

Do not use a random stock car that may imply wrong model.

---

# 49. Iconography

Style:

- Outline
- 1.75–2 px stroke
- Rounded joins
- Simple
- Recognizable

Avoid mixing:

- filled cartoon icons
- realistic automotive illustrations
- inconsistent icon packs

Core icon concepts:

```text
Garage
Car
Wrench
Wheel
Tyre
Engine
Community
Event
Map
Route
AI
History
Warning
Verified
```

---

# 50. Motion

Animation should communicate state.

Recommended duration:

```text
Micro interaction: 120–180 ms
Standard:          180–240 ms
Sheet/modal:       240–320 ms
```

Use:

- smooth card transitions
- image fade
- bottom sheet
- selected vehicle transition
- AI typing state

Avoid:

- racing animation
- speedometer animation everywhere
- constant glowing orange effects
- glow on glass — no exception for the `chrome`/`surface` roles (§77); the ban above
  already covers it, restated here because glass is where a "constant glow" is
  easiest to reach for

---

# 51. Loading States

Use skeletons for:

- vehicle cards
- build lists
- community content
- AI evidence

AI loading:

```text
Checking your Civic...
Searching similar builds...
Reviewing service history...
```

Only show these messages if they correspond to actual processing states.

---

# 52. Empty States

Empty states should encourage meaningful contribution.

Example:

```text
No modifications yet.

Start building your digital garage.

[ Add First Mod ]
```

Problem empty:

```text
No problem history.

Good sign.

When something comes up, record it here so your car's
history stays complete.
```

---

# 53. Error States

Tone:

- direct
- useful
- non-technical

Bad:

```text
Something went wrong.
```

Better:

```text
We couldn't load your garage.

Your data is safe.
Try again.
```

AI:

```text
I don't have enough fitment data to answer confidently.

Try adding your current suspension and wheel specs.
```

---

# 54. Onboarding Visual Direction

Onboarding should be short.

Recommended flow:

```text
Welcome
↓
What do you drive?
↓
Brand
↓
Model
↓
Generation
↓
Year
↓
Photo
↓
Garage Ready
```

After completion:

```text
Your Civic world is ready.

837 Builds
184 Known Issues
7 Communities

[ Explore My Car ]
```

Strong brand moment.

---

# 55. Splash Screen

Dark version recommended.

```text
Background:
#0E1217

Center:
AnakMobil symbol

Accent:
#ED491C
```

No long animation.

Maximum branding animation should feel quick and restrained.

---

# 56. Public Website

Public site goals:

1. SEO
2. Shareability
3. App acquisition
4. Trust

Header:

```text
Logo
Explore
Communities
AI
About

[ Open App ]
```

Public pages must work without login.

---

# 57. Landing Page Direction

Hero idea:

```text
YOUR CAR.
MORE THAN A PROFILE.

Build it.
Track it.
Ask it.
Meet the community behind it.

[ Join AnakMobil ]
```

Hero visual:

- Real enthusiast vehicle
- UI overlay showing digital garage
- Avoid generic luxury supercar if product targets broad Indonesian enthusiasts

---

# 58. Public Car Page

Example:

```text
Oksa's Civic FD

Honda Civic FD1
2008

12 Mods
8 Service Records
148 WHP

Build
Wheels
Suspension
Exhaust
```

Sensitive records are never publicly shown unless explicitly enabled.

---

# 59. SEO Knowledge Page

Example:

```text
Honda Civic FD Rack Steer Problems

63 solved community cases

Common reported causes
Average mileage
Typical solutions
Community price range

[ Ask AnakMobil AI ]
```

AI content must not create false authoritative claims.

---

# 60. Responsive Web

Breakpoints suggestion:

```text
Mobile       < 640
Tablet       640–1023
Desktop      1024–1439
Wide         >= 1440
```

Max content width:

```text
1280 px
```

Article/knowledge content:

```text
720–800 px
```

---

# 61. Accessibility

Minimum:

- WCAG AA contrast
- Touch target ≥ 44×44
- Do not communicate status by color alone
- Dynamic text support
- Semantic labels
- Keyboard navigation on web
- Focus visible
- Alt text for meaningful images

Automotive spec tables must remain readable at large text sizes.

---

# 62. Localization

Primary language:

```text
Bahasa Indonesia
```

Tone:

- friendly
- automotive-native
- concise
- not overly formal

Accept industry terminology:

```text
Fitment
Offset
PCD
Coilover
Remap
Build
Daily
Track
Stance
```

Do not translate terminology if translation makes it less understandable to enthusiast users.

---

# 63. Content Tone

Preferred:

```text
"Setup ini banyak dipakai Civic FD dengan spek serupa."
```

Avoid:

```text
"Kami menjamin setup ini 100% aman."
```

Preferred AI:

```text
"Kemungkinan besar cocok, tapi ride height dan fender clearance
masih perlu dicek."
```

---

# 64. Trust UI

Important trust indicators:

- Verified
- Community evidence count
- Solved case count
- Last updated
- Confidence
- Vehicle match

Avoid using:

- follower count
- likes
- viral badges

as primary technical trust indicators.

---

# 65. Data Visualization

Preferred:

- Horizontal bars
- Range charts
- Timeline
- Progress
- Simple line chart

Avoid:

- Gauge dashboard everywhere
- Speedometer-style charts
- 3D charts
- Decorative racing telemetry without purpose

---

# 66. Design Tokens — CSS Reference

```css
:root {
  --am-graphite-950: #0F141A;
  --am-graphite-900: #151B22;
  --am-graphite-800: #1D232A;
  --am-graphite-700: #2B323B;
  --am-graphite-600: #3C4550;

  --am-orange-700: #C93413;
  --am-orange-600: #DC3E17;
  --am-orange-500: #ED491C;
  --am-orange-400: #F45C32;
  --am-orange-300: #FF805E;

  --am-bg: #F7F8FA;
  --am-surface: #FFFFFF;
  --am-surface-subtle: #F1F3F5;

  --am-text: #171C22;
  --am-text-secondary: #5D6670;
  --am-text-tertiary: #616A74;

  --am-border: #E3E6E9;
  --am-border-strong: #CDD2D7;

  --am-success: #168A52;
  --am-warning: #D58A00;
  --am-danger: #D63B3B;
  --am-info: #2678D9;

  --am-radius-sm: 8px;
  --am-radius-md: 12px;
  --am-radius-lg: 16px;
  --am-radius-xl: 20px;
  --am-radius-2xl: 28px;
}
```

### Material — mobile only

Not emitted by `packages/tokens`' CSS build (`scripts/build.mjs`) — `apps/mobile`
reads these directly from `src/tokens.js`. Recorded here as reference (§77 Material
System has the full matrix).

```text
chrome:   tint #FBFCFD, 80% coverage  ->  composite #FAFBFC
surface:  tint #FCFDFD, 92% coverage  ->  composite #FCFDFD
working:  #FFFFFF, solid

edge highlight:    rgba(255, 255, 255, 0.90)
edge inset shadow: inset 0 -8px 12px -8px rgba(15, 20, 26, 0.14)
ground stops:      #FFFFFF @ 0, #F7F8FA @ 0.45, #EFF2F5 @ 1
```

---

# 67. Dark Theme Tokens — CSS Reference

```css
[data-theme="dark"] {
  --am-bg: #0E1217;
  --am-surface: #151A20;
  --am-surface-subtle: #202730;

  --am-text: #F5F7F8;
  --am-text-secondary: #AAB2BA;
  --am-text-tertiary: #8E98A2;

  --am-border: #29313A;
  --am-border-strong: #3A434E;
}
```

### Material — mobile only

```text
chrome:   tint #0E1217, 80% coverage  ->  composite #0E1217
surface:  tint #151A20, 92% coverage  ->  composite #14191F
working:  #151A20, solid

edge highlight:    rgba(255, 255, 255, 0.14)
edge inset shadow: inset 0 -8px 12px -8px rgba(0, 0, 0, 0.45)
ground stops:      #151B22 @ 0, #0E1217 @ 0.45, #0B0E12 @ 1
```

---

# 68. Flutter Theme Reference

Flutter implementation should expose semantic tokens rather than raw colors throughout widgets.

Concept:

```text
AnakMobilColors
AnakMobilTypography
AnakMobilSpacing
AnakMobilRadius
AnakMobilTheme
```

Do not scatter:

```dart
Color(0xFFED491C)
```

through feature code.

Use centralized tokens.

---

# 69. Component Naming

Recommended design-system naming:

```text
AmButton
AmCard
AmChip
AmBadge
AmTextField
AmVehicleCard
AmBuildCard
AmProblemCard
AmServiceCard
AmFitmentCard
AmConfidenceBadge
AmEvidenceCard
AmEmptyState
AmBottomSheet
AmAppBar
```

Feature-specific components can extend these primitives.

---

# 70. Mobile Screen Priority

V1 design priority:

```text
P0
Onboarding
Add Vehicle
Home
My Garage
Vehicle Detail
Build
Service
Problems
AI

P1
Explore
Community

P2
Public sharing / events
```

---

# 71. Design QA Checklist

Before a screen is considered ready:

- [ ] Is the active vehicle context obvious?
- [ ] Does the page have one clear primary action?
- [ ] Is orange used intentionally rather than everywhere?
- [ ] Are vehicle specs easy to scan?
- [ ] Is technical evidence visible when relevant?
- [ ] Are AI confidence and warnings clear?
- [ ] Does it work in light mode?
- [ ] Does it work in dark mode?
- [ ] Are touch targets at least 44 px?
- [ ] Are empty/loading/error states designed?
- [ ] Are sensitive data protected?
- [ ] Does the screen still make sense without social engagement?

---

# 72. Design Anti-Patterns

Do not implement:

### "Racing Game Dashboard"

```text
Huge tachometer
Fake gauges
Neon speed lines
Carbon texture everywhere
```

### "Marketplace Homepage"

```text
Promo banner
Promo banner
Product grid
Ads
Ads
```

### "Generic Social Feed"

```text
Post
Like
Comment
Repeat
```

### "AI Everything"

Do not put:

```text
Ask AI
Generate AI
Summarize AI
AI
AI
AI
```

on every component.

AI should appear when it actually helps a decision.

### "Glassmorphism Tempelan"

```text
Four-sided white border on a glass surface
Uniform transparency across chrome/surface/working roles
Glass on warnings or data (AI safety warnings, confidence badges, service records)
Blur treated as the identity, not the edge
Decorative refraction blobs
```

---

# 73. Signature AnakMobil Components

The following should become visually recognizable as AnakMobil:

1. **Vehicle Identity Card**
2. **Build Spec Card**
3. **Fitment Result Card**
4. **AI Evidence Card**
5. **Repair Intelligence Card**
6. **Service Timeline**
7. **Community Vehicle Match**
8. **Garage Evidence Profile**

If these components are strong, the product will feel distinct even without decorative branding.

---

# 74. Recommended Initial Visual Style

### Light

```text
Background: #F7F8FA
Cards:      #FFFFFF
Text:       #1D232A
Accent:     #ED491C
```

### Dark

```text
Background: #0E1217
Cards:      #151A20
Text:       #F5F7F8
Accent:     #ED491C
```

### Ground & Material

```text
Ground:   graphite gradient, optional vehicle-colour tint (§77)
Material: chrome / surface / working, distinguished by coverage (§77)
```

### Overall Ratio

```text
Neutral / Graphite : 85%
Orange Accent      : 10%
Semantic Colors    : 5%
```

Scoped to **UI tokens** — imagery is excluded. A red or yellow car in a vehicle hero
would otherwise make the ratio false on its own.

---

# 75. Product Design Thesis

AnakMobil.id should visually communicate:

> **"App ini ngerti mobil gue."**

Not:

> "App ini tentang mobil."

That difference is essential.

The user should always feel that the interface understands:

```text
MY CAR
MY BUILD
MY HISTORY
MY COMMUNITY
MY QUESTIONS
```

The logo establishes the brand through:

```text
GARAGE
+
A / AM
+
ROAD
+
MOVEMENT
```

The product UI continues that identity through:

```text
GRAPHITE
+
ORANGE
+
STRUCTURED AUTOMOTIVE DATA
+
STRONG VEHICLE PHOTOGRAPHY
+
EVIDENCE-BASED AI
```

---

# 76. Final Direction

**Brand:** AnakMobil.id  
**Primary Color:** Graphite `#1D232A`  
**Accent:** AnakMobil Orange `#ED491C`  
**Primary Typeface:** Inter  
**Core Radius:** 16 px  
**Primary Mobile Approach:** Clean card-based automotive utility  
**Preferred Theme:** Both light and dark; dark particularly important for automotive imagery  
**AI Style:** Grounded, structured, evidence-first  
**Social Style:** Vehicle-context-first, not feed-first  
**Material (mobile):** Glass — three roles by coverage, never by blur; solid wherever
a decision is made (§77)  

### Brand line

> **Mobil lo. Build lo. Komunitas lo.**

### Design line

> **Your car is the interface.**

---

# 77. Material System

Mobile only (`apps/mobile`). The single reference for the three roles, the contrast
contract, the full matrix, the edge, the ground, and the platform ladder. Introduced
by AM-15, 2026-08-19; every value here is `packages/tokens/src/tokens.js`, checked by
`packages/tokens/test/material.test.mjs`.

## Three roles

One material, three roles, distinguished by how much of the surface its tint covers —
never by blur radius.

| Role | Where it lives | Text allowed |
|---|---|---|
| `chrome` | app bar, tab bar, floating AI entry — the most glass | primary only |
| `surface` | content cards, sheets, list panels — high coverage, reads as workshop milk-glass | primary, secondary |
| `working` | service history, fitment results, forms, AI evidence, AI warnings, confidence badges, the eight §73 signature components — **solid, zero transparency** | primary, secondary, tertiary |

`working` is not a fallback or a degradation — it is the material for everything read
to make a decision, often outdoors in direct Indonesian sun. A surface whose contrast
varies with whatever is behind it is the wrong material for a service record.

## The contrast contract

**The binding token is the composited colour that passes WCAG AA — never an opacity
value.** Percentages describe how a surface ends up looking; they are rendering
inputs, not the contract. Two surfaces at the same coverage over different grounds
are different colours, and only one may pass. Secondary and tertiary text may never
sit on a material whose backdrop is unknown — they belong on `surface` (secondary
only) or `working`. If a design ever needs a coverage that breaks the contract, the
surface becomes solid; the contract wins, always.

## The matrix

`solid` is the composite over the app's own ground and is what renders whenever
transparency is unavailable — every Android below SDK 31, and any device with Reduce
Transparency on.

| Role | tint | coverage | solid | overWhite | overBlack | text allowed |
|---|---|---|---|---|---|---|
| `dark.chrome` | `#0E1217` | 80% | `#0E1217` | `#3E4145` | `#0B0E12` | primary |
| `dark.surface` | `#151A20` | 92% | `#14191F` | `#282C32` | `#13181D` | primary, secondary |
| `dark.working` | `#151A20` | 100% | `#151A20` | — | — | primary, secondary, tertiary |
| `light.chrome` | `#FBFCFD` | 80% | `#FAFBFC` | `#FCFDFD` | `#C9CACA` | primary |
| `light.surface` | `#FCFDFD` | 92% | `#FCFDFD` | `#FCFDFD` | `#E8E9E9` | primary, secondary |
| `light.working` | `#FFFFFF` | 100% | `#FFFFFF` | — | — | primary, secondary, tertiary |

Every allowed pair, at every backdrop (`overWhite / overBlack / overGround`). **All
pairs pass 4.5:1:**

| Role | primary | secondary | tertiary |
|---|---|---|---|
| `dark.chrome` | 9.55 / 18.00 / 17.49 | not allowed | not allowed |
| `dark.surface` | 13.06 / 16.62 / 16.44 | 6.54 / 8.32 / 8.23 | not allowed |
| `dark.working` | 16.28 | 8.15 | 5.97 |
| `light.chrome` | 16.81 / 10.43 / 16.54 | not allowed | not allowed |
| `light.surface` | 16.81 / 14.09 / 16.81 | 5.72 / 4.80 / 5.72 | not allowed |
| `light.working` | 17.13 | 5.83 | 5.49 |

`light.surface` tints off-white (`#FCFDFD`) rather than the subtle grey used
elsewhere in this document (`#F1F3F5`, §41): at 92% coverage that grey gives
secondary text only 4.40:1 over a dark backdrop — a fail. The edge, not the fill, is
what separates a light-mode surface from the page.

## The edge

Glass gets its form from its edge, not from being see-through: a 1px highlight on
the **top edge only**, plus an inset shadow at the bottom for thickness — the
instrument-panel read.

```text
Light: highlight rgba(255,255,255,0.90)  inset shadow 0 -8px 12px -8px rgba(15,20,26,0.14)
Dark:  highlight rgba(255,255,255,0.14)  inset shadow 0 -8px 12px -8px rgba(0,0,0,0.45)
```

**Forbidden, by name: a uniform 1px white border on all four sides.** It is the
single most recognisable signature of templated glassmorphism.

## The ground

A two-stop graphite gradient, tinted with the dominant colour of the active vehicle
when there is one — pure code: no image asset, no blur, no photograph. The middle
stop is the theme's own background, and it is the backdrop each role's `solid`
composites over.

```text
Light stops: #FFFFFF @ 0, #F7F8FA @ 0.45, #EFF2F5 @ 1  (tint strength 0.08)
Dark stops:  #151B22 @ 0, #0E1217 @ 0.45, #0B0E12 @ 1  (tint strength 0.14)
```

**Boundary:** `AmGround` accepts a tint it is given and falls back to neutral
graphite when there is none. Where that tint comes from is not built here — colour
extraction from a vehicle photo is deferred to the garage epic.

## Platform ladder — written honestly

Live blur is an enhancement, never the foundation. `expo-blur` on Android defaults to
`blurMethod: 'none'`, which is a tint, not a blur; a real blur needs Android 12+.

| Target | Rendering |
|---|---|
| iOS ≥ 26 | `expo-glass-effect` (`GlassView`), native Liquid Glass |
| iOS < 26 | `expo-blur` over `UIVisualEffectView` — native and cheap |
| Android ≥ 31 | `expo-blur` with `dimezisBlurViewSdk31Plus`, chrome only |
| Android < 31 | tint + edge, no blur — and this must look correct, not broken |

**This repository ships no blur library at all.** `chrome` has no consumer in AM-15
— the app bar and tab bar belong to the app-shell story — so `expo-blur` would be a
dependency bought for a surface that does not exist yet. Every role renders tint +
edge today; the app-shell story adds `expo-blur` when it has a `chrome` surface to
put it on.
