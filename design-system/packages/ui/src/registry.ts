import { buttonMeta } from "./components/Button/Button.meta";
import { fieldMeta } from "./components/Field/Field.meta";
import { iconButtonMeta } from "./components/IconButton/IconButton.meta";
import { inputMeta } from "./components/Input/Input.meta";
import { keyHintMeta } from "./components/KeyHint/KeyHint.meta";
import { searchFieldMeta } from "./components/SearchField/SearchField.meta";
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
  fieldMeta,
  iconButtonMeta,
  inputMeta,
  keyHintMeta,
  searchFieldMeta,
  switchMeta,
  tabGroupMeta,
] as const satisfies readonly ComponentMeta[];
