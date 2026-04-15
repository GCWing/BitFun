import type { LucideIcon } from 'lucide-react';
import type { PanelType } from '../../types';
import type { OverlaySceneId, SceneId } from '../../overlay/types';

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
  /** For behavior:'scene' — which overlay scene to open, or 'session' for base */
  sceneId?: SceneId;
  /** Optional nav-panel scene switch without opening right-side overlay */
  navSceneId?: OverlaySceneId;
}

export interface NavSection {
  id: string;
  /** Null hides the section header row entirely */
  label: string | null;
  /** Optional overlay scene opened when clicking the section header */
  sceneId?: SceneId;
  collapsible?: boolean;
  defaultExpanded?: boolean;
  items: NavItem[];
}

/** Props contract for any inline section component */
export interface InlineSectionProps {
  onTabSwitch?: (tab: PanelType) => void;
}
