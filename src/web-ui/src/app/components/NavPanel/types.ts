import type { LucideIcon } from 'lucide-react';
import type { PanelType } from '../../types';
import type { SceneTabId } from '../SceneBar/types';

/** Determines what clicking a NavItem does */
export type NavBehavior =
  | 'contextual'   // stays in current scene, updates AuxPane / inline section
  | 'scene';       // opens / activates a SceneBar tab

export interface NavItem {
  tab: PanelType;
  /** i18n key for label (e.g. nav.items.sessions). Preferred over label when present. */
  labelKey?: string;
  /** Fallback label when labelKey is not used */
  label?: string;
  /** i18n key for tooltip. When present, overrides the default label-based tooltip. */
  tooltipKey?: string;
  Icon: LucideIcon;
  behavior: NavBehavior;
  /** For behavior:'scene' — which SceneBar tab to open/activate */
  sceneId?: SceneTabId;
  /** Optional nav-panel scene switch without opening right-side scene */
  navSceneId?: SceneTabId;
  /** When true, clicking this item toggles an inline content area below it */
  inlineExpandable?: boolean;
}

export interface NavSection {
  id: string;
  /** Null hides the section header row entirely */
  label: string | null;
  collapsible?: boolean;
  defaultExpanded?: boolean;
  items: NavItem[];
}

/** Props contract for any inline section component */
export interface InlineSectionProps {
  onTabSwitch?: (tab: PanelType) => void;
}
