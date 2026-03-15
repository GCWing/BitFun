/**
 * SCENE_TAB_REGISTRY — static definitions for all scene tab types.
 *
 * Rules:
 *  - Max 3 open tabs total.
 *  - pinned = true: protected from auto-eviction and manual close.
 *  - pinned = false: can be auto-evicted and manually closed.
 */

import {
  MessageSquare,
  Terminal,
  GitBranch,
  Settings,
  FileCode2,
  CircleUserRound,
  Blocks,
  Users,
  Puzzle,
  Boxes,
  UserCircle2,
  Globe,
} from 'lucide-react';
import type { SceneTabDef, SceneTabId } from '../components/SceneBar/types';

export const MAX_OPEN_SCENES = 3;

export const SCENE_TAB_REGISTRY: SceneTabDef[] = [
  {
    id: 'welcome' as SceneTabId,
    label: 'Welcome',
    labelKey: 'welcomeScene.tabLabel',
    pinned: false,
    singleton: true,
    defaultOpen: true,
  },
  {
    id: 'session' as SceneTabId,
    label: 'Session',
    labelKey: 'scenes.aiAgent',
    Icon: MessageSquare,
    pinned: true,
    fixed: true,
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
    id: 'capabilities' as SceneTabId,
    label: 'Capabilities',
    Icon: Blocks,
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
    id: 'miniapps' as SceneTabId,
    label: 'Mini App',
    labelKey: 'scenes.miniApps',
    Icon: Boxes,
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
    id: 'my-agent' as SceneTabId,
    label: 'My Agent',
    labelKey: 'scenes.myAgent',
    Icon: UserCircle2,
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
];

export function getSceneDef(id: SceneTabId): SceneTabDef | undefined {
  return SCENE_TAB_REGISTRY.find(d => d.id === id);
}

/** Dynamic scene def for a MiniApp tab (used by SceneBar and useSceneManager). */
export function getMiniAppSceneDef(appId: string, appName?: string): SceneTabDef {
  const id: SceneTabId = `miniapp:${appId}`;
  return {
    id,
    label: appName ?? appId,
    Icon: Puzzle,
    pinned: false,
    fixed: false,
    closable: true,
    singleton: false,
    defaultOpen: false,
  };
}
