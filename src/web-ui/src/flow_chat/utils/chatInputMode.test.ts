import { describe, expect, it } from 'vitest';

import {
  normalizeUserDefaultChatInputModeId,
  resolveAvailableChatInputMode,
  resolveChatInputModePolicy,
  resolveSessionAssistantWorkspace,
  resolveSwitchableChatInputModes,
  resolveWorkspaceChatInputMode,
} from './chatInputMode';
import { WorkspaceKind, type WorkspaceInfo, WorkspaceType } from '@/shared/types';

function createWorkspace(overrides: Partial<WorkspaceInfo>): WorkspaceInfo {
  return {
    id: overrides.id ?? 'workspace-1',
    name: overrides.name ?? 'Workspace',
    rootPath: overrides.rootPath ?? 'D:/workspace/project',
    workspaceType: overrides.workspaceType ?? WorkspaceType.SingleProject,
    workspaceKind: overrides.workspaceKind ?? WorkspaceKind.Normal,
    languages: overrides.languages ?? [],
    openedAt: overrides.openedAt ?? new Date(0).toISOString(),
    lastAccessed: overrides.lastAccessed ?? new Date(0).toISOString(),
    tags: overrides.tags ?? [],
    ...overrides,
  };
}

describe('normalizeUserDefaultChatInputModeId', () => {
  it('normalizes non-empty strings and rejects blank values', () => {
    expect(normalizeUserDefaultChatInputModeId(' PlannerPlus ')).toBe('PlannerPlus');
    expect(normalizeUserDefaultChatInputModeId('   ')).toBeNull();
    expect(normalizeUserDefaultChatInputModeId(null)).toBeNull();
  });
});

describe('resolveWorkspaceChatInputMode', () => {
  it('forces Claw inside assistant workspaces', () => {
    expect(
      resolveWorkspaceChatInputMode({
        currentMode: 'agentic',
        isAssistantWorkspace: true,
        sessionMode: 'agentic',
      })
    ).toBe('Claw');
  });

  it('keeps non-Claw project modes unchanged', () => {
    expect(
      resolveWorkspaceChatInputMode({
        currentMode: 'Plan',
        isAssistantWorkspace: false,
        sessionMode: 'Plan',
      })
    ).toBeNull();
  });

  it('syncs when switching between project sessions with different modes', () => {
    expect(
      resolveWorkspaceChatInputMode({
        currentMode: 'Plan',
        isAssistantWorkspace: false,
        sessionMode: 'agentic',
      })
    ).toBe('agentic');
  });

  it('restores a project session mode after a transient assistant workspace state', () => {
    expect(
      resolveWorkspaceChatInputMode({
        currentMode: 'Claw',
        isAssistantWorkspace: false,
        sessionMode: 'agentic',
      })
    ).toBe('agentic');
  });

  it('restores Cowork when a project Cowork session inherited the Claw UI mode', () => {
    expect(
      resolveWorkspaceChatInputMode({
        currentMode: 'Claw',
        isAssistantWorkspace: false,
        sessionMode: 'Cowork',
      })
    ).toBe('Cowork');
  });

  it('falls back to agentic if a project session has no mode yet', () => {
    expect(
      resolveWorkspaceChatInputMode({
        currentMode: 'Claw',
        isAssistantWorkspace: false,
        sessionMode: undefined,
      })
    ).toBe('agentic');
  });

  it('keeps Claw sessions synchronized even before workspace state identifies the assistant workspace', () => {
    expect(
      resolveWorkspaceChatInputMode({
        currentMode: 'agentic',
        isAssistantWorkspace: false,
        sessionMode: 'Claw',
      })
    ).toBe('Claw');
  });
});

describe('resolveChatInputModePolicy', () => {
  it('allows mode switching for normal code sessions', () => {
    expect(
      resolveChatInputModePolicy({
        currentMode: 'agentic',
        isAssistantWorkspace: false,
        sessionMode: 'agentic',
      }),
    ).toEqual({
      canSwitchModes: true,
      fixedModeId: null,
      fixedReason: null,
    });
  });

  it('fixes assistant workspaces to Claw', () => {
    expect(
      resolveChatInputModePolicy({
        currentMode: 'agentic',
        isAssistantWorkspace: true,
        sessionMode: 'agentic',
      }),
    ).toEqual({
      canSwitchModes: false,
      fixedModeId: 'Claw',
      fixedReason: 'assistant-workspace',
    });
  });

  it('fixes Claw sessions even when workspace resolution is temporarily stale', () => {
    expect(
      resolveChatInputModePolicy({
        currentMode: 'agentic',
        isAssistantWorkspace: false,
        sessionMode: 'claw',
      }),
    ).toEqual({
      canSwitchModes: false,
      fixedModeId: 'Claw',
      fixedReason: 'session-mode',
    });
  });

  it('fixes Cowork sessions from current or session mode', () => {
    expect(
      resolveChatInputModePolicy({
        currentMode: 'Cowork',
        isAssistantWorkspace: false,
        sessionMode: 'agentic',
      }),
    ).toMatchObject({
      canSwitchModes: false,
      fixedModeId: 'Cowork',
      fixedReason: 'current-mode',
    });

    expect(
      resolveChatInputModePolicy({
        currentMode: 'agentic',
        isAssistantWorkspace: false,
        sessionMode: 'cowork',
      }),
    ).toMatchObject({
      canSwitchModes: false,
      fixedModeId: 'Cowork',
      fixedReason: 'session-mode',
    });
  });

  it('fixes ACP sessions without treating them as a product mode', () => {
    expect(
      resolveChatInputModePolicy({
        currentMode: 'agentic',
        isAssistantWorkspace: false,
        sessionMode: 'acp:example',
        isAcpTargetSession: true,
      }),
    ).toEqual({
      canSwitchModes: false,
      fixedModeId: null,
      fixedReason: 'acp-session',
    });
  });
});

