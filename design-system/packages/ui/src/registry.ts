import { actionItemMeta } from "./components/ActionItem/ActionItem.meta";
import { buttonMeta } from "./components/Button/Button.meta";
import { composerMeta } from "./components/Composer/Composer.meta";
import { fieldMeta } from "./components/Field/Field.meta";
import { iconButtonMeta } from "./components/IconButton/IconButton.meta";
import { inputMeta } from "./components/Input/Input.meta";
import { keyHintMeta } from "./components/KeyHint/KeyHint.meta";
import { menuMeta } from "./components/Menu/Menu.meta";
import { modalMeta } from "./components/Modal/Modal.meta";
import { navigationPanelMeta } from "./components/NavigationPanel/NavigationPanel.meta";
import { pageHeaderMeta } from "./components/PageHeader/PageHeader.meta";
import { scrollAreaMeta } from "./components/ScrollArea/ScrollArea.meta";
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
  actionItemMeta,
  buttonMeta,
  composerMeta,
  fieldMeta,
  iconButtonMeta,
  inputMeta,
  keyHintMeta,
  menuMeta,
  modalMeta,
  navigationPanelMeta,
  pageHeaderMeta,
  scrollAreaMeta,
  searchFieldMeta,
  switchMeta,
  tabGroupMeta,
] as const satisfies readonly ComponentMeta[];
