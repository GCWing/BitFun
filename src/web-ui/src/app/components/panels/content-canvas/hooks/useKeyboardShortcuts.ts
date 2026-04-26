/**
 * useKeyboardShortcuts Hook
 *
 * Registers canvas-level keyboard shortcuts via ShortcutManager.
 * All shortcuts use scope 'canvas' so they only fire when focus is inside
 * the editor canvas area (data-shortcut-scope="canvas").
 */

import { useCallback } from 'react';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { activeEditTargetService } from '@/tools/editor/services/ActiveEditTargetService';
import { useCanvasStore } from '../stores';
import type { EditorGroupId } from '../types';

interface UseKeyboardShortcutsOptions {
  enabled?: boolean;
  handleCloseWithDirtyCheck?: (tabId: string, groupId: EditorGroupId) => Promise<boolean>;
}

export const useKeyboardShortcuts = (options: UseKeyboardShortcutsOptions = {}) => {
  const { enabled = true, handleCloseWithDirtyCheck } = options;

  const {
    primaryGroup,
    secondaryGroup,
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

  const getActiveGroup = useCallback(() => {
    return activeGroupId === 'primary' ? primaryGroup : secondaryGroup;
  }, [activeGroupId, primaryGroup, secondaryGroup]);

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
    () => window.dispatchEvent(new CustomEvent('closePreview')),
    { enabled, priority: 5, description: 'keyboard.shortcuts.canvas.closePreview' }
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
