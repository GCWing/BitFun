/**
 * Global shortcuts for scene / overlay actions (catalog-driven keys — ⌘ on macOS, Ctrl on Win/Linux).
 *
 *   Mod+,       — Settings overlay
 *   Mod+Shift+` — Terminal overlay
 */

import { useCallback } from 'react';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { ALL_SHORTCUTS } from '@/shared/constants/shortcuts';
import { useOverlayStore } from '@/app/stores/overlayStore';
import type { OverlaySceneId } from '@/app/overlay/types';

const shortcut = (id: string) => ALL_SHORTCUTS.find((d) => d.id === id)!;

function openOverlayById(id: OverlaySceneId): void {
  useOverlayStore.getState().openOverlay(id);
}

export function useGlobalSceneShortcuts(): void {
  const openSettings = useCallback(() => openOverlayById('settings'), []);
  const openTerminal = useCallback(() => openOverlayById('terminal'), []);

  const dOpenSettings = shortcut('scene.openSettings');
  useShortcut(dOpenSettings.id, dOpenSettings.config, openSettings, {
    priority: 10,
    description: dOpenSettings.descriptionKey,
  });

  const dOpenTerminal = shortcut('scene.openTerminal');
  useShortcut(dOpenTerminal.id, dOpenTerminal.config, openTerminal, {
    priority: 10,
    description: dOpenTerminal.descriptionKey,
  });
}
