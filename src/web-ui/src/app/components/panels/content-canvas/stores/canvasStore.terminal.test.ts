import { beforeEach, describe, expect, it } from 'vitest';
import { useAgentCanvasStore } from './canvasStore';

describe('canvasStore terminal lifecycle', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  it('removes a closed terminal tab instead of hiding or recording it', () => {
    const store = useAgentCanvasStore.getState();
    store.addTab({
      type: 'terminal',
      title: 'Shell 1',
      data: { sessionId: 'terminal-1', sessionName: 'Shell 1' },
      metadata: { sessionId: 'terminal-1' },
    }, 'active', 'primary');

    const tabId = useAgentCanvasStore.getState().primaryGroup.tabs[0].id;
    useAgentCanvasStore.getState().closeTab(tabId, 'primary');

    const next = useAgentCanvasStore.getState();
    expect(next.primaryGroup.tabs).toEqual([]);
    expect(next.closedTabs).toEqual([]);
  });
});
