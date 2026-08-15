// @ts-check
import { defineConfig } from "astro/config";

/**
 * Astro ships zero kilobytes of JavaScript unless a component asks for it.
 * That is the whole reason this surface is Astro and not a React app: AM-341
 * AC6 says only the waitlist form may hydrate, and here that is the default
 * rather than something to enforce.
 *
 * Deliberately not configured yet:
 *   - @astrojs/react   — arrives with the waitlist island, which is the only
 *                        part of the page that needs a browser runtime.
 *   - @astrojs/sitemap — worth a line once there is more than one page.
 *
 * `site` is set now because canonical URLs, Open Graph tags, and any future
 * sitemap all read from it, and a wrong value there is invisible until a
 * crawler gets it wrong.
 */
export default defineConfig({
  site: "https://anakmobil.id",
  trailingSlash: "never",
  build: {
    // One stylesheet in <head> beats a request per component. The page is
    // small enough that inlining costs less than the round trips would.
    inlineStylesheets: "always",
  },
});
