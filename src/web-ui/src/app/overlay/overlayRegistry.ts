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
  CircleUserRound,
  Users,
  Puzzle,
  Boxes,
  User,
  ExternalLink,
  LayoutDashboard,
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
    id: 'profile',
    label: 'Profile',
    labelKey: 'scenes.profile',
    Icon: CircleUserRound,
  },
  {
    id: 'agents',
    label: 'Agents',
    labelKey: 'scenes.agents',
    Icon: Users,
  },
  {
    id: 'skills',
    label: 'Skills',
    labelKey: 'scenes.skills',
    Icon: Puzzle,
  },
  {
    id: 'miniapps',
    label: 'Mini App',
    labelKey: 'scenes.miniApps',
    Icon: Boxes,
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
  if (typeof id === 'string' && id.startsWith('miniapp:')) {
    const appId = id.slice('miniapp:'.length);
    return { id, label: appId, Icon: Puzzle };
  }
  return OVERLAY_SCENE_REGISTRY.find(d => d.id === id);
}
