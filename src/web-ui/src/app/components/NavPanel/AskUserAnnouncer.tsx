import { useEffect, useRef } from 'react';
import { useI18n, i18nService } from '@/infrastructure/i18n';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { stateMachineManager } from '@/flow_chat/state-machine';
import { SessionExecutionState } from '@/flow_chat/state-machine/types';
import { hasPendingAskUserQuestion, resolveTrackedTurn } from '@/flow_chat/utils/askUserQuestionState';
import { resolveSessionTitle } from '@/flow_chat/utils/sessionTitle';

type TFunc = (key: string, params?: Record<string, unknown>) => string;

/**
 * Compute an aria-live message from the previous and current sets of waiting
 * session titles. Exported as a pure function for unit testing.
 *
 * - Single add:  "Session '<name>' needs your input"
 * - Multi add:    "<n> sessions need your input"
 * - All resolved: "Sessions no longer waiting for input"
 * - Partial:     "Session '<name>' received input. <n> still waiting."
 */
export function computeAnnouncementMessage(
  prevTitles: Map<string, string>,
  currentTitles: Map<string, string>,
  t: TFunc,
): string {
  const added: string[] = [];
  const removed: string[] = [];
  for (const [id, title] of currentTitles) {
    if (!prevTitles.has(id)) added.push(title);
  }
  for (const [id, title] of prevTitles) {
    if (!currentTitles.has(id)) removed.push(title);
  }

  if (added.length > 0) {
    return added.length === 1
      ? t('nav.sessions.ariaNeedsInputWithName', { name: added[0] })
      : t('nav.sessions.ariaNeedsInputPlural', { count: currentTitles.size });
  }
  if (removed.length > 0) {
    if (currentTitles.size === 0) {
      return t('nav.sessions.ariaInputResolved');
    }
    return t('nav.sessions.ariaInputResolvedRemaining', { name: removed[0], count: currentTitles.size });
  }
  return '';
}

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
