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

export type FigmaSourceDisposition = "implemented" | "asset-catalog" | "reference-only";

export interface FigmaSourceInventoryEntry {
  component?: string;
  disposition: FigmaSourceDisposition;
  nodeId: string;
  reason?: string;
  sourceName: string;
}

export const figmaSourceInventory: readonly FigmaSourceInventoryEntry[] = [
  ...componentRegistry.flatMap((component) => component.figma.nodeIds.map((nodeId, index) => ({
    component: component.name,
    disposition: "implemented" as const,
    nodeId,
    sourceName: component.figma.sourceNames[index] ?? component.name,
  }))),
  {
    disposition: "reference-only",
    nodeId: "133:6763",
    reason: "The mixed input component set is implemented through its normal, search, and chat variant nodes so each production component has one explicit source boundary.",
    sourceName: "input",
  },
  {
    disposition: "asset-catalog",
    nodeId: "2:22",
    reason: "HarmonyOS Icon is a glyph asset set; public components accept consumer-supplied icon nodes instead of coupling to one platform catalog.",
    sourceName: "HarmonyOS Icon",
  },
  {
    disposition: "reference-only",
    nodeId: "131:6340",
    reason: "Mouse is a design annotation for cursor placement, not a runtime UI control.",
    sourceName: "Mouse",
  },
];
