import { useEffect, useRef } from 'react';
import { useI18n, i18nService } from '@/infrastructure/i18n';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { stateMachineManager } from '@/flow_chat/state-machine';
import { SessionExecutionState } from '@/flow_chat/state-machine/types';
import { hasPendingAskUserQuestion, resolveTrackedTurn } from '@/flow_chat/utils/askUserQuestionState';
import { resolveSessionTitle } from '@/flow_chat/utils/sessionTitle';
import { computeAnnouncementMessage } from './askUserAnnouncement';

/**
 * Collect the current set of sessions waiting for AskUserQuestion input.
 * Returns a Map of sessionId → display title. Excludes transient and
 * subagent sessions (same filter as the visible nav list).
 */
function collectWaitingTitles(): Map<string, string> {
  const state = flowChatStore.getState();
  const result = new Map<string, string>();
  for (const session of state.sessions.values()) {
    if (session.isTransient || session.sessionKind === 'subagent') continue;
    const machineState = stateMachineManager.getCurrentState(session.sessionId);
    if (
      machineState !== SessionExecutionState.PROCESSING &&
      machineState !== SessionExecutionState.FINISHING
    ) {
      continue;
    }
    if (hasPendingAskUserQuestion(resolveTrackedTurn(session))) {
      result.set(session.sessionId, resolveSessionTitle(session, (key, options) => i18nService.t(key, options)));
    }
  }
  return result;
}

/**
 * Single-instance aria-live announcer for AskUserQuestion waiting-state
 * changes. Rendered once in NavPanel to avoid duplicate announcements from
 * per-workspace SessionsSection instances.
 *
 * Uses direct DOM text-content manipulation (clear → rAF → set) to force
 * screen-reader re-announcement even when the message text is identical to
 * the previous one.
 */
export default function AskUserAnnouncer() {
  const { t } = useI18n('common');
  const liveRef = useRef<HTMLSpanElement>(null);
  const prevWaitingRef = useRef<Map<string, string>>(new Map());
  const tRef = useRef(t);
  tRef.current = t;

  useEffect(() => {
    const update = () => {
      const current = collectWaitingTitles();
      const prev = prevWaitingRef.current;
      const message = computeAnnouncementMessage(prev, current, tRef.current);

      if (message && liveRef.current) {
        // Clear then set on next frame to force screen-reader re-announcement
        // even when the message text is identical to the previous one.
        liveRef.current.textContent = '';
        requestAnimationFrame(() => {
          if (liveRef.current) {
            liveRef.current.textContent = message;
          }
        });
      }

      prevWaitingRef.current = current;
    };

    update();
    const unsubStore = flowChatStore.subscribe(update);
    const unsubMachines = stateMachineManager.subscribeGlobal(update);
    return () => {
      unsubStore();
      unsubMachines();
    };
  }, []);

  return (
    <span ref={liveRef} role="status" aria-live="polite" className="sr-only" />
  );
}
