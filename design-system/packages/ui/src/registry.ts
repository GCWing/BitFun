import { buttonMeta } from "./components/Button/Button.meta";
import { iconButtonMeta } from "./components/IconButton/IconButton.meta";
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
  iconButtonMeta,
  switchMeta,
  tabGroupMeta,
] as const satisfies readonly ComponentMeta[];
