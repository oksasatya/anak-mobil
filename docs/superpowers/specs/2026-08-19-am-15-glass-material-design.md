# AM-15 — A glass material system for the mobile app

Ticket: [AM-15](https://oksasatyaa.atlassian.net/browse/AM-15) · Epic E0 — Fondasi & App Shell · design approved 2026-08-19

The owner asked for "glass modern, but not AI slop", applied comprehensively, in both themes. That is one request, not two, and the whole of this spec is the answer to it: **what kind of glass is not the template everyone else ships.**

**"Comprehensively" is worth pinning down before it misleads.** It means one material system draws every surface in the app — there is no second, unrelated visual language hiding in some corner. It does **not** mean every surface is transparent. One of the three roles below is deliberately opaque, and the section after next explains why that is the design rather than an exception to it.

This design was attacked twice before it was written down. A cross-model adversarial pass (Codex) returned a **BLOCK** on the first version and is why the material is defined by coverage rather than transparency. A grill against this repository's own `docs/design.md` then found six places where the new material contradicts decisions the document had already made — including the one the owner had just chosen. Both are recorded in full at the end, because a design that survived something should say what.

## What this changes, stated plainly

**This replaces the mobile material system. It is not a continuity edit, and pretending otherwise would be dishonest.** `docs/design.md` currently says cards are solid, "use borders before shadows", shadows are for floating elements only, and "avoid glossy dashboard UI". A pervasive glass material reverses three of those. The document is therefore revised explicitly (section list below) rather than quietly extended.

One consequence follows immediately. The 85 / 10 / 5 ratio (neutral / orange / semantic) **cannot** mean a proportion of screen pixels once vehicle imagery and a tinted ground are present — a red car makes the claim false on its own. The ratio survives as a rule about **UI tokens**, with imagery excluded, and §74 is edited to say so.

## The core decision: coverage, not transparency

The first version of this design claimed a scrim would guarantee WCAG AA no matter what passed through the glass. That claim is false, and the arithmetic is worth keeping because it is what shaped everything else.

Primary text on dark is `#F5F7F8`, relative luminance **0.9271**. For a 4.5:1 ratio it needs a background at luminance **≤ 0.1671**. If the backdrop can be a white car in direct sun, a black scrim must reach **55.4% opacity** to hold that ratio. Blur does not rescue it: a large white region stays white after blurring.

**These figures were wrong in the first version of this spec and are corrected here**, because the mistake is instructive. They were computed by treating the required *luminance* as if it were an sRGB alpha (`α = 1 − L`), which gave 83.3 / 94.1 / 99.4 %. Alpha compositing happens in gamma-encoded sRGB, not in linear light, so the alpha must be solved for the sRGB grey whose luminance hits the target. Verified by compositing back: black at 55.4 % over white yields exactly 4.50:1.

**And primary text is the easy case.** Computed the same way, against the same white backdrop:

| Text role | Luminance | Max background L for 4.5:1 | Black scrim required over white |
|---|---|---|---|
| primary `#F5F7F8` | 0.9271 | 0.1671 | 55.4% |
| secondary `#AAB2BA` | 0.4393 | 0.0587 | 73.1% |
| tertiary `#737D87` | 0.2006 | 0.0057 | **93.3%** |

Tertiary text needs a scrim that is opaque in all but name — and primary text does not, which is the part worth keeping. A surface carrying only primary text can be genuinely translucent; it is the quiet text that forces opacity. So the rule is not "surfaces are mostly opaque". It is sharper and more useful than that:

**Secondary and tertiary text may never sit on a material whose backdrop is unknown.** They belong on `working`, or on a `surface` whose composited token is already fixed. Where a translucent role genuinely needs a caption — a tab bar label — it uses the primary text token, or the surface under it becomes solid. There is no third option that passes.

So an honest "glass everywhere" is **opaque in proportion to how quiet its text is**. That is not a compromise imposed on the idea; it *is* the idea, and it happens to be the thing that separates this from the slop, which is uniformly transparent precisely where it should not be.

**Therefore: the binding token is the composited colour that passes AA, never an opacity value.** Percentages quoted anywhere in this spec describe how a surface ends up looking; they are not the contract and must never be implemented as the source of truth. Two surfaces at the same opacity over different grounds are different colours, and only one of them may pass. Opacity and blur are how a surface is *rendered*; the contrast contract is settled at the token, once, against the worst case. The backdrop is decoration. If a design ever needs an opacity that would break the contract, the surface becomes solid — the contract wins, always.

## Three material roles

One material, three roles, distinguished by how much they cover — not by three arbitrary blur radii.

| Role | Where it lives | Character | Text contrast |
|---|---|---|---|
| `chrome` | app bar, tab bar, floating AI entry | the most glass — moderate tint, largest blur where blur exists | short labels only; each label carries a local scrim |
| `surface` | content cards, sheets, list panels | high coverage — around 88–92% tint in practice; reads as workshop milk-glass, not a window | composited token, AA guaranteed |
| `working` | service history, fitment results, forms, AI evidence, AI warnings, confidence badges, the eight signature components | **solid. Zero transparency.** | plain token pairs |

`working` is not a fallback or a degradation. It is the material for everything that is **read to make a decision**, and it is solid on purpose: these screens are used outdoors, at a workshop, in direct Indonesian sun, which is the single most common real usage condition for this product. A surface whose contrast varies with whatever is behind it is the wrong material for a service record.

## The ground

The bottom layer of the app is a graphite gradient **tinted with the dominant colour of the active vehicle**. A red car gives a copper-tinted garage; a white car gives a cool one.

This is deliberately *not* the vehicle photograph. §47 of the design document requires "preserve vehicle color" and "avoid excessive filters", and a heavily blurred, scrimmed photo violates both — it turns the car into a grey smear, which is also how "your car is the interface" quietly becomes "your car is wallpaper". The tint keeps what the photo contributes (this garage belongs to *this* car, and no two users see the same one) and discards what it costs.

What it costs is not small. A 4000×3000 photo decodes to roughly 46 MiB; twenty cached vehicles approach a gigabyte before any render buffers. A tint is one or two colour values.

**The ground is pure code** — a gradient plus fine grain, no image asset, no blur. That is what makes it identical on an iPhone and on an Android 10 phone, which matters more than it sounds (see the platform ladder).

**Boundary:** this spec defines the ground's *contract* — it accepts one optional tint colour and falls back to neutral graphite when there is none. **Where that colour comes from is not built here.** Extraction (server-side on upload, or client-side) is decided when vehicle photos actually exist, in the garage epic. Building an extraction pipeline now, against a photo the app cannot yet receive, would be inventing a consumer for data that does not exist.

## The edge

Glass gets its form from its edge, not from being see-through.

- A 1px highlight on the **top edge only**, as if light caught the lip.
- An inset shadow at the **bottom**, giving the surface thickness — the instrument-panel read.
- **Forbidden: a uniform 1px white border on all four sides.** That is the single most recognisable signature of templated glassmorphism, and it is banned by name.

The edge is what makes this design survive its own platform ladder: it renders identically with or without blur.

## Photography is treated in the opposite direction

Vehicle photos stay **sharp**, and they stay where identity lives: the vehicle hero and the Selected Vehicle Card (§20 requires the photo to be dominant there — a card at 88–92% tint would gut the product's most important component). A scrim appears only directly under overlaid text, sized to that text, not spread across the image.

So: blur is never applied to a vehicle photo. Ambient softness belongs to the ground, which is not a photo.

## What glass may never touch

These are not style preferences. Each traces to a rule that already exists.

- **AI safety warnings.** The repository rule is that an AI answer never ships without its warning, prominently. Prominence cannot be guaranteed on a surface whose backdrop changes. Warnings are `working` — solid, always.
- **AI confidence badges.** §9 assigns confidence to semantic colours, and §61 forbids communicating status by colour alone. On a variable backdrop, a semantic colour shifts in perception. Confidence badges are solid, and carry a label or shape as well as a colour.
- **The eight signature components** (§73: Vehicle Identity Card, Build Spec Card, Fitment Result Card, AI Evidence Card, Repair Intelligence Card, Service Timeline, Community Vehicle Match, Garage Evidence Profile). The document's own thesis is that these make the product recognisable *"even without decorative branding"*. Glass is decorative branding. Letting it take over the components that carry the identity would move the identity into the decoration — precisely backwards.
- **Orange.** Orange is never the glass. It is content *on* glass (an active tab icon) or a solid fill (a primary action), never the material itself. This preserves the ratio and creates a legible hierarchy: **pressable is solid, container is glass.** And no glow — §50 forbids "constant glowing orange effects", which the first mockup of this design violated.
- **Skeletons** (§51) are drawn on the same material as the component they stand in for, so a loading card does not shimmer against a backdrop that swallows it.

## Platform ladder — written honestly

Live blur is an **enhancement, never the foundation**. The reason is specific and verified from Expo's own source:

- `expo-blur` on Android defaults to `blurMethod: 'none'`, and `'none'` is **not a blur** — it renders a semi-transparent tint.
- `'dimezisBlurView'` is documented to "lead to decreased performance on Android SDK 30 and below".
- `'dimezisBlurViewSdk31Plus'` gives a real blur only on Android 12+, and falls back to `'none'` below that.

A large share of the Indonesian Android market will therefore never see a blur. That is a platform boundary, not a choice.

| Target | Rendering |
|---|---|
| iOS ≥ 26 | `expo-glass-effect` (`GlassView`), native Liquid Glass — already installed, cheap |
| iOS < 26 | `expo-blur` over `UIVisualEffectView` — native and cheap on iOS |
| Android ≥ 31 | `expo-blur` with `dimezisBlurViewSdk31Plus`, **chrome only**, after measuring on a real mid-range device |
| Android < 31 | tint + edge, no blur — and this must look correct, not broken |

Because the ground is a gradient rather than a photo, and because the edge does the shaping, the no-blur rendering is a legitimate variant rather than a degraded one. **This is the constraint that makes the design honest across platforms**, and it is why the material was defined as tint + edge in the first place.

**Not claimed:** that a solid composited token is visually identical to a blurred one. It is not — blur preserves local gradients from what is behind it, and a flat token does not. Where the two meet, the boundary is made deliberate (a divider, a panel edge), never pretended away.

## Changes to `docs/design.md`

| Section | Change |
|---|---|
| §15 Elevation | Rewritten as **Elevation & Material**. "Borders before shadows" is superseded for mobile and the reason is recorded. |
| §40 Dark Mode / §41 Light Mode | Two glass recipes, one per theme; light mode tints **dark**, never white-on-white. |
| §46 Cards | Default card becomes `surface`; the `working` exception is named. |
| §47 Image Treatment | Unchanged in substance, but gains an explicit note that the ground is a tint, not a filtered photo — recording why. |
| §50 Motion | Gains "no glow on glass"; existing "avoid constant glowing orange effects" reaffirmed. |
| §66 / §67 Token Reference | New glass tokens: composited colour per role per theme, edge highlight, inset, ground gradient stops, tint slot. |
| §72 Anti-Patterns | New entry: **"Glassmorphism Tempelan"** — four-sided white border, uniform transparency across roles, glass on warnings or data, blur as identity, decorative refraction blobs. |
| §74 Recommended Initial Visual Style | Ground + material added; the 85/10/5 ratio is scoped to UI tokens, imagery excluded. |
| §76 Final Direction | One line for the material direction. |
| New section | **Material System** — the single reference for the three roles, the contrast contract, and the platform ladder. |

## Anti-goals

- **No image assets are requested for this ticket.** The ground is code; the missing-photo placeholder is an SVG silhouette, which §48 already requires ("neutral silhouette", never a stock car that implies the wrong model).
- No blur on long scrolling lists, and no per-item blur anywhere.
- No stacking glass on glass beyond two layers.
- No animated blur, no animated glass, no refraction or "liquid" decoration.
- No colour-extraction pipeline (deferred to the garage epic — see the ground's boundary note).
- No feature-specific components. AM-15 builds primitives; `AmVehicleCard`, `AmFitmentCard`, and the rest belong to their own epics.
- No backoffice/table components (E13).

## Verification

- **Contrast:** every composited token × text-role pair is computed against both black and white extremes and recorded with its ratio. A pair that cannot reach AA makes its surface solid. `Reduce Transparency` / `prefers-reduced-transparency` is honoured and tested.
- **No-blur variant:** the component catalogue (AM-29) is reviewed with blur disabled, which is the Android < 31 reality. It must look intentional.
- **Touch targets:** ≥ 44×44 pt on every interactive primitive, without the caller having to add padding (§61, AM-15 AC3).
- **Ratio:** orange appears only on primary actions, selected states, AI highlights, and important markers — checked on the catalogue screen.
- **Device check:** the catalogue is opened on the iOS simulator and, before AM-15 is called done, on one real mid-range Android — the platform ladder's claims are unverified until then.

## What the two adversarial passes changed

Recorded so a later reader can tell which parts of this design were argued for rather than assumed.

**Cross-model (Codex) — returned BLOCK on version 1.** It killed the claim that a scrim guarantees AA for free (the arithmetic above is its calculation, re-verified here), rejected "a solid token looks like a blurred one" as false, put real numbers on the photo-ground memory cost, and named the outdoor-workshop failure mode that produced the `working` role. It also observed that the design was replacing the document's material system while claiming continuity — which is why this spec says so in its second paragraph. Its sharpest objection, "your car is the interface becomes your car is wallpaper", is the one the grill then confirmed from the document's own text.

**Grill against `docs/design.md` — six contradictions.** §47 forbade the blurred photo ground outright, which reversed a decision the owner had already made and had to be taken back to them. §20 showed a glass Selected Vehicle Card would gut the product's most important component. §29 plus the repository's own AI-warning rule forced warnings to be solid. §50 caught that the first mockup used a glowing orange shadow the document explicitly bans. §73 raised the deepest objection — that identity is supposed to come from the data components, not from decoration — which is why glass is kept out of them. §48 answered the asset question: the placeholder is a silhouette, so this ticket needs no generated imagery at all.
