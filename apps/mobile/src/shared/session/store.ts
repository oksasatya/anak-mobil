import { create } from "zustand";

/** The caller's identity and derived onboarding state, as `GET /me` reports it. */
export interface Me {
  id: string;
  email: string;
  username: string | null;
  displayName: string | null;
  hasVehicles: boolean;
}

export type SessionStatus = "loading" | "signedOut" | "signedIn";

interface SessionState {
  status: SessionStatus;
  user: Me | null;
}

/**
 * Starts at "loading", never at "signedOut".
 *
 * A cold start of a signed-in app has to read the Keychain and call `/me`
 * before it knows anything. Defaulting to signed-out shows the welcome screen
 * for a frame every single launch.
 */
const useStore = create<SessionState>(() => ({ status: "loading", user: null }));

/**
 * The auth epoch, deliberately outside React state.
 *
 * `apiRequest` is not a component and cannot call a hook; it reads this
 * synchronously to decide whether a response that has just resolved still
 * belongs to the account that asked for it. A value held in React state and
 * captured by a closure is precisely the stale read this exists to prevent.
 *
 * Incremented first in the sign-out transaction. Any response landing after
 * that is dropped instead of written, which is what stops an in-flight request
 * from repopulating a cache that was just cleared — the mechanism by which the
 * next account would otherwise see the previous account's garage.
 */
let epoch = 0;

export function currentEpoch(): number {
  return epoch;
}

export function bumpEpoch(): number {
  epoch += 1;
  return epoch;
}

/**
 * The session, for components.
 *
 * Two individual selectors rather than one returning an object literal: a
 * literal is a new reference every render, so every subscriber would re-render
 * on every unrelated store write. The object this function itself returns is
 * still a fresh literal every render, though — a `useEffect(..., [session])`
 * depending on the object from this hook re-fires on every render, not only
 * on an actual `status`/`user` change; depend on `session.status` /
 * `session.user` directly instead.
 */
export function useSession(): { status: SessionStatus; user: Me | null } {
  const status = useStore((state) => state.status);
  const user = useStore((state) => state.user);
  return { status, user };
}

/** Both at once, so nothing observes `signedIn` with a null user. */
export function setSignedIn(user: Me): void {
  useStore.setState({ status: "signedIn", user });
}

export function setSignedOut(): void {
  useStore.setState({ status: "signedOut", user: null });
}

/** Replace the user without touching `status` — what `refreshMe` needs. */
export function setUser(user: Me): void {
  useStore.setState({ user });
}

/**
 * The signed-in account's id, read outside React — the persisted cache is keyed on it.
 *
 * Deliberately NOT named with a `use` prefix (the plan's original name,
 * `useSessionStoreUserId`, fails `react-hooks/rules-of-hooks`: eslint-plugin-react-hooks
 * flags any `use*`-named function called from a plain async function, and `signOut.ts`'s
 * `run()` is exactly that). It is not a hook and must not read like one, which also
 * settles the plan's own worry about a bare `userId()` inviting a component to reach for
 * this instead of `useSession()`.
 */
export function sessionUserId(): string | null {
  return useStore.getState().user?.id ?? null;
}
