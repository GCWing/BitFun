/**
 * useKeyboardShortcuts Hook
 *
 * Registers canvas-level keyboard shortcuts via ShortcutManager.
 * All shortcuts use scope 'canvas' so they only fire when focus is inside
 * the editor canvas area (data-shortcut-scope="canvas").
 */

import { useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useHasDismissibleLayer } from '@/infrastructure/hooks/useDismissibleLayer';
import { dismissibleLayerManager } from '@/infrastructure/services/DismissibleLayerManager';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { notificationService } from '@/shared/notification-system';
import { activeEditTargetService } from '@/tools/editor/services/ActiveEditTargetService';
import { useCanvasStore, useAgentCanvasStore, GROUP_STATE_KEY } from '../stores';
import type { EditorGroupId, EditorGroupState } from '../types';

interface UseKeyboardShortcutsOptions {
  enabled?: boolean;
  handleCloseWithDirtyCheck?: (tabId: string, groupId: EditorGroupId) => Promise<boolean>;
}

export const useKeyboardShortcuts = (options: UseKeyboardShortcutsOptions = {}) => {
  const { enabled = true, handleCloseWithDirtyCheck } = options;
  const hasCanvasDismissibleLayer = useHasDismissibleLayer('canvas');
  const { t } = useTranslation('components');

  const {
    primaryGroup,
    secondaryGroup,
    tertiaryGroup,
    slot4Group,
    slot5Group,
    slot6Group,
    slot7Group,
    slot8Group,
    slot9Group,
    slot10Group,
    slot11Group,
    slot12Group,
    slot13Group,
    slot14Group,
    slot15Group,
    slot16Group,
    activeGroupId,
    layout,
    closeTab,
    switchToTab,
    reopenClosedTab,
    setSplitMode,
    setAnchorPosition,
    toggleMaximize,
    toggleMissionControl,
  } = useCanvasStore();

  // Keyed by GROUP_STATE_KEY so getActiveGroup can resolve any of the 16
  // editor groups through the same mapping canvasStore uses.
  const groups: Record<string, EditorGroupState> = useMemo(() => ({
    primaryGroup,
    secondaryGroup,
    tertiaryGroup,
    slot4Group,
    slot5Group,
    slot6Group,
    slot7Group,
    slot8Group,
    slot9Group,
    slot10Group,
    slot11Group,
    slot12Group,
    slot13Group,
    slot14Group,
    slot15Group,
    slot16Group,
  }), [
    primaryGroup,
    secondaryGroup,
    tertiaryGroup,
    slot4Group,
    slot5Group,
    slot6Group,
    slot7Group,
    slot8Group,
    slot9Group,
    slot10Group,
    slot11Group,
    slot12Group,
    slot13Group,
    slot14Group,
    slot15Group,
    slot16Group,
  ]);

  // Resolve the active group through the shared GROUP_STATE_KEY mapping so
  // Ctrl+W (tab.close) works in any of the 16 grid9 cells (slot4..slot16),
  // not just primary/secondary. All 16 group fields are subscribed and
  // forwarded through the mapping, so the callback stays mode-aware (reads
  // the same useCanvasStore values this hook is subscribed to) and re-binds
  // whenever any group's tabs change.
  const getActiveGroup = useCallback(() => {
    return groups[GROUP_STATE_KEY[activeGroupId]];
  }, [activeGroupId, groups]);

  const getVisibleTabs = useCallback(() => {
    return getActiveGroup().tabs.filter((t) => !t.isHidden);
  }, [getActiveGroup]);

  // Find in file (Monaco) — only when `data-shortcut-scope="editor"` is innermost
  useShortcut(
    'editor.findInFile',
    { key: 'f', ctrl: true, scope: 'editor', allowInInput: true },
    () => {
      activeEditTargetService.openMonacoFind();
    },
    { enabled, priority: 20, description: 'keyboard.shortcuts.editor.findInFile' }
  );

  // Mission control
  useShortcut(
    'canvas.missionControl',
    { key: 'Tab', ctrl: true, scope: 'canvas', allowInInput: true },
    () => toggleMissionControl(),
    { enabled, priority: 10, description: 'keyboard.shortcuts.canvas.missionControl' }
  );

  // Horizontal split: mod+\
  useShortcut(
    'canvas.splitHorizontal',
    { key: '\\', ctrl: true, scope: 'canvas' },
    () => setSplitMode(layout.splitMode === 'horizontal' ? 'none' : 'horizontal'),
    { enabled, description: 'keyboard.shortcuts.canvas.splitHorizontal' }
  );

  // Vertical split: mod+Shift+\
  useShortcut(
    'canvas.splitVertical',
    { key: '\\', ctrl: true, shift: true, scope: 'canvas' },
    () => setSplitMode(layout.splitMode === 'vertical' ? 'none' : 'vertical'),
    { enabled, description: 'keyboard.shortcuts.canvas.splitVertical' }
  );

  // 3x3 grid (grid9): mod+Shift+9 — cycle grid9 on/off.
  // Key deliberately avoids mod+Shift+G (scene.openGit, app scope) and is
  // registered in BOTH canvas and chat scopes so it fires whether focus is in
  // the auxiliary canvas or the center chat pane (chat does not inherit canvas
  // scope in ShortcutManager.findCandidates).
  const toggleGrid9 = useCallback(() => {
    // The auxiliary canvas runs in 'agent' mode; read its live state for the
    // empty-canvas check (no tabs → show a hint instead of a silent no-op).
    const hasTabs = useAgentCanvasStore.getState().getAllTabs().length > 0;
    if (!hasTabs) {
      notificationService.info(t('canvas.grid9EmptyHint'), { duration: 3000 });
      return;
    }
    setSplitMode(layout.splitMode === 'grid9' ? 'none' : 'grid9');
  }, [layout.splitMode, setSplitMode, t]);
  useShortcut(
    'canvas.splitGrid9',
    { key: '9', ctrl: true, shift: true, scope: 'canvas' },
    toggleGrid9,
    { enabled, description: 'keyboard.shortcuts.canvas.splitGrid9' }
  );
  useShortcut(
    'canvas.splitGrid9.chat',
    { key: '9', ctrl: true, shift: true, scope: 'chat' },
    toggleGrid9,
    { enabled, description: 'keyboard.shortcuts.canvas.splitGrid9' }
  );

  // Anchor zone: mod+`
  useShortcut(
    'canvas.anchorZone',
    { key: '`', ctrl: true, scope: 'canvas' },
    () => setAnchorPosition(layout.anchorPosition === 'hidden' ? 'bottom' : 'hidden'),
    { enabled, description: 'keyboard.shortcuts.canvas.anchorZone' }
  );

  // Maximize: mod+Shift+M
  useShortcut(
    'canvas.maximize',
    { key: 'M', ctrl: true, shift: true, scope: 'canvas' },
    () => toggleMaximize(),
    { enabled, description: 'keyboard.shortcuts.canvas.maximize' }
  );

  // Close canvas preview/modal overlay: Escape
  useShortcut(
    'canvas.closePreview',
    { key: 'Escape', scope: 'canvas', allowInInput: true },
    () => {
      dismissibleLayerManager.dismissTop('canvas');
    },
    {
      enabled: enabled && hasCanvasDismissibleLayer,
      priority: 5,
      description: 'keyboard.shortcuts.canvas.closePreview',
    }
  );

  // Close current tab: mod+W
  useShortcut(
    'tab.close',
    { key: 'W', ctrl: true, scope: 'canvas', allowInInput: true },
    () => {
      const activeGroup = getActiveGroup();
      if (!activeGroup.activeTabId) return;
      if (handleCloseWithDirtyCheck) {
        handleCloseWithDirtyCheck(activeGroup.activeTabId, activeGroupId);
      } else {
        closeTab(activeGroup.activeTabId, activeGroupId);
      }
    },
    { enabled, priority: 10, description: 'keyboard.shortcuts.tab.close' }
  );

  // Reopen closed tab: mod+Shift+T
  useShortcut(
    'tab.reopenClosed',
    { key: 'T', ctrl: true, shift: true, scope: 'canvas', allowInInput: true },
    () => reopenClosedTab(),
    { enabled, priority: 10, description: 'keyboard.shortcuts.tab.reopenClosed' }
  );

  // Switch to tab by number: mod+1~9
  const switchToTabByIndex = useCallback(
    (index: number) => {
      const tabs = getVisibleTabs();
      const target = index === -1 ? tabs[tabs.length - 1] : tabs[index];
      if (target) switchToTab(target.id, activeGroupId);
    },
    [getVisibleTabs, switchToTab, activeGroupId]
  );

  // allowInInput so Ctrl+1..9 still work while focus is in a Monaco editor
  useShortcut('tab.switch1',    { key: '1', ctrl: true, scope: 'canvas', allowInInput: true }, () => switchToTabByIndex(0),  { enabled, description: 'keyboard.shortcuts.tab.switchMerged' });
  useShortcut('tab.switch2',    { key: '2', ctrl: true, scope: 'canvas', allowInInput: true }, () => switchToTabByIndex(1),  { enabled, description: 'keyboard.shortcuts.tab.switchMerged' });
  useShortcut('tab.switch3',    { key: '3', ctrl: true, scope: 'canvas', allowInInput: true }, () => switchToTabByIndex(2),  { enabled, description: 'keyboard.shortcuts.tab.switchMerged' });
  useShortcut('tab.switch4',    { key: '4', ctrl: true, scope: 'canvas', allowInInput: true }, () => switchToTabByIndex(3),  { enabled, description: 'keyboard.shortcuts.tab.switchMerged' });
  useShortcut('tab.switch5',    { key: '5', ctrl: true, scope: 'canvas', allowInInput: true }, () => switchToTabByIndex(4),  { enabled, description: 'keyboard.shortcuts.tab.switchMerged' });
  useShortcut('tab.switch6',    { key: '6', ctrl: true, scope: 'canvas', allowInInput: true }, () => switchToTabByIndex(5),  { enabled, description: 'keyboard.shortcuts.tab.switchMerged' });
  useShortcut('tab.switch7',    { key: '7', ctrl: true, scope: 'canvas', allowInInput: true }, () => switchToTabByIndex(6),  { enabled, description: 'keyboard.shortcuts.tab.switchMerged' });
  useShortcut('tab.switch8',    { key: '8', ctrl: true, scope: 'canvas', allowInInput: true }, () => switchToTabByIndex(7),  { enabled, description: 'keyboard.shortcuts.tab.switchMerged' });
  useShortcut('tab.switchLast', { key: '9', ctrl: true, scope: 'canvas', allowInInput: true }, () => switchToTabByIndex(-1), { enabled, description: 'keyboard.shortcuts.tab.switchMerged' });
};

export default useKeyboardShortcuts;
