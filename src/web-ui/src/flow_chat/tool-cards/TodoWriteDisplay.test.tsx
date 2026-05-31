import { describe, expect, it } from 'vitest';

import { createTodoRenderItems } from './todoRenderItems';

describe('createTodoRenderItems', () => {
  it('keeps React render keys unique when restored todos reuse ids', () => {
    const items = createTodoRenderItems([
      { id: '[truncated for session view]', content: 'Phase 1', status: 'completed' },
      { id: '[truncated for session view]', content: 'Phase 2', status: 'completed' },
      { id: 'p3-2', content: 'Phase 3', status: 'pending' },
    ]);

    expect(new Set(items.map(item => item.key)).size).toBe(items.length);
    expect(items.map(item => item.key)).toEqual([
      '[truncated for session view]-0',
      '[truncated for session view]-1',
      'p3-2',
    ]);
  });
});
