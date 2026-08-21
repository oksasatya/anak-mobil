/**
 * The Brandfetch client id, and it is deliberately public.
 *
 * `EXPO_PUBLIC_*` values are compiled into the bundle and anybody can unzip an
 * IPA and read them, which is why the repository rule is that nothing secret
 * goes into a mobile environment at all. This one qualifies: Brandfetch's Logo
 * Link is a client-side CDN and the id travels in the URL of every image the
 * app requests, so it is visible on the wire regardless. It authorises reading
 * public brand logos and nothing else.
 */
const CLIENT_ID = process.env.EXPO_PUBLIC_BRANDFETCH_CLIENT_ID ?? "";

/**
 * A brand's logo, at the size the caller asks for.
 *
 * The server stores a DOMAIN (`brands.logo_domain`) and each client composes
 * its own URL, so the CDN, the size, and the key stay a client decision — see
 * the migration that added the column.
 *
 * TWO SOURCES, and the difference is worth knowing before reading a screen:
 *
 *   * **Brandfetch Logo Link**, when `EXPO_PUBLIC_BRANDFETCH_CLIENT_ID` is
 *     set. This is the real thing: the brand's actual mark, at the requested
 *     size, on a transparent background. Without the id the CDN does not serve
 *     an image at all — it answers 302 to its own hotlinking documentation,
 *     which is why an unkeyed build shows no logos rather than broken ones.
 *   * **Google's favicon service** otherwise. A favicon is not a brand logo:
 *     it is 16-64px of whatever the marque put in its browser tab, often a
 *     cropped glyph. It is here so a build with no key still reads as a garage
 *     rather than a column of grey initials, and it is a stopgap, not the
 *     design.
 *
 * Returns `null` for a car with no catalog match — no brand means no logo, and
 * the caller draws the initial instead.
 *
 * ONE TRAP, recorded because it looks like a bad key and is not: the CDN has a
 * hotlinking guard that answers 302 to its own documentation when the request
 * carries NO User-Agent at all. `curl` with no `-A` hits it and reads as
 * "the client id is wrong"; any real client, including this app, sends one and
 * is served normally. Verified against the live service.
 *
 * Both sources are third parties that learn which brand domains this device
 * looks up. Neither is sent a user identifier, a vehicle id, or anything else
 * from the account; the request carries a public domain name and nothing more.
 *
 * @param domain the brand's primary domain, e.g. `toyota.com`
 * @param size   the square size in points; the CDN is asked for 2x for retina
 */
export function brandLogoUrl(domain: string | null | undefined, size = 32): string | null {
  if (!domain) return null;
  const px = Math.round(size * 2);
  const host = encodeURIComponent(domain);
  if (CLIENT_ID) {
    return `https://cdn.brandfetch.io/${host}/w/${px}/h/${px}?c=${encodeURIComponent(CLIENT_ID)}`;
  }
  // Google caps this at 256 and ignores anything larger.
  return `https://www.google.com/s2/favicons?domain=${host}&sz=${Math.min(px, 256)}`;
}

/** `true` when this build is showing real brand marks rather than favicons. */
export function brandLogosAreBranded(): boolean {
  return CLIENT_ID !== "";
}
