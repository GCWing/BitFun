import { configManager } from '@/infrastructure/config/services/ConfigManager';
import type { TerminalConfig, TerminalPanelPosition } from '@/infrastructure/config/types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('TerminalPanelPreferenceService');

const DEFAULT_TERMINAL_PANEL_POSITION: TerminalPanelPosition = 'right';
const VALID_POSITIONS: TerminalPanelPosition[] = ['right', 'bottom'];

let cachedPosition: TerminalPanelPosition = DEFAULT_TERMINAL_PANEL_POSITION;
let initialized = false;
const listeners = new Set<(position: TerminalPanelPosition) => void>();

function normalizeTerminalPanelPosition(value: unknown): TerminalPanelPosition {
  return VALID_POSITIONS.includes(value as TerminalPanelPosition)
    ? (value as TerminalPanelPosition)
    : DEFAULT_TERMINAL_PANEL_POSITION;
}

async function loadTerminalPanelPosition(): Promise<TerminalPanelPosition> {
  const config = await configManager.getConfig<TerminalConfig>('terminal');
  return normalizeTerminalPanelPosition(config?.terminal_panel_position);
}

function setCachedPosition(position: TerminalPanelPosition): void {
  if (cachedPosition === position && initialized) {
    return;
  }

  cachedPosition = position;
  initialized = true;
  listeners.forEach((listener) => {
    try {
      listener(position);
    } catch (error) {
      log.warn('Terminal panel position listener failed', { error });
    }
  });
}

export function getCachedTerminalPanelPosition(): TerminalPanelPosition {
  if (!initialized) {
    void refreshTerminalPanelPosition();
  }

  return cachedPosition;
}

export async function refreshTerminalPanelPosition(): Promise<TerminalPanelPosition> {
  try {
    setCachedPosition(await loadTerminalPanelPosition());
  } catch (error) {
    log.warn('Failed to load terminal panel position preference', { error });
    setCachedPosition(DEFAULT_TERMINAL_PANEL_POSITION);
  }

  return cachedPosition;
}

export async function setTerminalPanelPosition(position: TerminalPanelPosition): Promise<void> {
  const normalized = normalizeTerminalPanelPosition(position);
  await configManager.setConfig('terminal.terminal_panel_position', normalized);
  setCachedPosition(normalized);
}

export function onTerminalPanelPositionChange(
  listener: (position: TerminalPanelPosition) => void,
): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

configManager.onConfigChange((path, _oldValue, newValue) => {
  if (path === 'terminal.terminal_panel_position') {
    setCachedPosition(normalizeTerminalPanelPosition(newValue));
    return;
  }

  if (path === 'terminal') {
    setCachedPosition(normalizeTerminalPanelPosition(
      (newValue as Partial<TerminalConfig> | undefined)?.terminal_panel_position,
    ));
  }
});
