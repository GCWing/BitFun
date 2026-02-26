/**
 * SceneBar type definitions.
 */

import type { LucideIcon } from 'lucide-react';

/** Scene tab identifier — max 3 open at a time */
export type SceneTabId = 'welcome' | 'session' | 'terminal' | 'git' | 'settings' | 'file-viewer' | 'profile' | 'capabilities' | 'team';

/** Static definition (from registry) for a scene tab type */
export interface SceneTabDef {
  id: SceneTabId;
  label: string;
  /** i18n key under common.scenes — when provided, SceneBar will translate instead of using label */
  labelKey?: string;
  Icon?: LucideIcon;
  /** Pinned tabs are always open and cannot be closed */
  pinned: boolean;
  /** Only one instance allowed */
  singleton: boolean;
  /** Open on app start */
  defaultOpen: boolean;
}

/** Runtime instance of an open scene tab */
export interface SceneTab {
  id: SceneTabId;
  /** Last-used timestamp for LRU eviction */
  lastUsed: number;
}
