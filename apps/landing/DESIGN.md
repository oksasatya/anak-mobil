# Landing Page — Design Brief

A brief for designing the public landing page at **anakmobil.id** (Jira [AM-341](https://oksasatyaa.atlassian.net/browse/AM-341), epic E15). Hand this to a design tool or agent; it is self-contained. The global design system lives in [`docs/design.md`](../../docs/design.md) and the committed tokens in [`packages/tokens`](../../packages/tokens) — this brief does not replace them, it applies them to one page.

Copy that appears on the page is quoted in **Bahasa Indonesia**, because that is what ships. Everything else — rationale, structure, constraints — is in English, because this is a developer/design document.

---

## 1. The product, in one breath

AnakMobil.id is a **digital garage and community for car owners in Indonesia**: keep a service history that survives selling the car, record a build as structured data (not a photo album), and ask people who fitted the same part to the same car. There is also a grounded AI assistant, but the landing page does not lead with AI — the brand rule is *"hindari klaim AI berlebihan."*

**Who finds this page:** someone who searched, followed a shared link, or clicked an ad. A daily-driver owner, an enthusiast, a builder. They have **five seconds** to understand what this is and decide whether to care.

## 2. This page has one job, and it is not "sell the app"

The app is not in the stores yet. So the honest primary action is **join the waitlist**, not a download button that goes nowhere.

This is the single most important framing decision, and it comes from a product rule with teeth:

> **The platform launches empty and says so.** No invented community counts, no fabricated testimonials, no screenshots of activity that never happened. The low-data state is designed as a primary experience, not a fallback.

So this page must be **compelling without lying**. It cannot show "12,000 builds shared" or fake reviews. It sells the *idea* and the *intent* — what your garage will hold, why a service history that outlives the car matters, what it feels like to find someone who fitted your exact wheel — and it asks the visitor to be early. The empty state is the pitch: *jadi yang pertama.*

**A dead control costs more trust than an empty page.** No button that does nothing. If the waitlist form's backend is not ready (it is [AM-346](https://oksasatyaa.atlassian.net/browse/AM-346), and does not exist yet), the form is still designed here but wired only when that lands — never shipped as a decorative input.

## 3. Brand direction — the feel

From `docs/design.md`, condensed. Design to this, not to a generic SaaS template.

**It should feel like:** a modern automotive product. Confident, clean, technical-but-usable, premium without being flashy. Dark graphite is the primary identity; a single orange-red accent carries energy, used sparingly.

**Core statement:** *Digital garage yang modern, cepat, dan hidup.*

**Three ideas in the brand, worth echoing visually on the page:**
- **Garage / shelter** — the logo's outline reads as a roof. AnakMobil is a *home* for your car. Warmth and belonging, not a cold database.
- **Road / motion** — the orange-red line is a lane, a racing line, a modification journey. Progression, movement.
- **Letterform** — a geometric A / AM monogram.

**Personality dials:**

| Trait | Direction |
|---|---|
| Modern | clean surfaces, strong typography, minimal ornament |
| Automotive | technical specs, structured data, strong imagery |
| Energetic | orange-red accent, controlled |
| Trustworthy | no gimmicky UI, no overblown AI claims |
| Community | owner / build / community always feel connected |
| Premium | generous spacing, strong hierarchy, uncluttered |

**It is emphatically NOT:** a racing-game UI, a cyberpunk dashboard, a banner-choked marketplace, a 2008 forum, a social-media clone, or a corporate workshop POS. If a choice drifts toward any of those, it is wrong.

## 4. The committed tokens — do not invent new ones

These are already decided and generated into `packages/tokens`. Use them; do not introduce a second colour language. Light and dark are both first-class — design both.

**Brand**
- Graphite `#1D232A` — primary identity
- AnakMobil Orange `#ED491C` — the one accent; primary actions, selected state, the "road" line, key highlights only

**Light theme**
- background `#F7F8FA` · surface `#FFFFFF` · surface-subtle `#F1F3F5`
- text primary `#171C22` · secondary `#5D6670` · tertiary `#8A939D`
- border `#E3E6E9` · border-strong `#CDD2D7`

**Dark theme** (the brand's home ground)
- background `#0E1217` · surface `#151A20` · surface-subtle `#202730` · surface-raised `#1B2128`
- text primary `#F5F7F8` · secondary `#AAB2BA` · tertiary `#737D87`
- border `#29313A` · border-strong `#3A434E`

**Semantic** (separate from the accent — orange is never a status colour)
- success `#168A52` · warning `#D58A00` · danger `#D63B3B` · info `#2678D9`

**Typography** — **Inter** (`--font-sans`). This is the committed UI typeface; keep it. Desktop type scale (size / line-height / weight):

```
Hero   56 / 64 / 750     H1  44 / 52 / 700     H2  36 / 44 / 700
H3     28 / 36 / 650     Title 20 / 28 / 650   Body 16 / 25 / 400   Small 14 / 21 / 400
```

For any automotive figure — `225/40 R18`, `146.120 KM`, `18×8.5 ET40`, `Rp 14.500.000` — use **tabular numbers**, medium/semibold, high contrast. Specs must scan. This is a signature of the brand and the landing should use at least one real spec-styled number to signal it.

**Radius** xs 6 · sm 8 · md 12 · lg 16 · xl 20 · pill 999 — cards lean on `lg` (16px), buttons on `md` (12px).

**Layout** page-padding 16px · container-max 1280px · touch-target-min **44px** (hard floor on every tappable thing).

**Ornament discipline:** prefer borders over shadows; reserve soft shadows for genuinely raised things (a floating waitlist card). No blanket glassmorphism, no gradient-mesh hero.

## 5. Page structure — section by section

Mobile-first: design the phone layout first, then let it breathe on desktop. Each section says what it must prove and gives copy direction. Copy is a *direction*, not final — sharpen it, keep it honest, keep it Indonesian, active voice.

**A · Hero (above the fold, AC1 — the value must land in one glance)**
The single most important frame. In one look the visitor must know: this is *for car owners*, and *what they can do here*. Lead with the most characteristic thing in the product's world — a garage that holds your car's real history, or a build described in numbers.
- Wordmark: `AnakMobil.id` (the `.` can carry the orange accent, as the holding page does).
- Headline direction: *"Garasi digital buat mobil kamu."* — plain, confident, no jargon.
- Sub: what makes it different, concretely — *"Riwayat servis yang tidak hilang saat mobilnya dijual. Catatan build yang rapi. Dan orang yang pernah pasang part yang sama di mobil yang sama."*
- Primary CTA: **the waitlist** (see AC2/AC3 below for the platform-aware behaviour).
- Honest pre-launch line, small, near the CTA: *"Masih dibangun. Daftar sekarang, jadi yang pertama tahu waktu pendaftaran dibuka."*
- A hero visual is welcome but must not be a fake screenshot of populated data. Prefer an on-brand abstract (the garage-roof / road-line motif), an honest empty-state mock, or strong automotive photography — never invented content presented as real.

**B · What you get — three concrete capabilities (the pitch, no fake proof)**
Three ideas, each a real capability, each answering "why care". Suggested trio, matching the product's actual pillars:
1. **Riwayat servis yang bertahan** — *"Catat servis sekali. Tetap ada meski mobilnya pindah tangan."* The service history that outlives the car.
2. **Build sebagai data, bukan album foto** — *"Offset, PCD, ukuran ban — tersimpan sebagai angka yang bisa dibandingkan."* Show a spec-styled number here (`225/40 R18`, `18×8.5 ET40`) to make the "structured, not a photo" point visually.
3. **Orang yang pernah di posisi kamu** — *"Tanya yang sudah pasang part yang sama di mobil yang sama."* Community grounded in the same car.
Present as three clean cards or a structured row — not three identical centred feature cards (an AI-slop tell). Give them different visual weight if one matters more.

**C · The honest early-days section (turns "empty" into "be first")**
One short section that names the cold start and reframes it as an invitation. This is where the *"launch empty and says so"* rule becomes a selling point rather than an apology. Direction: *"Komunitasnya baru mulai. Yang pertama masuk yang menentukan bentuknya."* No fabricated numbers anywhere near this.

**D · Waitlist (the one interactive island — AC3, AC6)**
A focused signup. Per AM-346 the fields are: **email (required)** and an **optional "sebut mobilmu"** (name your car) — one line, not a form to fill out. After success, a clear confirmation state — *"Terdaftar. Kami kabari begitu pendaftaran dibuka."*
- Email is personal data: collect it with **clear consent stated in the form**, in plain readable language, not buried in a document nobody opens.
- Design all four states: idle, submitting, success, error (including the friendly "kamu sudah terdaftar" for a duplicate — AM-346 dedupes silently, so a repeat is a soft confirmation, not an error).
- Bot protection is expected (AM-346), but it must be invisible to a real person — no CAPTCHA that a genuine owner has to solve.
- This is the *only* part of the page that ships JavaScript (a React island). Everything else is static.
- Its backend is AM-346 and does not exist yet — design the full form and its states, but it is wired live only when AM-346 lands. Until then it is not shipped as a dead control.

**E · Store CTA / footer (AC2)**
Platform-aware primary action, and the honest fallback:
- On iPhone → App Store. On Android → Play Store. On desktop → show both.
- **Before the app is in the stores**, this reverts to the waitlist (AC3) — no dead store links.
- Footer: wordmark, a one-line what-this-is, contact, and a link to a real privacy note (email is stored — say so). Nothing invented.

## 6. The eight acceptance criteria, as design constraints

Every one of these is testable and several are hard gates. Design so they pass, not so they look passed.

| AC | Design constraint |
|---|---|
| **AC1** value in one glance | The hero, unscrolled, tells a first-time visitor it is for car owners and what they can do. |
| **AC2** CTA by platform | Store link resolves to the visitor's platform; desktop shows both; pre-launch → waitlist. |
| **AC3** waitlist is the pre-launch CTA | While the app is not in stores, the primary action is signup, with a clear confirmation. No download link that fails. |
| **AC4** full HTML for crawlers *(hard gate)* | All meaningful text is in the server-rendered HTML — title, meta description, OG/Twitter image, and schema.org JSON-LD present. Not an empty shell hydrated later. |
| **AC5** fast on Indonesian mobile *(hard gate)* | Core Web Vitals in the "good" band on a mid-range phone on cellular. Images sized to the viewport (responsive `srcset`), not full-res. |
| **AC6** JS only where needed | Only the waitlist island ships JavaScript. Static sections hydrate nothing. |
| **AC7** small screens first | Reads with no sideways scroll on a phone; touch targets ≥ 44×44px. |
| **AC8** same face as the app | Colour, type, and shape come from `packages/tokens`. The landing and the app must feel like one product. |

## 7. Hard technical constraints (they shape the design, so they are here)

- **Astro, zero-JS by default.** The static page ships no JavaScript; the waitlist is a React island, the single exception. Design accordingly — no reliance on client-side interactivity outside that form.
- **This is a public, indexable surface**, so the SEO gate is mandatory (unlike the behind-auth app): real HTML, meta/OG/Twitter, JSON-LD, canonical, sitemap. AC4 is this, made concrete.
- **Performance is a gate, not an aspiration.** Measure the production build, target CWV "good" and a Lighthouse ceiling on mobile. Heavy hero media is the usual killer — keep it light or lazy.
- **Both themes** are first-class; design light and dark from the tokens, don't invert naïvely.
- **No fake data, anywhere.** This is the one rule that overrides any "make it look impressive" instinct. Impressive-and-honest, or not at all.

## 8. Out of scope for this page

Blog and editorial content, a pricing page (E14, price not decided), the workshop portal, the seller portal, and multi-language. The landing is one page, done well.

---

*Grounded in `docs/design.md` and `packages/tokens` as of 2026-08-17. If a token value here disagrees with `packages/tokens`, the package wins — it is the generated source of truth.*
