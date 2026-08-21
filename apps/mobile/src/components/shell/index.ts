// `AddButton` is deliberately NOT exported. It has exactly one mount, inside
// `TabScreen`, which imports it relatively. A barrel export is the affordance
// that lets a future screen mount a second one inside its own TabScreen — two
// floating buttons, two independent sheets. Found in the Tasks 3-5 review.
export { TabScreen } from "./TabScreen";
export type { TabScreenProps } from "./TabScreen";
export { TabStack } from "./TabStack";
