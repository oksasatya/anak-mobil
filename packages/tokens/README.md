# @anakmobil/tokens

Colour, spacing, radius, and typography — defined once, consumed by three surfaces.

Values come from [`docs/design.md`](../../docs/design.md). This package is the machine-readable form of that document, not a second opinion about it.

## Why generate instead of copy

The landing page needs CSS custom properties. The backoffice needs a Tailwind v4 `@theme` block. The mobile app needs JavaScript numbers. Maintaining the same palette in three files means it stays identical until the first time someone changes one and forgets the others — and then the drift is invisible until you put two screens side by side.

One source, generated outputs. Change the orange in one place and all three surfaces move together.

## Use it

**Mobile (React Native)** — import the source directly:

```ts
import { accent, radius, spacing } from "@anakmobil/tokens";

const styles = StyleSheet.create({
  cta: {
    backgroundColor: accent[500],
    borderRadius: radius.md,
    paddingHorizontal: spacing[4],
    minHeight: 44,
  },
});
```

**Landing (Astro)** — import the custom properties:

```css
@import "@anakmobil/tokens/css";

.cta {
  background: var(--accent-500);
  border-radius: var(--radius-md);
  padding: var(--space-4);
}
```

**Backoffice (Tailwind v4)** — import the theme, then use the utilities it generates:

```css
@import "@anakmobil/tokens/theme";
```

```tsx
<button className="bg-accent-500 rounded-md px-4">Simpan</button>
```

## Change a token

```bash
cd packages/tokens
# edit src/tokens.js
npm run check      # regenerates dist/ and runs the tests
```

`dist/` is generated. Editing it directly works until the next build overwrites it, so don't.

## What the tests actually check

Not that the values are "right" — that is a design decision, and `docs/design.md` is where it is made. They check the properties that break silently:

- **Both themes define the same keys.** A key present in light but missing from dark renders as an unset variable — text the same colour as its background, in one theme only.
- **Orange is never reused as a status colour.** Orange means primary action or AI. A success state wearing it makes both meanings unreadable.
- **Spacing stays on the four-pixel base.** One value off the grid is how a layout starts looking subtly wrong without anyone being able to say why.
- **The generated CSS covers all three theme states.** An explicit choice stamps `data-theme` on the root; the default "system" setting stamps nothing, so `prefers-color-scheme` has to carry that case, and a toggle has to win in both directions.
- **The touch target meets 44 pt**, the floor in AM-273.

## The one rule worth repeating

Never write a hex code, a pixel value, or a font stack into a component. If a value is missing here, add it here.

That is what makes AM-342 — *"tokens used as shared values, not rewritten with numbers of their own"* — enforceable rather than aspirational.
