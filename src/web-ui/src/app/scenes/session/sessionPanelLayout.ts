import { appManager } from '../../services/AppManager';
import { loadPanelWidth, RIGHT_PANEL_CONFIG, STORAGE_KEYS } from '../../layout/panelConfig';

/** Explicit, synchronous layout writes remain safe when open requests repeat. */
export function expandSessionAuxPane(): void {
  if (!appManager.getState().layout.rightPanelCollapsed) return;

  window.dispatchEvent(new CustomEvent('expand-right-panel-immediate', {
    detail: { noAnimation: true },
  }));
  appManager.updateLayout({
    rightPanelWidth: loadPanelWidth(
      STORAGE_KEYS.RIGHT_PANEL_LAST_WIDTH,
      RIGHT_PANEL_CONFIG.COMFORTABLE_DEFAULT,
    ),
    rightPanelCollapsed: false,
  });
}

export function collapseSessionAuxPane(): void {
  // In editor mode this pane is the main content surface.
  if (appManager.getState().layout.chatCollapsed) return;
  appManager.updateLayout({ rightPanelCollapsed: true });
}

export function expandSessionBottomTerminalPane(height: number): void {
  appManager.updateLayout({
    bottomTerminalPanelHeight: height,
    bottomTerminalPanelCollapsed: false,
  });
}

export function collapseSessionBottomTerminalPane(): void {
  appManager.updateLayout({ bottomTerminalPanelCollapsed: true });
}
