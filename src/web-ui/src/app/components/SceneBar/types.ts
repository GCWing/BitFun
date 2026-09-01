/**
 * SceneBar type definitions.
 */

import type { ReactNode, SVGProps } from 'react';

export type SceneTabIconProps = Omit<SVGProps<SVGSVGElement>, 'ref'> & {
  size?: number | string;
};

/** SVG icon contract shared by design-system and third-party icon components. */
export type SceneTabIcon = (props: SceneTabIconProps) => ReactNode;

/** Scene tab identifier. Open scenes are kept until the user closes them. */
export type SceneTabId =
  | 'session'
  | 'terminal'
  | 'git'
  | 'settings'
  | 'file-viewer'
  | 'profile'
  | 'agents'
  | 'skills'
  | 'ecosystem-compatibility'
  | 'miniapps'
  | 'pages'
  | 'browser'
  | 'assistant'
  | 'todos'
  | 'insights'
  | 'shell'
  | 'panel-view'
  | `miniapp:${string}`;

/** Static definition (from registry) for a scene tab type */
export interface SceneTabDef {
  id: SceneTabId;
  label: string;
  /** i18n key resolved through the common namespace, or an explicit namespace key such as shared:features.settings. */
  labelKey?: string;
  Icon?: SceneTabIcon;
  /** Keep this tab ahead of regular tabs while it is open. */
  pinned: boolean;
  /** If false, the user cannot close the tab. Defaults to true. */
  closable?: boolean;
  /** Only one instance allowed */
  singleton: boolean;
  /** Open on app start */
  defaultOpen: boolean;
}

/** Runtime instance of an open scene. */
export interface SceneTab {
  id: SceneTabId;
  /** Last-used timestamp for activate/close fallback (e.g. which tab to activate after close). */
  lastUsed: number;
}
