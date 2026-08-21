import type Ionicons from "@expo/vector-icons/Ionicons";
import type { Href } from "expo-router";

/**
 * An Ionicons glyph name, as a TYPE-ONLY import so this module stays free of
 * anything React Native at runtime — see the note on `hasAddActions` below.
 */
export type AddActionIcon = keyof (typeof Ionicons)["glyphMap"];

export interface AddAction {
  readonly key: string;
  readonly label: string;
  /**
   * Where the row goes — or `null` while the form it would open does not
   * exist yet.
   *
   * `null` is not a soft version of a destination. A row with one renders
   * DISABLED and says so; it is never tappable and never navigates. The type
   * is nullable rather than optional on purpose: adding an entry forces the
   * author to answer "does this go anywhere" instead of forgetting to.
   */
  readonly href: Href | null;
  /** The glyph in the sheet row. Decorative — the label is what is announced. */
  readonly icon: AddActionIcon;
  /** One line under the label saying what the form actually records. */
  readonly description: string;
}

/**
 * What "+" offers, and what it admits it cannot do yet.
 *
 * AM-16 AC2 names four — modifikasi, servis, problem, foto — and none of their
 * forms exist; each belongs to its own epic.
 *
 * THIS ARRAY USED TO BE EMPTY, and the reasoning has changed rather than been
 * forgotten. The rule was "an entry whose destination is missing is ABSENT
 * rather than present and broken", so `AddButton` rendered nothing at all and
 * the shell had no "+". The owner asked for the button the redesign specifies,
 * which means the four rows are visible before their forms are. The rule they
 * were protecting — no control that lies — is kept by a different mechanism:
 * a row with `href: null` is drawn as unavailable and cannot be tapped, so it
 * reads as a roadmap rather than as a dead end. Shipping the form for one is
 * one line here: give it an `href`.
 *
 *   modifikasi  build epic              Part, setup, dan build
 *   servis      service-history epic    Catatan servis dan biaya
 *   problem     known-issues epic       Gejala dan masalah
 *   foto        vehicle-photos epic     Foto mobil kamu
 */
export const ADD_ACTIONS: readonly AddAction[] = [
  {
    key: "modifikasi",
    label: "Modifikasi",
    href: null,
    icon: "construct-outline",
    description: "Part, setup, dan build",
  },
  {
    key: "servis",
    label: "Servis",
    href: null,
    icon: "clipboard-outline",
    description: "Catatan servis dan biaya",
  },
  {
    key: "problem",
    label: "Problem",
    href: null,
    icon: "warning-outline",
    description: "Gejala dan masalah",
  },
  {
    key: "foto",
    label: "Foto",
    href: null,
    icon: "camera-outline",
    description: "Foto mobil kamu",
  },
];

/**
 * Whether the add mechanism has anything to show.
 *
 * Pure and react-native-free on purpose: it is the one rule in this task worth
 * a test, and pulling it out of `AddButton` is what lets `bun test` hold it —
 * a React component cannot be rendered by this runner.
 */
export function hasAddActions(actions: readonly AddAction[]): boolean {
  return actions.length > 0;
}

/** Whether this row can be tapped, or is a name with no form behind it yet. */
export function isAddActionReady(action: AddAction): boolean {
  return action.href !== null;
}
