# @anakmobil/landing

The public page at anakmobil.id. Astro, static, no server.

## Run it

```bash
make fe-dev        # dev server on :4321
make fe-check      # type check + build — the gate
make fe-preview    # serve the built output, which is what Lighthouse must measure
```

Never measure performance against `fe-dev`. The dev server ships unminified code and a hot-reload socket, and the score it produces is fiction in both directions.

## Why Astro

AM-341 AC6 says only the waitlist form may send JavaScript to the browser. In a React app that is a rule someone has to keep enforcing; in Astro it is the default — components render to HTML at build time and hydrate only when a `client:*` directive asks them to.

AC4 falls out of the same property. A crawler gets the finished HTML, not an empty shell waiting on a bundle.

## What is here now

A holding page. It says what AnakMobil is and admits it is not open yet.

AM-341 replaces it with the real page — hero, product sections, store links, and the waitlist form. The form and the storage behind it are AM-346, and until that exists the alternative is a signup button that accepts an email and drops it.

## Structure

```
src/
  layouts/Base.astro     document shell — title, description, canonical, OG
  pages/index.astro      one page per file, routed by filename
  styles/global.css      the token import, the font, and the reset
public/                  copied verbatim to the site root
```

## Rules that are easy to break

**The token import stays the first statement in `global.css`.** CSS silently drops an `@import` that follows any other rule. The build only warns, the page still compiles, and every colour resolves to nothing. This exact mistake was made while writing this file — the `@font-face` block sat above the import, and the whole `:root` palette vanished from the output.

**No hex codes, pixel values, or font stacks in a component.** They come from `@anakmobil/tokens`. If a value is missing, add it there. `make fe-build` regenerates the tokens first so a stale `dist/` cannot ship yesterday's palette.

**Inter is self-hosted, and the family is named `Inter`, not `Inter Variable`.** The `@fontsource-variable` package names it the latter, which does not match the token stack — the font would download and then go unused. That failure is invisible on any machine with Inter installed locally, which is most developer machines.

**A link that 404s is worse than an absent link.** `og:image`, `twitter:card` as `summary_large_image`, and the `Sitemap:` line in `robots.txt` are all deliberately missing until the files they point at exist.

## Not installed yet, and why

- **`@astrojs/react`** — arrives with the waitlist island, the only part of the page that needs a browser runtime.
- **`@astrojs/sitemap`** — worth a line once there is a second page.
- **A font preload** — the stylesheet is inlined, so the browser sees the `@font-face` in the first response. A preload saves one round trip on top of that; add it if a real Lighthouse trace says the font blocks LCP, not before.
