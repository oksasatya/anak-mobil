/// <reference types="bun-types" />
/**
 * A net under the three ordering properties in the session layer that no type
 * checker can see: `client.ts`'s epoch guard on the refresh branch,
 * `refresh.ts`'s guard against replaying a spent token, and `signOut.ts`'s
 * failure-atomic teardown. `make mb-check` (tsc + lint) stays green with any
 * of the three deleted — this file is what goes red instead.
 *
 * Every dependency of the three files under test is faked via `mock.module`
 * (Bun's own module-mocking facility — no added library). `@/shared/api/errors`
 * is the one exception: it is pure and native-free, so it runs for real.
 * `@/shared/api/queryClient` and `@/shared/vehicle/activeVehicle` are faked
 * even though they are never "under test" here, because both touch
 * `react-native-mmkv` at module load and would crash outside a device.
 *
 * `@/shared/api/refresh` and `@/shared/session/signOut` are never mocked: they
 * are each the file under test in one scenario and a real dependency in
 * another (`client.ts` calls both), and `mock.module` mocks a specifier for
 * the whole process, so mocking either would corrupt the scenario that
 * exercises it directly.
 */
import { beforeEach, expect, mock, test } from "bun:test";
import { readFileSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

import type { StoredSession } from "@/shared/session/secure";

let sessionState: StoredSession | null = null;
let epochValue = 0;
let setSignedOutCalls = 0;

const readSession = mock((): Promise<StoredSession | null> => Promise.resolve(sessionState));
const writeSession = mock((value: Omit<StoredSession, "refreshPending">): Promise<void> => {
  sessionState = { ...value, refreshPending: false };
  return Promise.resolve();
});
const markRefreshPending = mock((): Promise<void> => {
  if (sessionState) sessionState = { ...sessionState, refreshPending: true };
  return Promise.resolve();
});
let clearSessionImpl = (): Promise<void> => {
  sessionState = null;
  return Promise.resolve();
};
const clearSession = mock((): Promise<void> => clearSessionImpl());

const currentEpoch = mock((): number => epochValue);
const bumpEpoch = mock((): number => (epochValue += 1));
const setSignedOut = mock((): void => {
  setSignedOutCalls += 1;
});

const cancelQueries = mock((): Promise<void> => Promise.resolve());
const clearQueryClient = mock((): void => {});
let purgeAllPersistedCacheImpl = (): void => {};
const purgeAllPersistedCache = mock((): void => purgeAllPersistedCacheImpl());
const clearActiveVehicle = mock((): void => {});

mock.module("@/shared/api/baseUrl", () => ({ BASE_URL: "http://test.invalid" }));
mock.module("@/shared/session/secure", () => ({
  readSession,
  writeSession,
  markRefreshPending,
  clearSession,
}));
mock.module("@/shared/session/store", () => ({ currentEpoch, bumpEpoch, setSignedOut }));
mock.module("@/shared/api/queryClient", () => ({
  queryClient: { cancelQueries, clear: clearQueryClient },
  purgeAllPersistedCache,
}));
mock.module("@/shared/vehicle/activeVehicle", () => ({ clearActiveVehicle }));

interface FetchCall {
  url: string;
  init: RequestInit | undefined;
}
const fetchCalls: FetchCall[] = [];
let fetchImpl: (url: string, init: RequestInit | undefined) => Response = () => {
  throw new Error("unexpected fetch in this test");
};
function toUrlString(input: RequestInfo | URL): string {
  if (typeof input === "string") return input;
  if (input instanceof URL) return input.href;
  return input.url;
}
// Cast through `unknown`, not `any`: Bun's own `fetch` type carries a static
// `preconnect` method this fake has no use for, so the two types do not
// structurally overlap even though every call site here only ever calls it
// as a plain function.
const fetchMock = mock((input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
  const url = toUrlString(input);
  fetchCalls.push({ url, init });
  return Promise.resolve(fetchImpl(url, init));
});
globalThis.fetch = fetchMock as unknown as typeof fetch;

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

// Real production code, dynamically imported so every `mock.module` above is
// registered first — a static top-level import would resolve before these
// run and pull in the un-mocked originals instead.
const { apiRequest } = await import("@/shared/api/client");
const { ensureRefreshed } = await import("@/shared/api/refresh");
const { signOut } = await import("@/shared/session/signOut");

beforeEach(() => {
  sessionState = null;
  epochValue = 0;
  setSignedOutCalls = 0;
  clearSessionImpl = () => {
    sessionState = null;
    return Promise.resolve();
  };
  purgeAllPersistedCacheImpl = () => {};
  fetchCalls.length = 0;
  fetchImpl = () => {
    throw new Error("unexpected fetch in this test");
  };
  for (const fn of [
    readSession,
    writeSession,
    markRefreshPending,
    clearSession,
    currentEpoch,
    bumpEpoch,
    setSignedOut,
    cancelQueries,
    clearQueryClient,
    purgeAllPersistedCache,
    clearActiveVehicle,
  ]) {
    fn.mockClear();
  }
  fetchMock.mockClear();
});

test("a request overtaken by a sign-out is never retried through a refresh", async () => {
  sessionState = { access: "stale-access", refresh: "stale-refresh", refreshPending: false };
  // The main request 401s; a concurrent sign-out lands while it is in flight —
  // simulated by bumping the epoch as the fake response comes back, the same
  // moment `bumpEpoch()` would run for real in `signOut.ts`.
  fetchImpl = (url) => {
    if (url === "http://test.invalid/me") {
      epochValue += 1;
      return jsonResponse(401, { error: { code: "unauthorized", message: "Sesi berakhir" } });
    }
    throw new Error(`unexpected fetch to ${url}`);
  };

  await expect(apiRequest("/me")).rejects.toMatchObject({ kind: "unauthorized" });

  // The one and only fetch is the original request. Deleting
  // `&& currentEpoch() === epoch` from client.ts's refresh guard sends this
  // response into `ensureRefreshed()`, which issues a second fetch to
  // `/auth/refresh` — turning this count to 2.
  expect(fetchCalls).toHaveLength(1);
});

test("a spent refresh token is never replayed", async () => {
  sessionState = { access: "stale-access", refresh: "stale-refresh", refreshPending: true };

  await expect(ensureRefreshed()).rejects.toMatchObject({ kind: "unauthorized" });

  // Deleting `|| stored.refreshPending` from refresh.ts's guard reaches the
  // fetch below it — turning this count to 1.
  expect(fetchCalls).toHaveLength(0);
});

test("sign-out ends the session even when its own teardown throws", async () => {
  sessionState = null; // no stored session — the best-effort logout POST is skipped
  const original = new Error("purgeAllPersistedCache failed (test)");
  purgeAllPersistedCacheImpl = () => {
    throw original;
  };
  // Teardown fails too, on top of the try body — proving the finally's own
  // `.catch(() => {})` on clearSession doesn't mask `original` or stop
  // setSignedOut from running.
  clearSessionImpl = () => Promise.reject(new Error("clearSession failed (test)"));

  // Deleting the `finally` from signOut.ts means `purgeAllPersistedCache`
  // throwing skips clearSession/setSignedOut entirely — setSignedOutCalls
  // would stay 0.
  await expect(signOut()).rejects.toBe(original);
  expect(setSignedOutCalls).toBe(1);
});

test("a stale refresh overtaken by sign-out then sign-in never deletes the new session", async () => {
  sessionState = { access: "stale-access", refresh: "stale-refresh", refreshPending: false };
  const freshSession: StoredSession = {
    access: "fresh-access-2",
    refresh: "fresh-refresh-2",
    refreshPending: false,
  };

  fetchImpl = (url) => {
    if (url === "http://test.invalid/auth/refresh") {
      // Simulate the interleaving branch-gate finding 1 is about: a sign-out
      // lands (bumps the epoch), then a fresh sign-in writes a brand-new
      // session — both while this refresh is still in flight — before the
      // server finally answers that the (now-revoked) refresh token it was
      // sent no longer works.
      epochValue += 1;
      sessionState = freshSession;
      return jsonResponse(401, { error: { code: "unauthorized", message: "Token tidak valid" } });
    }
    throw new Error(`unexpected fetch to ${url}`);
  };

  await expect(ensureRefreshed()).rejects.toMatchObject({ kind: "unauthorized" });

  // Moving the epoch guard back to sit only in front of `writeSession` (its
  // pre-fix position) lets this stale refresh's `pair === null` branch run
  // unconditionally and wipe the fresh session a later sign-in just wrote.
  expect(clearSession).not.toHaveBeenCalled();
  expect(sessionState).toEqual(freshSession);
});

test("the refresh fetch is bounded to 10s, and a timeout surfaces as offline", async () => {
  sessionState = { access: "stale-access", refresh: "stale-refresh", refreshPending: false };

  const originalSetTimeout = globalThis.setTimeout;
  let capturedDelay: number | undefined;
  let capturedCallback: (() => void) | undefined;
  globalThis.setTimeout = ((callback: () => void, delay?: number) => {
    capturedCallback = callback;
    capturedDelay = delay;
    return 0 as unknown as ReturnType<typeof setTimeout>;
  }) as typeof setTimeout;

  fetchImpl = (url) => {
    if (url === "http://test.invalid/auth/refresh") {
      // Fire the captured abort callback before "the server" ever answers —
      // this is what a real 10s-elapsed timer does — then raise the same
      // error a real aborted `fetch` raises.
      capturedCallback?.();
      throw new DOMException("Aborted", "AbortError");
    }
    throw new Error(`unexpected fetch to ${url}`);
  };

  try {
    await expect(ensureRefreshed()).rejects.toMatchObject({ kind: "offline" });
  } finally {
    globalThis.setTimeout = originalSetTimeout;
  }

  // Deleting the AbortController/setTimeout wiring around this fetch removes
  // the call entirely — capturedDelay would stay undefined.
  expect(capturedDelay).toBe(10_000);
});

test("single-flight: concurrent 401s issue exactly one refresh", async () => {
  sessionState = { access: "stale-access", refresh: "stale-refresh", refreshPending: false };
  fetchImpl = (url) => {
    if (url === "http://test.invalid/auth/refresh") {
      return jsonResponse(200, {
        data: { access_token: "fresh-access", refresh_token: "fresh-refresh" },
      });
    }
    throw new Error(`unexpected fetch to ${url}`);
  };

  const results = await Promise.all([ensureRefreshed(), ensureRefreshed(), ensureRefreshed()]);

  expect(results).toEqual(["fresh-access", "fresh-access", "fresh-access"]);
  expect(fetchCalls).toHaveLength(1);
});

/**
 * Task 16 (ledger 95): none of the four tests above touch the route-group
 * gating this task added, so `tsc` and `expo lint` are the only net under
 * it — and neither sees a removed `<AppGate>` or a screen dropped outside
 * its group.
 *
 * A render test would be the more direct check, but `expo-router` cannot be
 * IMPORTED under `bun:test` at all: it pulls in `react-native`, whose own
 * entry file uses Flow syntax (`import typeof * as ReactNativePublicAPI
 * from './index.js.flow'`) that bun's transpiler rejects outright — verified
 * by importing `expo-router` alone in an otherwise-empty test file, which
 * fails before any test body runs. There is no renderer in this repo that
 * gets past that (jest-expo pulls in Jest + a Babel pipeline neither `bun
 * test` nor this file uses), so the two tests below read the route tree as
 * plain text and as a directory listing instead of rendering it.
 */
const APP_DIR = fileURLToPath(new URL("../src/app", import.meta.url));

test("each group layout wraps its navigator in the matching gate", () => {
  // `(app)` moved from `<Stack>` to `<Tabs>` in Plan C Task 2 — the group's
  // own tab navigator, per-tab `<Stack>`s live one level down in
  // components/shell/TabStack.tsx. `(auth)` and `(onboarding)` are untouched
  // by this plan and still wrap `<Stack>` directly.
  const groups: Array<{ file: string; gate: string; navigator: string }> = [
    { file: "(app)/_layout.tsx", gate: "AppGate", navigator: "<Tabs" },
    { file: "(auth)/_layout.tsx", gate: "AuthGate", navigator: "<Stack" },
    { file: "(onboarding)/_layout.tsx", gate: "OnboardingGate", navigator: "<Stack" },
  ];

  for (const { file, gate, navigator } of groups) {
    const source = readFileSync(join(APP_DIR, file), "utf8");
    const openTag = source.indexOf(`<${gate}>`);
    const closeTag = source.lastIndexOf(`</${gate}>`);
    // Searched from `openTag`, not from 0: each of these files has a comment
    // above the gate that may also contain the literal navigator text (e.g.
    // "Plan C replaces the `<Stack>` body...") — starting the search at the
    // gate's own open tag skips that false match and finds only the real JSX
    // usage.
    const navigatorTag = source.indexOf(navigator, openTag);

    // Deleting `<AppGate>` from `(app)/_layout.tsx` — the exact defect the
    // component-not-inline design exists to prevent — leaves openTag at -1.
    expect(openTag).toBeGreaterThanOrEqual(0);
    expect(closeTag).toBeGreaterThan(openTag);
    // The navigator must sit strictly between the gate's open and close tags
    // — unwrapping it (or the gate) trips one of these instead.
    expect(navigatorTag).toBeGreaterThan(openTag);
    expect(navigatorTag).toBeLessThan(closeTag);
  }
});

test("every route file outside catalog.tsx and the entry index.tsx lives inside a route group", () => {
  const allowedUngrouped = new Set(["catalog.tsx", "index.tsx"]);

  function collectRouteFiles(dir: string): string[] {
    const files: string[] = [];
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) {
        files.push(...collectRouteFiles(full));
      } else if (entry.name.endsWith(".tsx") && entry.name !== "_layout.tsx") {
        files.push(full);
      }
    }
    return files;
  }

  for (const file of collectRouteFiles(APP_DIR)) {
    const segments = relative(APP_DIR, file).split("/");
    const topLevelName = segments[0];

    if (segments.length === 1) {
      // A new screen at `src/app/foo.tsx` lands here — ungated, and nothing
      // else in the gate/route tree catches it.
      expect(allowedUngrouped.has(topLevelName)).toBe(true);
    } else {
      expect(topLevelName.startsWith("(") && topLevelName.endsWith(")")).toBe(true);
    }
  }
});

