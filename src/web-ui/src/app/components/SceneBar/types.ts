/**
 * SceneBar type definitions.
 */

import type { LucideIcon } from 'lucide-react';

/** Scene tab identifier — max 3 open at a time */
export type SceneTabId =
  | 'welcome'
  | 'session'
  | 'terminal'
  | 'git'
  | 'settings'
  | 'file-viewer'
  | 'profile'
  | 'agents'
  | 'skills'
  | 'miniapps'
  | 'browser'
  | 'my-agent'
  | 'shell'
  | `miniapp:${string}`;

/** Static definition (from registry) for a scene tab type */
export interface SceneTabDef {
  id: SceneTabId;
  label: string;
  /** i18n key under common.scenes — when provided, SceneBar will translate instead of using label */
  labelKey?: string;
  Icon?: LucideIcon;
  /** @deprecated Prefer fixed + closable. Pinned tabs cannot be closed and were protected from eviction. */
  pinned: boolean;
  /** If true, tab is always kept and never evicted by capacity policy (e.g. agent/session). */
  fixed?: boolean;
  /** If false, user cannot close the tab. Default true for non-fixed scenes. */
  closable?: boolean;
  /** Only one instance allowed */
  singleton: boolean;
  /** Open on app start */
  defaultOpen: boolean;
}

/** Runtime instance of an open scene tab */
export interface SceneTab {
  id: SceneTabId;
  /** First-open timestamp for FIFO eviction (oldest replaceable tab is evicted). */
  openedAt: number;
  /** Last-used timestamp for activate/close fallback (e.g. which tab to activate after close). */
  lastUsed: number;
}
