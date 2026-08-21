-- Where a brand's logo comes from.
--
-- A DOMAIN, not an image URL. The two are not the same decision:
--
--   * The domain is a stable fact about the brand. toyota.com will still be
--     Toyota's domain when whatever CDN we use today has been replaced.
--   * An image URL bakes a provider, a size, a format, and an API key into
--     the database. Changing any of them then means a migration and a
--     backfill, for a change that is purely a client concern — the mobile app
--     wants 64px, the backoffice wants 24px, and a future og:image wants 512.
--
-- So the server stores the domain and each client composes its own URL. The
-- clients use Brandfetch's Logo Link CDN, which takes exactly this shape:
-- https://cdn.brandfetch.io/{domain}/w/64/h/64?c={client_id}
--
-- Nullable, because a brand can genuinely have no usable domain — Timor has
-- not existed since 2000 — and a NOT NULL here would force an invented one.
-- A NULL means "fall back to initials", which every client must handle anyway
-- for the moment the network is not there.
ALTER TABLE brands ADD COLUMN logo_domain TEXT;

COMMENT ON COLUMN brands.logo_domain IS
    'Brand primary domain, e.g. toyota.com. Clients compose their own logo URL from it.';
