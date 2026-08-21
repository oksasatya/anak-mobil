/**
 * `brandLogoUrl` reads `EXPO_PUBLIC_BRANDFETCH_CLIENT_ID` at MODULE LOAD, the
 * way `BASE_URL` does, so each case has to import the module fresh after
 * setting the variable — `Bun.build`-style `await import()` with the registry
 * reset, not a top-level import.
 *
 * The rule worth holding: an unkeyed build must still return a URL, because
 * Brandfetch answers an unkeyed request with a 302 to its own documentation
 * rather than an image, and a garage of grey initials was the bug this
 * fallback exists to fix.
 */
import { afterEach, beforeEach, expect, test } from "bun:test";

const KEY = "EXPO_PUBLIC_BRANDFETCH_CLIENT_ID";
const previous = process.env[KEY];
const MODULE = "@/features/catalog/brandLogo";

/** Bun caches modules by specifier; a query string gives each case its own. */
let seq = 0;
async function load() {
  seq += 1;
  return (await import(`${MODULE}?case=${seq}`)) as typeof import("@/features/catalog/brandLogo");
}

beforeEach(() => {
  delete process.env[KEY];
});

afterEach(() => {
  if (previous === undefined) delete process.env[KEY];
  else process.env[KEY] = previous;
});

test("a client id produces a Brandfetch URL at 2x the asked size", async () => {
  process.env[KEY] = "test-client";
  const { brandLogoUrl, brandLogosAreBranded } = await load();
  expect(brandLogoUrl("toyota.com", 32)).toBe(
    "https://cdn.brandfetch.io/toyota.com/w/64/h/64?c=test-client",
  );
  expect(brandLogosAreBranded()).toBe(true);
});

test("no client id still yields a logo, from the favicon fallback", async () => {
  const { brandLogoUrl, brandLogosAreBranded } = await load();
  // NOT null. Brandfetch without a key answers 302 to its hotlinking docs, so
  // returning its URL here would render nothing at all.
  expect(brandLogoUrl("toyota.com", 32)).toBe(
    "https://www.google.com/s2/favicons?domain=toyota.com&sz=64",
  );
  expect(brandLogosAreBranded()).toBe(false);
});

test("the favicon size is capped at what the service actually serves", async () => {
  const { brandLogoUrl } = await load();
  expect(brandLogoUrl("honda.com", 256)).toBe(
    "https://www.google.com/s2/favicons?domain=honda.com&sz=256",
  );
});

test("a car with no catalog match has no brand, so it has no logo", async () => {
  process.env[KEY] = "test-client";
  const { brandLogoUrl } = await load();
  expect(brandLogoUrl(null)).toBeNull();
  expect(brandLogoUrl(undefined)).toBeNull();
  expect(brandLogoUrl("")).toBeNull();
});

test("the domain is escaped rather than interpolated raw", async () => {
  process.env[KEY] = "test-client";
  const { brandLogoUrl } = await load();
  // Not a real brand domain — the point is that a value from the database
  // cannot walk out of its path segment and add query parameters of its own.
  expect(brandLogoUrl("a.com/x?c=other")).toBe(
    "https://cdn.brandfetch.io/a.com%2Fx%3Fc%3Dother/w/64/h/64?c=test-client",
  );
});
