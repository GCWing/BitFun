import { describe, expect, it } from 'vitest';
import {
  filterToolsForReviewMode,
  normalizeReviewModeState,
  type SubagentEditorToolInfo,
} from './subagentEditorUtils';

const tools: SubagentEditorToolInfo[] = [
  { name: 'Read', isReadonly: true },
  { name: 'Grep', isReadonly: true },
  { name: 'Write', isReadonly: false },
  { name: 'Bash', isReadonly: false },
];

describe('subagentEditorUtils', () => {
  it('shows only readonly tools for review subagents', () => {
    expect(filterToolsForReviewMode(tools, true).map((tool) => tool.name)).toEqual([
      'Read',
      'Grep',
    ]);
    expect(filterToolsForReviewMode(tools, false).map((tool) => tool.name)).toEqual([
      'Read',
      'Grep',
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
});