describe('resolveSwitchableChatInputModes', () => {
  it('removes fixed collaboration modes from boost selection', () => {
    expect(
      resolveSwitchableChatInputModes([
        { id: 'agentic' },
        { id: 'Cowork' },
        { id: 'Claw' },
        { id: 'PlannerPlus' },
      ]),
    ).toEqual([
      { id: 'agentic' },
      { id: 'PlannerPlus' },
    ]);
  });
});

describe('resolveSessionAssistantWorkspace', () => {
  it('does not treat a project session as assistant during workspace scene transitions', () => {
    const projectWorkspace = createWorkspace({
      id: 'project-1',
      rootPath: 'E:/Projects/repos/claude-code',
      workspaceKind: WorkspaceKind.Normal,
    });
    const assistantWorkspace = createWorkspace({
      id: 'assistant-1',
      rootPath: 'C:/Users/wsp/.bitfun/personal_assistant/workspace',
      workspaceKind: WorkspaceKind.Assistant,
    });

    expect(
      resolveSessionAssistantWorkspace({
        currentWorkspace: assistantWorkspace,
        sessionWorkspaceId: projectWorkspace.id,
        sessionWorkspacePath: projectWorkspace.rootPath,
        openedWorkspaces: [projectWorkspace, assistantWorkspace],
      }),
    ).toBe(false);
  });

  it('recognizes assistant sessions from their own workspace scope even before current workspace catches up', () => {
    const projectWorkspace = createWorkspace({
      id: 'project-1',
      rootPath: 'E:/Projects/repos/claude-code',
      workspaceKind: WorkspaceKind.Normal,
    });
    const assistantWorkspace = createWorkspace({
      id: 'assistant-1',
      rootPath: 'C:/Users/wsp/.bitfun/personal_assistant/workspace',
      workspaceKind: WorkspaceKind.Assistant,
    });

    expect(
      resolveSessionAssistantWorkspace({
        currentWorkspace: projectWorkspace,
        sessionWorkspaceId: assistantWorkspace.id,
        sessionWorkspacePath: assistantWorkspace.rootPath,
        openedWorkspaces: [projectWorkspace, assistantWorkspace],
      }),
    ).toBe(true);
  });

  it('falls back to the current workspace kind when the session has no explicit workspace scope yet', () => {
    const assistantWorkspace = createWorkspace({
      id: 'assistant-1',
      rootPath: 'C:/Users/wsp/.bitfun/personal_assistant/workspace',
      workspaceKind: WorkspaceKind.Assistant,
    });

    expect(
      resolveSessionAssistantWorkspace({
        currentWorkspace: assistantWorkspace,
        openedWorkspaces: [assistantWorkspace],
      }),
    ).toBe(true);
  });
});

describe('resolveAvailableChatInputMode', () => {
  it('returns the synchronized session mode when it is still available', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'agentic',
        isAssistantWorkspace: false,
        sessionMode: 'Plan',
        availableModeIds: ['agentic', 'Plan', 'Team'],
      }),
    ).toBe('Plan');
  });

  it('falls back to agentic when the current mode is no longer available', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'PlannerPlus',
        isAssistantWorkspace: false,
        sessionMode: 'PlannerPlus',
        availableModeIds: ['agentic', 'Team'],
      }),
    ).toBe('agentic');
  });

  it('keeps the current mode when only the session snapshot is stale', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'Team',
        isAssistantWorkspace: false,
        sessionMode: 'PlannerPlus',
        availableModeIds: ['agentic', 'Team'],
      }),
    ).toBeNull();
  });

  it('keeps assistant workspaces pinned to Claw when available', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'PlannerPlus',
        isAssistantWorkspace: true,
        sessionMode: 'PlannerPlus',
        availableModeIds: ['agentic', 'Claw'],
      }),
    ).toBe('Claw');
  });

  it('keeps Claw sessions pinned even before assistant workspace resolution catches up', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'agentic',
        isAssistantWorkspace: false,
        sessionMode: 'Claw',
        availableModeIds: ['agentic', 'Claw', 'PlannerPlus'],
      }),
    ).toBe('Claw');
  });

  it('falls back to the first available mode when agentic is unavailable', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'PlannerPlus',
        isAssistantWorkspace: false,
        sessionMode: 'PlannerPlus',
        availableModeIds: ['Team', 'Plan'],
      }),
    ).toBe('Team');
  });

  it('uses the user default mode when starting from the internal project default', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'agentic',
        isAssistantWorkspace: false,
        sessionMode: undefined,
        userDefaultModeId: 'PlannerPlus',
        availableModeIds: ['agentic', 'PlannerPlus'],
      }),
    ).toBe('PlannerPlus');
  });

  it('does not let the user default override an existing session mode', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'Team',
        isAssistantWorkspace: false,
        sessionMode: 'Team',
        userDefaultModeId: 'PlannerPlus',
        availableModeIds: ['agentic', 'Team', 'PlannerPlus'],
      }),
    ).toBeNull();
  });

  it('ignores unavailable user defaults and falls back to agentic', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'MissingMode',
        isAssistantWorkspace: false,
        sessionMode: undefined,
        userDefaultModeId: 'PlannerPlus',
        availableModeIds: ['agentic', 'Team'],
      }),
    ).toBe('agentic');
  });

  it('keeps assistant workspaces pinned to Claw even with a user default', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'agentic',
        isAssistantWorkspace: true,
        sessionMode: undefined,
        userDefaultModeId: 'PlannerPlus',
        availableModeIds: ['agentic', 'Claw', 'PlannerPlus'],
      }),
    ).toBe('Claw');
  });
});
