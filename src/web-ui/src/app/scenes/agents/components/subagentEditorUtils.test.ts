import { describe, expect, it } from 'vitest';
import {
  evaluateReviewSubagentToolReadiness,
  filterToolsForReviewMode,
  normalizeReviewModeState,
  type SubagentEditorToolInfo,
} from './subagentEditorUtils';

const tools: SubagentEditorToolInfo[] = [
  { name: 'GetFileDiff', isReadonly: true },
  { name: 'Read', isReadonly: true },
  { name: 'Grep', isReadonly: true },
  { name: 'Glob', isReadonly: true },
  { name: 'LS', isReadonly: true },
  { name: 'Write', isReadonly: false },
  { name: 'Bash', isReadonly: false },
];

describe('subagentEditorUtils', () => {
  it('shows only readonly tools for review subagents', () => {
    expect(filterToolsForReviewMode(tools, true).map((tool) => tool.name)).toEqual([
      'GetFileDiff',
      'Read',
      'Grep',
      'Glob',
      'LS',
    ]);
    expect(filterToolsForReviewMode(tools, false).map((tool) => tool.name)).toEqual([
      'GetFileDiff',
      'Read',
      'Grep',
      'Glob',
      'LS',
      'Write',
      'Bash',
    ]);
  });

  it('forces review subagents to readonly and removes writable selected tools', () => {
    const next = normalizeReviewModeState({
      review: true,
      readonly: false,
      selectedTools: new Set(['Read', 'Write', 'Bash']),
      availableTools: tools,
    });

    expect(next.readonly).toBe(true);
    expect(Array.from(next.selectedTools)).toEqual(['Read']);
    expect(next.removedToolNames).toEqual(['Write', 'Bash']);
  });

  it('marks review subagent tooling invalid when the minimum diff or read tool is missing', () => {
    expect(evaluateReviewSubagentToolReadiness(new Set(['Read']))).toMatchObject({
      readiness: 'invalid',
      missingRequiredTools: ['GetFileDiff'],
    });
  });

  it('marks review subagent tooling degraded when only the minimum tools are present', () => {
    expect(evaluateReviewSubagentToolReadiness(new Set(['GetFileDiff', 'Read']))).toMatchObject({
      readiness: 'degraded',
      missingRecommendedTools: ['Grep', 'Glob', 'LS'],
    });
  });

  it('marks review subagent tooling ready when the standard review tools are present', () => {
    expect(
      evaluateReviewSubagentToolReadiness(
        new Set(['GetFileDiff', 'Read', 'Grep', 'Glob', 'LS']),
      ),
    ).toMatchObject({
      readiness: 'ready',
      missingRequiredTools: [],
      missingRecommendedTools: [],
    });
  });
});
