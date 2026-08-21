/**
 * The geometry of the floating dock: the tab bar and the add button.
 *
 * Two files draw it — `(app)/_layout.tsx` positions the bar, `AddButton.tsx`
 * positions the button — and they have to agree to the point or the button
 * sits a few pixels off the bar's baseline. The numbers live here so that
 * agreement is a shared constant rather than two copies that drift.
 */

/** Distance from the left and right screen edges. */
export const DOCK_INSET = 14;

/** The gap between the bar and the add button. */
export const DOCK_GAP = 10;

/** The add button is a circle; this is its diameter. */
export const DOCK_BUTTON = 58;

/**
 * The bar's own height, excluding the safe-area offset below it.
 *
 * 68, not 60. The navigator stacks a 22pt icon over an 11/14 label and adds
 * its own padding around both; at 60 the labels were clipped by the pill's
 * own rounded edge, which reads as a rendering fault rather than a tight fit.
 */
export const DOCK_BAR_HEIGHT = 68;

/**
 * How far the dock floats above the bottom edge.
 *
 * On a device with a home indicator the safe-area inset already clears it, so
 * the dock rides on top of that. On one without, 12 is the floating gap —
 * flush to the edge would read as a fixed bar rather than a floating one.
 */
export function dockBottom(safeAreaBottom: number): number {
  return Math.max(safeAreaBottom, 12);
}

/**
 * The bar's right edge: the screen inset, plus the button and its gap when a
 * button is actually there.
 *
 * `AddButton` renders nothing while the add-action registry is empty, so
 * reserving its slot unconditionally would leave the bar visibly off-centre
 * with nothing in the space it gave up.
 */
export function dockBarRight(hasButton: boolean): number {
  return hasButton ? DOCK_INSET + DOCK_BUTTON + DOCK_GAP : DOCK_INSET;
}

/**
 * How much bottom padding a scrollable tab screen owes so its last line clears
 * the dock.
 *
 * `useBottomTabBarHeight()` reports the navigator's own measurement, which was
 * right while the bar was welded to the bottom edge. A floating bar also has
 * the gap underneath it, and the navigator does not know about that gap — so
 * the clearance is computed from the same constants that draw the dock.
 */
export function dockClearance(safeAreaBottom: number): number {
  return dockBottom(safeAreaBottom) + DOCK_BAR_HEIGHT;
}