/**
 * AC1's STACK half had no assertion anywhere, and that is why this exists.
 *
 * Task 2 moved `<Stack>` out of `(app)/_layout.tsx` and down into
 * `components/shell/TabStack.tsx`, rendered by each tab's own `_layout.tsx`.
 * Nothing referenced `TabStack` from a test — so deleting any tab's
 * `_layout.tsx` left the route still rendering (expo-router falls back to the
 * parent navigator), silently collapsed that tab's independent navigation
 * stack, and the suite stayed green. Found in Task 2's review.
 *
 * Filesystem + readFileSync, the same shape as the gate test above, because
 * this repository's runner has no React renderer.
 */
test("every tab owns its own navigation stack", () => {
  const tabs = ["home", "garage", "explore", "community", "profile"];

  for (const tab of tabs) {
    const layout = join(APP_DIR, "(app)", tab, "_layout.tsx");
    const source = readFileSync(layout, "utf8");
    // Deleting the file throws here; rendering something other than TabStack
    // fails the assertion. Either way AC1's stack half stops being silent.
    expect(source).toContain("<TabStack");
  }
});

test("nothing re-enables the tab-reset behaviour AC1 forbids", () => {
  // `popToTopOnBlur: false` IS AC1's stack half, and `unmountOnBlur` destroys
  // the scroll half. Both are correct BY DEFAULT, which means a future edit
  // can break them by ADDING a line — the case a structural test catches and
  // a screenshot does not.
  // Matches an actual prop assignment (`popToTopOnBlur:` or `=`), not the bare
  // word — both names are DISCUSSED in comments in that file precisely because
  // leaving them unset is deliberate, and a test that forbade the word would
  // punish the comment that explains the decision.
  const layout = readFileSync(join(APP_DIR, "(app)", "_layout.tsx"), "utf8");
  expect(layout).not.toMatch(/popToTopOnBlur\s*[:=]/);
  expect(layout).not.toMatch(/unmountOnBlur\s*[:=]/);
});
