/**
 * useKeyboardShortcuts Hook
 * Keyboard shortcut system.
 */

import { useEffect, useCallback } from 'react';
import { useCanvasStore } from '../stores';
import type { EditorGroupId } from '../types';

/**
 * Shortcut definitions.
 */
const SHORTCUTS = {
  // Mission control
  'mod+tab': 'toggleMissionControl',
  
  // Split layout
  'mod+\\': 'toggleHorizontalSplit',
  'mod+shift+\\': 'toggleVerticalSplit',
  
  // Anchor zone
  'mod+`': 'toggleAnchorZone',
  
  // Maximize
  'mod+shift+m': 'maximizeEditor',
  
  // Tab actions
  'mod+w': 'closeCurrentTab',
  'mod+shift+t': 'reopenClosedTab',
  'mod+1': 'switchToTab1',
  'mod+2': 'switchToTab2',
  'mod+3': 'switchToTab3',
  'mod+4': 'switchToTab4',
  'mod+5': 'switchToTab5',
  'mod+6': 'switchToTab6',
  'mod+7': 'switchToTab7',
  'mod+8': 'switchToTab8',
  'mod+9': 'switchToLastTab',
} as const;

type ShortcutAction = typeof SHORTCUTS[keyof typeof SHORTCUTS];

interface UseKeyboardShortcutsOptions {
  /** Whether enabled */
  enabled?: boolean;
  /** Dirty-check callback before closing tab */
  handleCloseWithDirtyCheck?: (tabId: string, groupId: EditorGroupId) => Promise<boolean>;
}

/**
 * Check if modifier keys are pressed.
 */
const checkModifiers = (e: KeyboardEvent, mod: boolean, shift: boolean): boolean => {
  const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
  const modPressed = isMac ? e.metaKey : e.ctrlKey;
  
  if (mod && !modPressed) return false;
  if (shift && !e.shiftKey) return false;
  if (!shift && e.shiftKey && mod) return false; // Shift pressed when not required
  
  return true;
};

/**
 * Keyboard shortcuts hook.
 */
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
  // Execute shortcut action
  const executeAction = useCallback((action: ShortcutAction) => {
    const activeGroup = activeGroupId === 'primary' ? primaryGroup : secondaryGroup;
    const visibleTabs = activeGroup.tabs.filter(t => !t.isHidden);

    switch (action) {
      case 'toggleMissionControl':
        toggleMissionControl();
        break;
        
      case 'toggleHorizontalSplit':
        setSplitMode(layout.splitMode === 'horizontal' ? 'none' : 'horizontal');
        break;
        
      case 'toggleVerticalSplit':
        setSplitMode(layout.splitMode === 'vertical' ? 'none' : 'vertical');
        break;
        
      case 'toggleAnchorZone':
        setAnchorPosition(layout.anchorPosition === 'hidden' ? 'bottom' : 'hidden');
        break;
        
      case 'maximizeEditor':
        toggleMaximize();
        break;
        
      case 'closeCurrentTab':
        if (activeGroup.activeTabId) {
          if (handleCloseWithDirtyCheck) {
            handleCloseWithDirtyCheck(activeGroup.activeTabId, activeGroupId);
          } else {
            closeTab(activeGroup.activeTabId, activeGroupId);
          }
        }
        break;
        
      case 'reopenClosedTab':
        reopenClosedTab();
        break;
        
      case 'switchToTab1':
      case 'switchToTab2':
      case 'switchToTab3':
      case 'switchToTab4':
      case 'switchToTab5':
      case 'switchToTab6':
      case 'switchToTab7':
      case 'switchToTab8': {
        const tabIndex = parseInt(action.replace('switchToTab', '')) - 1;
        if (visibleTabs[tabIndex]) {
          switchToTab(visibleTabs[tabIndex].id, activeGroupId);
        }
        break;
      }
        
      case 'switchToLastTab':
        if (visibleTabs.length > 0) {
          switchToTab(visibleTabs[visibleTabs.length - 1].id, activeGroupId);
        }
        break;
    }
  }, [
    activeGroupId,
    handleCloseWithDirtyCheck,
    primaryGroup,
    secondaryGroup,
    layout,
    toggleMissionControl,
    setSplitMode,
    setAnchorPosition,
    toggleMaximize,
    closeTab,
    reopenClosedTab,
    switchToTab,
  ]);

  // Keyboard event handling
  useEffect(() => {
    if (!enabled) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Skip most shortcuts when an input is focused
      const activeElement = document.activeElement;
      const isInputFocused = activeElement instanceof HTMLInputElement ||
        activeElement instanceof HTMLTextAreaElement ||
        (activeElement as HTMLElement)?.isContentEditable;

      // Some shortcuts should still work while typing
      const alwaysHandle = ['mod+tab', 'mod+w', 'mod+shift+t'];

      // Check each shortcut
      for (const [shortcut, action] of Object.entries(SHORTCUTS)) {
        const parts = shortcut.split('+');
        const key = parts[parts.length - 1];
        const needsMod = parts.includes('mod');
        const needsShift = parts.includes('shift');

        // Check key
        if (e.key.toLowerCase() !== key.toLowerCase() && 
            e.key !== key && 
            e.code !== `Key${key.toUpperCase()}` &&
            e.code !== `Digit${key}` &&
            e.key !== '`' && key !== '`') {
          continue;
        }

        // Special handling for Tab
        if (key === 'tab' && e.key !== 'Tab') continue;
        
        // Special handling for backtick
        if (key === '`' && e.key !== '`') continue;

        // Check modifiers
        if (!checkModifiers(e, needsMod, needsShift)) {
          continue;
        }

        // Only handle certain shortcuts when input is focused
        if (isInputFocused && !alwaysHandle.includes(shortcut)) {
          continue;
        }

        // Prevent default and execute action
        e.preventDefault();
        executeAction(action);
        return;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [enabled, executeAction]);

  return {
    executeAction,
  };
};

export default useKeyboardShortcuts;
