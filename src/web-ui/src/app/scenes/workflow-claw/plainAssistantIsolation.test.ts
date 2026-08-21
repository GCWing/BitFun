// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';
import type { WorkspaceInfo } from '@/shared/types';
import { splitAssistantWorkspacesByWorkflow } from './workflowClawWorkspace';

function makeWorkspace(id: string, assistantId: string | null): WorkspaceInfo {
  return {
    id,
    name: id,
    rootPath: `C:/Users/me/.bitfun/personal_assistant/workspace-${assistantId ?? 'default'}`,
    assistantId,
    workspaceKind: 'assistant',
    languages: [],
    openedAt: '2026-08-17T00:00:00.000Z',
    lastAccessed: '2026-08-17T00:00:00.000Z',
    tags: [],
  } as WorkspaceInfo;
}

describe('plain assistant list isolation (R-WF-18 assertion 3)', () => {
  it('keeps workflow-member Claws out of the plain assistant partition', () => {
    const workflowMember = makeWorkspace('workspace-researcher', 'researcher');
    const plain = makeWorkspace('workspace-3f2a9c1d', '3f2a9c1d');

    const { workflowClaws, plainAssistants } = splitAssistantWorkspacesByWorkflow([
      workflowMember,
      plain,
    ]);

    expect(plainAssistants.map(w => w.id)).toEqual(['workspace-3f2a9c1d']);
    expect(workflowClaws.map(w => w.id)).toEqual(['workspace-researcher']);
  });

  it('does not drop the default assistant (no assistantId) from the plain side', () => {
    const defaultAssistant = makeWorkspace('workspace-default', null);
    const { workflowClaws, plainAssistants } = splitAssistantWorkspacesByWorkflow([defaultAssistant]);

    expect(workflowClaws).toEqual([]);
    expect(plainAssistants.map(w => w.id)).toEqual(['workspace-default']);
  });
});
