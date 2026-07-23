import { describe, expect, it } from 'vitest';

import {
  effectiveToolInvocation,
  projectEffectiveToolItem,
} from './toolInvocationIdentity';

describe('toolInvocationIdentity', () => {
  it('derives an effective invocation without changing the wire input', () => {
    const wireInput = {
      tool_name: 'mcp__docs__search',
      args: { query: 'identity' },
    };

    expect(effectiveToolInvocation('CallDeferredTool', wireInput)).toEqual({
      toolName: 'mcp__docs__search',
      input: wireInput.args,
      isDeferred: true,
    });
    expect(wireInput).toEqual({
      tool_name: 'mcp__docs__search',
      args: { query: 'identity' },
    });
  });

  it('falls back to the wire identity for malformed gateway input', () => {
    const input = { path: 'README.md' };
    expect(effectiveToolInvocation('CallDeferredTool', input)).toEqual({
      toolName: 'CallDeferredTool',
      input,
      isDeferred: false,
    });
  });

  it('projects an effective card view while retaining the canonical item', () => {
    const item = {
      id: 'tool-1',
      type: 'tool' as const,
      toolName: 'CallDeferredTool',
      toolCall: {
        id: 'tool-1',
        input: {
          tool_name: 'Write',
          args: { file_path: 'README.md', content: 'updated' },
        },
      },
      status: 'pending_confirmation' as const,
      timestamp: 1,
    };

    const projected = projectEffectiveToolItem(item);
    expect(projected.toolName).toBe('Write');
    expect(projected.toolCall.input).toEqual({ file_path: 'README.md', content: 'updated' });
    expect(item.toolName).toBe('CallDeferredTool');
    expect(item.toolCall.input).toHaveProperty('tool_name', 'Write');
  });
});
