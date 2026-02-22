/**
 * NAV_SECTIONS — pure data config for NavPanel navigation.
 *
 * behavior:'contextual' → stays in current scene, updates AuxPane / inline section
 * behavior:'scene'      → opens / activates a SceneBar tab
 *
 * Section groups:
 *   - workspace: project workspace essentials (sessions, files)
 *   - my-agent:  everything describing the super agent (profile, team)
 *   - dev-suite: developer toolkit (git, terminal, shell-hub)
 */

import {
  MessageSquare,
  Folder,
  Terminal,
  GitBranch,
  UserCircle2,
  Users,
  SquareTerminal,
} from 'lucide-react';
import type { NavSection } from './types';

export const NAV_SECTIONS: NavSection[] = [
  {
    id: 'workspace',
    label: 'Workspace',
    collapsible: false,
    items: [
      {
        tab: 'sessions',
        labelKey: 'nav.items.sessions',
        Icon: MessageSquare,
        behavior: 'contextual',
        inlineExpandable: true,
      },
      {
        tab: 'files',
        labelKey: 'nav.items.project',
        Icon: Folder,
        behavior: 'contextual',
        navSceneId: 'file-viewer',
      },
    ],
  },
  {
    id: 'my-agent',
    label: 'My Agent',
    collapsible: true,
    defaultExpanded: false,
    items: [
      {
        tab: 'profile',
        labelKey: 'nav.items.persona',
        tooltipKey: 'nav.tooltips.persona',
        Icon: UserCircle2,
        behavior: 'scene',
        sceneId: 'profile',
      },
      {
        tab: 'team',
        labelKey: 'nav.items.team',
        tooltipKey: 'nav.tooltips.team',
        Icon: Users,
        behavior: 'scene',
        sceneId: 'team',
        inlineExpandable: true,
      },
    ],
  },
  {
    id: 'dev-suite',
    label: '开发套件',
    collapsible: true,
    defaultExpanded: false,
    items: [
      {
        tab: 'git',
        labelKey: 'nav.items.git',
        Icon: GitBranch,
        behavior: 'scene',
        sceneId: 'git',
        inlineExpandable: true,
      },
      {
        tab: 'terminal',
        labelKey: 'nav.items.terminal',
        Icon: Terminal,
        behavior: 'scene',
        sceneId: 'terminal',
        inlineExpandable: true,
      },
      {
        tab: 'shell-hub',
        labelKey: 'nav.items.shellHub',
        Icon: SquareTerminal,
        behavior: 'contextual',
        inlineExpandable: true,
      },
    ],
  },
];
