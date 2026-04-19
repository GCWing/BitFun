/**
 * Global Escape — return toward the Agentic OS base (Dispatcher) session:
 *
 * 1. Scene overlay open → close overlay (base session visible again).
 * 2. No overlay, but active chat session is not Agentic OS Dispatcher → switch to Dispatcher.
 *
 * Registered in capture phase on window before ShortcutManager. Lets ShortcutManager handle
 * the same Esc when chat is focused and the session is still processing (stop generation),
 * and when nested UI should consume Esc first.
 */

import { useOverlayStore } from './stores/overlayStore';
import { shortcutManager } from '@/infrastructure/services/ShortcutManager';
import { ALL_SHORTCUTS } from '@/shared/constants/shortcuts';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { stateMachineManager } from '@/flow_chat/state-machine';
import { SessionExecutionState } from '@/flow_chat/state-machine/types';
import type { Session } from '@/flow_chat/types/flow-chat';
import { openDispatcherSession } from '@/flow_chat/services/openDispatcherSession';

const ESCAPE_TO_AGENTIC_BASE_DEF = ALL_SHORTCUTS.find((d) => d.id === 'scene.escapeToAgenticBase');

const INNER_ESCAPE_ROOT_SELECTORS = [
  '.modal-overlay',
  '.bitfun-nav-search-dialog__overlay',
] as const;

const GLOBAL_DEFER_SELECTORS = ['.select__dropdown', '.context-menu'] as const;

function shouldDeferForNestedEscapeUi(event: KeyboardEvent): boolean {
  const target = event.target;
  if (target instanceof Element) {
    for (const sel of INNER_ESCAPE_ROOT_SELECTORS) {
      if (target.closest(sel)) return true;
    }
  }
  for (const sel of GLOBAL_DEFER_SELECTORS) {
    if (document.querySelector(sel)) return true;
  }
  return false;
}

function isAgenticOsDispatcherSession(session: Session | null): boolean {
  if (!session) return false;
  if (session.mode?.toLowerCase() === 'dispatcher') return true;
  if (session.storageScope === 'agentic_os') return true;
  return false;
}

function isTargetInsideChatShortcutScope(target: EventTarget | null): boolean {
  if (!target || !(target instanceof Element)) return false;
  return target.closest('[data-shortcut-scope="chat"]') !== null;
}

function isActiveSessionProcessing(): boolean {
  const sid = flowChatStore.getState().activeSessionId;
  if (!sid) return false;
  return stateMachineManager.getCurrentState(sid) === SessionExecutionState.PROCESSING;
}

function eventMatchesEscapeToAgenticBinding(event: KeyboardEvent): boolean {
  if (ESCAPE_TO_AGENTIC_BASE_DEF) {
    return shortcutManager.matchesShortcutId(ESCAPE_TO_AGENTIC_BASE_DEF.id, ESCAPE_TO_AGENTIC_BASE_DEF.config, event);
  }
  return event.key === 'Escape';
}

let installed = false;

export function installGlobalOverlayEscapeToSession(): void {
  if (installed) return;
  installed = true;

  window.addEventListener(
    'keydown',
    (event: KeyboardEvent) => {
      if (!eventMatchesEscapeToAgenticBinding(event)) return;
      if (shouldDeferForNestedEscapeUi(event)) return;

      const { activeOverlay, closeOverlay } = useOverlayStore.getState();
      if (activeOverlay !== null) {
        event.preventDefault();
        event.stopImmediatePropagation();
        closeOverlay();
        return;
      }

      const state = flowChatStore.getState();
      const session = state.activeSessionId ? state.sessions.get(state.activeSessionId) ?? null : null;
      if (isAgenticOsDispatcherSession(session)) {
        return;
      }

      if (isTargetInsideChatShortcutScope(event.target) && isActiveSessionProcessing()) {
        return;
      }

      event.preventDefault();
      event.stopImmediatePropagation();
      void openDispatcherSession();
    },
    true
  );
}
