/**
 * SCENE_TAB_REGISTRY — static definitions for all scene tab types.
 *
 * Rules:
 *  - Every explicitly opened scene remains open until the user closes it.
 *  - pinned = true: protected from manual close.
 *  - Scene ids are unique, while Mini Apps use one id per app instance.
 */

import { SessionIcon } from '@bitfun/ui';
import {
  MessageSquare,
  Terminal,
  GitBranch,
  Settings,
  FileCode2,
  CircleUserRound,
  Users,
  Puzzle,
  Boxes,
  PanelsTopLeft,
  Globe,
  User,
  BarChart3,
  CalendarClock,
  ExternalLink,
  Network,
} from 'lucide-react';
import type { SceneTabDef, SceneTabId } from '../components/SceneBar/types';

export const SCENE_TAB_REGISTRY: SceneTabDef[] = [
  {
    id: 'welcome' as SceneTabId,
    label: 'Welcome',
    labelKey: 'welcomeScene.tabLabel',
    Icon: MessageSquare,
    pinned: false,
    singleton: true,
    defaultOpen: true,
  },
  {
    id: 'session' as SceneTabId,
    label: 'Session',
    labelKey: 'scenes.aiAgent',
    Icon: SessionIcon,
    pinned: true,
    closable: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'terminal' as SceneTabId,
    label: 'Terminal',
    Icon: Terminal,
    pinned: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'git' as SceneTabId,
    label: 'Git',
    Icon: GitBranch,
    pinned: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'settings' as SceneTabId,
    label: 'Settings',
    labelKey: 'shared:features.settings',
    Icon: Settings,
    pinned: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'file-viewer' as SceneTabId,
    label: 'File Viewer',
    Icon: FileCode2,
    pinned: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'profile' as SceneTabId,
    label: 'Profile',
    Icon: CircleUserRound,
    pinned: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'agents' as SceneTabId,
    label: 'Agents',
    labelKey: 'scenes.agents',
    Icon: Users,
    pinned: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'skills' as SceneTabId,
    label: 'Skills',
    labelKey: 'scenes.skills',
    Icon: Puzzle,
    pinned: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'ecosystem-compatibility' as SceneTabId,
    label: 'Ecosystem Compatibility',
    labelKey: 'nav.items.ecosystemCompatibility',
    Icon: Network,
    pinned: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'miniapps' as SceneTabId,
    label: 'Mini App',
    labelKey: 'scenes.miniApps',
    Icon: Boxes,
    pinned: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'pages' as SceneTabId,
    label: 'Pages',
    Icon: PanelsTopLeft,
    pinned: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'browser' as SceneTabId,
    label: 'Browser',
    labelKey: 'scenes.browser',
    Icon: Globe,
    pinned: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'assistant' as SceneTabId,
    label: 'Assistant',
    labelKey: 'scenes.assistant',
    Icon: User,
    pinned: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'todos' as SceneTabId,
    label: 'Task Board',
    labelKey: 'scenes.todos',
    Icon: CalendarClock,
    pinned: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'insights' as SceneTabId,
    label: 'Insights',
    labelKey: 'scenes.insights',
    Icon: BarChart3,
    pinned: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'shell' as SceneTabId,
    label: 'Shell',
    labelKey: 'scenes.shell',
    Icon: Terminal,
    pinned: false,
    singleton: true,
    defaultOpen: false,
  },
  {
    id: 'panel-view' as SceneTabId,
    label: 'Panel View',
    labelKey: 'scenes.panelView',
    Icon: ExternalLink,
    pinned: false,
    closable: true,
    singleton: true,
    defaultOpen: false,
  },
];

export function getSceneDef(id: SceneTabId): SceneTabDef | undefined {
  return SCENE_TAB_REGISTRY.find(d => d.id === id);
}

/** Static singleton scene def for the panel-view scene. */
export const PANEL_VIEW_SCENE_DEF: SceneTabDef = SCENE_TAB_REGISTRY.find(d => d.id === 'panel-view')!;

/** Dynamic scene def for a MiniApp tab (used by SceneBar and useSceneManager). */
export function getMiniAppSceneDef(appId: string, appName?: string): SceneTabDef {
  const id: SceneTabId = `miniapp:${appId}`;
  return {
    id,
    label: appName ?? appId,
    Icon: Puzzle,
    pinned: false,
    closable: true,
    singleton: false,
    defaultOpen: false,
  };
}
