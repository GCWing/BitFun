import { describe, expect, it, vi } from 'vitest';

import { AppManager } from './AppManager';

describe('AppManager layout updates', () => {
  it('does not emit layout changes when the requested layout is already current', () => {
    const manager = new AppManager();
    const listener = vi.fn();
    manager.addEventListener(listener);
    const current = manager.getState().layout;

    manager.updateLayout({
      leftPanelActiveTab: current.leftPanelActiveTab,
      leftPanelCollapsed: current.leftPanelCollapsed,
    });

    expect(listener).not.toHaveBeenCalled();
  });

  it('emits layout changes when a layout value changes', () => {
    const manager = new AppManager();
    const listener = vi.fn();
    manager.addEventListener(listener);
    const current = manager.getState().layout;

    manager.updateLayout({
      leftPanelCollapsed: !current.leftPanelCollapsed,
    });

    expect(listener).toHaveBeenCalledWith({
      type: 'layout:changed',
      payload: {
        leftPanelCollapsed: !current.leftPanelCollapsed,
      },
    });
  });
});
