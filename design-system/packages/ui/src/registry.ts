import { actionItemMeta } from "./components/ActionItem/ActionItem.meta";
import { actionCardMeta } from "./components/ActionCard/ActionCard.meta";
import { activityItemMeta } from "./components/ActivityItem/ActivityItem.meta";
import { alertMeta } from "./components/Alert/Alert.meta";
import { avatarMeta } from "./components/Avatar/Avatar.meta";
import { buttonMeta } from "./components/Button/Button.meta";
import { checkboxMeta } from "./components/Checkbox/Checkbox.meta";
import { cardMeta } from "./components/Card/Card.meta";
import { composerMeta } from "./components/Composer/Composer.meta";
import { comboboxMeta } from "./components/Combobox/Combobox.meta";
import { confirmDialogMeta } from "./components/ConfirmDialog/ConfirmDialog.meta";
import { disclosureMeta } from "./components/Disclosure/Disclosure.meta";
import { emptyMeta } from "./components/Empty/Empty.meta";
import { fieldMeta } from "./components/Field/Field.meta";
import { fieldGroupMeta } from "./components/FieldGroup/FieldGroup.meta";
import { iconMeta } from "./components/Icon/Icon.meta";
import { iconButtonMeta } from "./components/IconButton/IconButton.meta";
import { inputMeta } from "./components/Input/Input.meta";
import { keyHintMeta } from "./components/KeyHint/KeyHint.meta";
import { listboxMeta } from "./components/Listbox/Listbox.meta";
import { menuMeta } from "./components/Menu/Menu.meta";
import { dialogMeta, sheetMeta } from "./components/Dialog/Dialog.meta";
import { multiSelectMeta } from "./components/MultiSelect/MultiSelect.meta";
import { navigationPanelMeta } from "./components/NavigationPanel/NavigationPanel.meta";
import { numberInputMeta } from "./components/NumberInput/NumberInput.meta";
import { numberBadgeMeta } from "./components/NumberBadge/NumberBadge.meta";
import { pageHeaderMeta } from "./components/PageHeader/PageHeader.meta";
import { radioMeta } from "./components/Radio/Radio.meta";
import { scrollAreaMeta } from "./components/ScrollArea/ScrollArea.meta";
import { searchFieldMeta } from "./components/SearchField/SearchField.meta";
import { segmentedControlMeta } from "./components/SegmentedControl/SegmentedControl.meta";
import { selectMeta } from "./components/Select/Select.meta";
import { statusPillMeta } from "./components/StatusPill/StatusPill.meta";
import { loadingStateMeta, spinnerMeta } from "./components/Spinner/Spinner.meta";
import { switchMeta } from "./components/Switch/Switch.meta";
import { textareaMeta } from "./components/Textarea/Textarea.meta";
import { tabGroupMeta } from "./components/TabGroup/TabGroup.meta";
import { toolbarMeta } from "./components/Toolbar/Toolbar.meta";
import { tooltipMeta } from "./components/Tooltip/Tooltip.meta";
import { askUserMeta } from "./flow-chat/ask-user/AskUser.meta";
import { chatComposerMeta } from "./flow-chat/composer/ChatComposer.meta";
import { ambientToolCardMeta } from "./flow-chat/tool-cards/AmbientToolCard.meta";
import { commandToolCardMeta } from "./flow-chat/tool-cards/CommandToolCard.meta";
import { contextCompressionToolCardMeta } from "./flow-chat/tool-cards/ContextCompressionToolCard.meta";
import { fileOperationToolCardMeta } from "./flow-chat/tool-cards/FileOperationToolCard.meta";
import { prominentToolCardMeta } from "./flow-chat/tool-cards/ProminentToolCard.meta";
import {
  agentControlToolCardMeta,
  fileDiffToolCardMeta,
  gitToolCardMeta,
  pageDeployToolCardMeta,
  pagePublishToolCardMeta,
  reviewSummaryToolCardMeta,
} from "./flow-chat/tool-cards/ProminentToolCards.meta";
import { readFileToolCardMeta } from "./flow-chat/tool-cards/ReadFileToolCard.meta";
import {
  agentWaitToolCardMeta,
  defaultToolCardMeta,
  directoryListToolCardMeta,
  getToolSpecToolCardMeta,
  globSearchToolCardMeta,
  grepSearchToolCardMeta,
  runCodeToolCardMeta,
  sessionControlToolCardMeta,
  sessionMessageToolCardMeta,
  skillToolCardMeta,
  terminalControlToolCardMeta,
  todoToolCardMeta,
  viewImageToolCardMeta,
  webFetchToolCardMeta,
  webSearchToolCardMeta,
} from "./flow-chat/tool-cards/StandardToolCards.meta";
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
  alertMeta,
  agentControlToolCardMeta,
  agentWaitToolCardMeta,
  ambientToolCardMeta,
  askUserMeta,
  avatarMeta,
  buttonMeta,
  cardMeta,
  checkboxMeta,
  chatComposerMeta,
  commandToolCardMeta,
  composerMeta,
  comboboxMeta,
  confirmDialogMeta,
  contextCompressionToolCardMeta,
  defaultToolCardMeta,
  disclosureMeta,
  emptyMeta,
  directoryListToolCardMeta,
  fieldMeta,
  fieldGroupMeta,
  fileDiffToolCardMeta,
  fileOperationToolCardMeta,
  getToolSpecToolCardMeta,
  gitToolCardMeta,
  globSearchToolCardMeta,
  grepSearchToolCardMeta,
  iconMeta,
  iconButtonMeta,
  inputMeta,
  keyHintMeta,
  listboxMeta,
  loadingStateMeta,
  menuMeta,
  dialogMeta,
  multiSelectMeta,
  navigationPanelMeta,
  numberInputMeta,
  numberBadgeMeta,
  pageDeployToolCardMeta,
  pageHeaderMeta,
  radioMeta,
  pagePublishToolCardMeta,
  prominentToolCardMeta,
  readFileToolCardMeta,
  reviewSummaryToolCardMeta,
  runCodeToolCardMeta,
  scrollAreaMeta,
  searchFieldMeta,
  segmentedControlMeta,
  selectMeta,
  sheetMeta,
  sessionControlToolCardMeta,
  sessionMessageToolCardMeta,
  skillToolCardMeta,
  statusPillMeta,
  spinnerMeta,
  switchMeta,
  tabGroupMeta,
  textareaMeta,
  terminalControlToolCardMeta,
  todoToolCardMeta,
  toolbarMeta,
  tooltipMeta,
  viewImageToolCardMeta,
  webFetchToolCardMeta,
  webSearchToolCardMeta,
] as const satisfies readonly ComponentMeta[];
