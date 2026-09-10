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

  it('keeps full terminal titles when opening old payloads and renaming a session', () => {
    const fullName = 'A terminal session with a name longer than twenty characters';
    const store = useAgentCanvasStore.getState();
    store.addTab({
      type: 'terminal', title: 'A terminal session w...',
      data: { sessionId: 'terminal-1', sessionName: fullName },
      metadata: { sessionId: 'terminal-1' },
    }, 'active', 'primary');
    expect(useAgentCanvasStore.getState().primaryGroup.tabs[0].title).toBe(fullName);
    const renamed = `${fullName} after renaming`;
    store.renameTerminalTabBySessionId('terminal-1', renamed);
    const tab = useAgentCanvasStore.getState().primaryGroup.tabs[0];
    expect(tab.title).toBe(renamed);
    expect(tab.content.title).toBe(renamed);
  });
});
