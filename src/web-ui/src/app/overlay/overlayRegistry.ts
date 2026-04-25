/**
 * overlayRegistry — static definitions for all overlay scene types.
 *
 * Overlay scenes appear on top of the Agentic OS base session.
 * The base session ('session') is not listed here — it is always present.
 */

import {
  Terminal,
  Settings,
  FileCode2,
  Brain,
  CircleUserRound,
  Users,
  Puzzle,
  Wrench,
  User,
  ExternalLink,
  LayoutDashboard,
  Bot,
} from 'lucide-react';
import type { OverlaySceneDef, OverlaySceneId } from './types';

export const OVERLAY_SCENE_REGISTRY: OverlaySceneDef[] = [
  {
    id: 'terminal',
    label: 'Terminal',
    labelKey: 'scenes.terminal',
    Icon: Terminal,
  },
  {
    id: 'settings',
    label: 'Settings',
    labelKey: 'scenes.settings',
    Icon: Settings,
  },
  {
    id: 'file-viewer',
    label: 'File Viewer',
    labelKey: 'scenes.fileViewer',
    Icon: FileCode2,
  },
  {
    id: 'memory',
    label: 'Memory',
    labelKey: 'scenes.memory',
    Icon: Brain,
  },
  {
    id: 'profile',
    label: 'Profile',
    labelKey: 'scenes.profile',
    Icon: CircleUserRound,
  },
  {
    id: 'apps',
    label: 'Apps',
    labelKey: 'scenes.apps',
    Icon: Users,
  },
  {
    id: 'subagents',
    label: 'SubAgents',
    labelKey: 'scenes.subagents',
    Icon: Bot,
  },
  {
    id: 'skills',
    label: 'Skills',
    labelKey: 'scenes.skills',
    Icon: Puzzle,
  },
  {
    id: 'tools',
    label: 'Tools',
    labelKey: 'scenes.tools',
    Icon: Wrench,
  },
  {
    id: 'assistant',
    label: 'Assistant',
    labelKey: 'scenes.assistant',
    Icon: User,
  },
  {
    id: 'shell',
    label: 'Shell',
    labelKey: 'scenes.shell',
    Icon: Terminal,
  },
  {
    id: 'panel-view',
    label: 'Panel View',
    labelKey: 'scenes.panelView',
    Icon: ExternalLink,
  },
  {
    id: 'task-detail',
    label: 'Task Detail',
    labelKey: 'scenes.taskDetail',
    Icon: LayoutDashboard,
  },
];

export function getOverlayDef(id: OverlaySceneId): OverlaySceneDef | undefined {
  if (typeof id === 'string' && id.startsWith('live-app:')) {
    const appId = id.slice('live-app:'.length);
    return { id, label: appId, Icon: Puzzle };
  }
  return OVERLAY_SCENE_REGISTRY.find(d => d.id === id);
}
