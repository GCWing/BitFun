import { buttonMeta } from "./components/Button/Button.meta";
import { iconButtonMeta } from "./components/IconButton/IconButton.meta";
import { headingMeta } from "./components/Heading/Heading.meta";
import { inputMeta } from "./components/Input/Input.meta";
import { keyHintMeta } from "./components/KeyHint/KeyHint.meta";
import { listItemMeta } from "./components/ListItem/ListItem.meta";
import { mediaThumbnailMeta } from "./components/MediaThumbnail/MediaThumbnail.meta";
import { navigationListMeta } from "./components/NavigationList/NavigationList.meta";
import { promptComposerMeta } from "./components/PromptComposer/PromptComposer.meta";
import { searchMeta } from "./components/Search/Search.meta";
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
  headingMeta,
  inputMeta,
  keyHintMeta,
  listItemMeta,
  mediaThumbnailMeta,
  navigationListMeta,
  promptComposerMeta,
  searchMeta,
  switchMeta,
  tabGroupMeta,
] as const satisfies readonly ComponentMeta[];
