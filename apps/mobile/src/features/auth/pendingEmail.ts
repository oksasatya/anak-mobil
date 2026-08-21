/**
 * The address carried from register's "email sudah terdaftar" panel to login.
 *
 * Deliberately NOT a route parameter, and this is a rule rather than a taste.
 * `router.push({ pathname: "/login", params: { email } })` renders on the web
 * target as `/login?email=oksa%40example.com` — in the address bar, in browser
 * history, in the back/forward cache, and in the `Referer` of any subsequent
 * cross-origin request. This repository's standing rule is that personal data
 * never goes in a URL parameter or query string, and `app.config.ts` already
 * declares `web: { output: "static" }` with a `bun run web` script, so that is
 * one command away rather than hypothetical.
 *
 * Passing it out-of-band also removed a crash. `useLocalSearchParams<{ email?:
 * string }>()` is a type ASSERTION, not a guarantee: expo-router passes arrays
 * straight through, so `anakmobil://login?email=a&email=b` delivered
 * `["a","b"]` into a `readonly value: string` prop, and the first tap on Masuk
 * hit `email.trim is not a function`. A crafted link killed the login screen's
 * primary action. With no param there is nothing to craft.
 *
 * Read-once by design: `take` clears as it reads, so arriving at login by any
 * other route — the welcome screen, a back gesture — starts empty rather than
 * resurrecting an address from an abandoned attempt.
 *
 * Module scope rather than a store because there is exactly one producer, one
 * consumer, and no render that depends on it: the value is read in a
 * `useState` initialiser and never again. A zustand slice would buy
 * reactivity nothing here wants. Found in Task 4's review.
 */
let pending: string | null = null;

export function setPendingEmail(email: string): void {
  pending = email;
}

/** The pending address, cleared as it is read. Empty string when there is none. */
export function takePendingEmail(): string {
  const email = pending ?? "";
  pending = null;
  return email;
}
