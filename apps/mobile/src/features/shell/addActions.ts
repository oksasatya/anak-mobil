import type { Href } from "expo-router";

export interface AddAction {
  readonly key: string;
  readonly label: string;
  readonly href: Href;
}

/**
 * What "+" can add today.
 *
 * AM-16 AC2 names four — modifikasi, servis, problem, foto — and none of
 * their forms exist; each belongs to its own epic. The spec's rule is that an
 * entry whose destination is missing is ABSENT rather than present and
 * broken, so this array is empty, and `AddButton` renders nothing while it
 * is. A "+" that opens onto an empty sheet is a dead end, which is the one
 * thing the empty-state rules in this codebase exist to prevent.
 *
 * Adding an entry here is the entire integration: the sheet's contents, the
 * pre-filled vehicle, and the button's existence all read from this array.
 *
 *   modifikasi  build epic
 *   servis      service-history epic
 *   problem     known-issues epic
 *   foto        vehicle-photos epic
 *
 * The explicit type annotation matters: without it the empty literal infers
 * `never[]` and the first entry added fails to type-check for the wrong
 * reason.
 */
export const ADD_ACTIONS: readonly AddAction[] = [];

/**
 * Whether the add mechanism has anything to offer.
 *
 * Pure and react-native-free on purpose: it is the one rule in this task
 * worth a test (AC2's "no dead-end button"), and pulling it out of
 * `AddButton` is what lets `bun test` hold it — a React component cannot be
 * rendered by this runner.
 */
export function hasAddActions(actions: readonly AddAction[]): boolean {
  return actions.length > 0;
}
