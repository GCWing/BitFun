// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';
import type { WorkspaceInfo } from '@/shared/types';
import {
  isWorkflowClawWorkspace,
  isPlainAssistantWorkspace,
  splitAssistantWorkspacesByWorkflow,
} from './workflowClawWorkspace';

function makeWorkspace(overrides: Partial<WorkspaceInfo>): WorkspaceInfo {
  return {
    id: 'workspace-1',
    name: 'workspace',
    rootPath: 'C:/Users/me/.bitfun/personal_assistant/workspace-researcher',
    workspaceType: 'assistant',
    workspaceKind: 'assistant',
    languages: [],
    openedAt: '2026-08-17T00:00:00.000Z',
    lastAccessed: '2026-08-17T00:00:00.000Z',
    tags: [],
    ...overrides,
  } as WorkspaceInfo;
}

describe('isWorkflowClawWorkspace (R-WF-18 data-source isolation)', () => {
  it('treats a semantic node-id workspace as a workflow member Claw', () => {
    const ws = makeWorkspace({
      assistantId: 'researcher',
      rootPath: 'C:/Users/me/.bitfun/personal_assistant/workspace-researcher',
    });
    expect(isWorkflowClawWorkspace(ws)).toBe(true);
  });

  it('treats a hex-uuid assistant workspace as a plain Claw', () => {
    const ws = makeWorkspace({
      assistantId: '3f2a9c1d',
      rootPath: 'C:/Users/me/.bitfun/personal_assistant/workspace-3f2a9c1d',
    });
    expect(isWorkflowClawWorkspace(ws)).toBe(false);
  });

  it('treats a default assistant workspace (no assistantId) as a plain Claw', () => {
    const ws = makeWorkspace({
      assistantId: null,
      rootPath: 'C:/Users/me/.bitfun/personal_assistant/workspace',
    });
    expect(isWorkflowClawWorkspace(ws)).toBe(false);
  });

  it('is symmetric with isPlainAssistantWorkspace', () => {
    const workflow = makeWorkspace({ assistantId: 'implementer' });
    const plain = makeWorkspace({ assistantId: '7b1e0c4a' });
    const defaultWs = makeWorkspace({ assistantId: null });

    expect(isPlainAssistantWorkspace(workflow)).toBe(false);
    expect(isPlainAssistantWorkspace(plain)).toBe(true);
    expect(isPlainAssistantWorkspace(defaultWs)).toBe(true);
  });
});

describe('splitAssistantWorkspacesByWorkflow', () => {
  it('partitions a mixed list without dropping either side', () => {
    const workflowA = makeWorkspace({ id: 'a', assistantId: 'researcher' });
    const workflowB = makeWorkspace({ id: 'b', assistantId: 'reviewer' });
    const plain = makeWorkspace({ id: 'c', assistantId: '9d2f44c1' });
    const defaultWs = makeWorkspace({ id: 'd', assistantId: null });

    const { workflowClaws, plainAssistants } = splitAssistantWorkspacesByWorkflow([
      workflowA,
      plain,
      workflowB,
      defaultWs,
    ]);

    expect(workflowClaws.map(w => w.id)).toEqual(['a', 'b']);
    expect(plainAssistants.map(w => w.id)).toEqual(['c', 'd']);
  });

  it('returns empty workflow side when no member Claw exists', () => {
    const plain = makeWorkspace({ id: 'c', assistantId: '9d2f44c1' });
    const { workflowClaws, plainAssistants } = splitAssistantWorkspacesByWorkflow([plain]);

    expect(workflowClaws).toEqual([]);
    expect(plainAssistants.map(w => w.id)).toEqual(['c']);
  });
});
