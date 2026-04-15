/**
 * Global shortcuts for scene/overlay quick-open actions.
 *
 *   Mod+Shift+A — return to Agentic OS base session (close overlay)
 *   Mod+,       — open Settings overlay
 *   Mod+Shift+` — open Terminal overlay
 */

import { useCallback } from 'react';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { useOverlayStore } from '@/app/stores/overlayStore';
import type { OverlaySceneId } from '@/app/overlay/types';

function openOverlayById(id: OverlaySceneId): void {
  useOverlayStore.getState().openOverlay(id);
}

function closeActiveOverlay(): void {
  useOverlayStore.getState().closeOverlay();
}

export function useGlobalSceneShortcuts(): void {
  const openSettings = useCallback(() => openOverlayById('settings'), []);
  const openTerminal = useCallback(() => openOverlayById('terminal'), []);
  const returnToSession = useCallback(() => closeActiveOverlay(), []);

  // Return to Agentic OS base session
  useShortcut(
    'scene.openSession',
    { key: 'A', ctrl: true, shift: true, scope: 'app', allowInInput: true },
    returnToSession,
    { priority: 10, description: 'keyboard.shortcuts.scene.openSession' }
  );

  // Open Settings overlay
  useShortcut(
    'scene.openSettings',
    { key: ',', ctrl: true, scope: 'app', allowInInput: true },
    openSettings,
    { priority: 10, description: 'keyboard.shortcuts.scene.openSettings' }
  );

  // Open Terminal overlay
  useShortcut(
    'scene.openTerminal',
    { key: '`', ctrl: true, shift: true, scope: 'app', allowInInput: true },
    openTerminal,
    { priority: 10, description: 'keyboard.shortcuts.scene.openTerminal' }
  );
}
