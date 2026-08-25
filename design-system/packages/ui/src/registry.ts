import { buttonMeta } from "./components/Button/Button.meta";
import { switchMeta } from "./components/Switch/Switch.meta";
import { tabGroupMeta } from "./components/TabGroup/TabGroup.meta";
import type { ComponentMeta } from "./registry.types";

export type {
  ComponentMaturity,
  ComponentMeta,
  ComponentPropMeta,
} from "./registry.types";

export const componentRegistry = [
  buttonMeta,
  switchMeta,
  tabGroupMeta,
] as const satisfies readonly ComponentMeta[];
