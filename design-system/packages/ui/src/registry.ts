import { actionItemMeta } from "./components/ActionItem/ActionItem.meta";
import { actionCardMeta } from "./components/ActionCard/ActionCard.meta";
import { activityItemMeta } from "./components/ActivityItem/ActivityItem.meta";
import { buttonMeta } from "./components/Button/Button.meta";
import { cardMeta } from "./components/Card/Card.meta";
import { composerMeta } from "./components/Composer/Composer.meta";
import { confirmDialogMeta } from "./components/ConfirmDialog/ConfirmDialog.meta";
import { fieldMeta } from "./components/Field/Field.meta";
import { fieldGroupMeta } from "./components/FieldGroup/FieldGroup.meta";
import { iconMeta } from "./components/Icon/Icon.meta";
import { iconButtonMeta } from "./components/IconButton/IconButton.meta";
import { inputMeta } from "./components/Input/Input.meta";
import { keyHintMeta } from "./components/KeyHint/KeyHint.meta";
import { menuMeta } from "./components/Menu/Menu.meta";
import { modalMeta } from "./components/Modal/Modal.meta";
import { navigationPanelMeta } from "./components/NavigationPanel/NavigationPanel.meta";
import { pageHeaderMeta } from "./components/PageHeader/PageHeader.meta";
import { scrollAreaMeta } from "./components/ScrollArea/ScrollArea.meta";
import { searchFieldMeta } from "./components/SearchField/SearchField.meta";
import { selectMeta } from "./components/Select/Select.meta";
import { statusPillMeta } from "./components/StatusPill/StatusPill.meta";
import { switchMeta } from "./components/Switch/Switch.meta";
import { tabGroupMeta } from "./components/TabGroup/TabGroup.meta";
import { toolbarMeta } from "./components/Toolbar/Toolbar.meta";
import type { ComponentMeta } from "./registry.types";

export type {
  ComponentMaturity,
  ComponentMeta,
  ComponentPropMeta,
} from "./registry.types";

export const componentRegistry = [
  actionCardMeta,
  actionItemMeta,
  activityItemMeta,
  buttonMeta,
  cardMeta,
  composerMeta,
  confirmDialogMeta,
  fieldMeta,
  fieldGroupMeta,
  iconMeta,
  iconButtonMeta,
  inputMeta,
  keyHintMeta,
  menuMeta,
  modalMeta,
  navigationPanelMeta,
  pageHeaderMeta,
  scrollAreaMeta,
  searchFieldMeta,
  selectMeta,
  statusPillMeta,
  switchMeta,
  tabGroupMeta,
  toolbarMeta,
] as const satisfies readonly ComponentMeta[];
