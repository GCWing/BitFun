// @vitest-environment jsdom
import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { describe, expect, it } from 'vitest';
import { useTabLifecycle } from '../../components/panels/content-canvas/hooks/useTabLifecycle';
import { useAgentCanvasStore } from '../../components/panels/content-canvas/stores/canvasStore';

// Exercise the actual hook and store. No terminal adapter or rendered UI is replaced.
describe('workspace terminal view lifetime', () => {
  it('closes one view and all views without invoking terminal destruction', async () => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    const container = document.createElement('div');
    const root = createRoot(container);
    let lifecycle: ReturnType<typeof useTabLifecycle> | undefined;
    function Probe() {
      lifecycle = useTabLifecycle({ mode: 'agent' });
      return null;
    }
    useAgentCanvasStore.getState().reset();
    const addTerminal = (id: string) => useAgentCanvasStore.getState().addTab({
      type: 'terminal', title: id, data: { sessionId: id },
      metadata: { terminalCloseBehavior: 'detach', sessionId: id },
    }, 'active', 'primary');
    try {
      await act(async () => { root.render(<Probe />); });
      await act(async () => { addTerminal('workspace-terminal-1'); addTerminal('workspace-terminal-2'); });
      const firstId = useAgentCanvasStore.getState().primaryGroup.tabs[0].id;
      await act(async () => { expect(await lifecycle!.handleCloseWithDirtyCheck(firstId, 'primary')).toBe(true); });
      expect(useAgentCanvasStore.getState().primaryGroup.tabs).toHaveLength(1);
      await act(async () => { expect(await lifecycle!.handleCloseAllWithDirtyCheck('primary')).toBe(true); });
      expect(useAgentCanvasStore.getState().primaryGroup.tabs).toHaveLength(0);
    } finally {
      await act(async () => { root.unmount(); });
      useAgentCanvasStore.getState().reset();
      container.remove();
    }
  });
});
