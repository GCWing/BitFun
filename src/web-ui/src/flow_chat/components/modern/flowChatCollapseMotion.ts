/**
 * Shared FlowChat collapse / expand motion contract.
 *
 * Auto-collapse and manual toggles share one duration so VirtualMessageList
 * collapse-intent protection can cover the full animated height change.
 */

export const FLOWCHAT_COLLAPSE_DURATION_MS = 300;

export const FLOWCHAT_COLLAPSE_EASING = 'cubic-bezier(0.4, 0, 0.2, 1)';

/** Extra rAF frames after the CSS duration before finalizing an auto collapse intent. */
export const FLOWCHAT_AUTO_COLLAPSE_SETTLE_FRAMES = 4;

/** Hard backup TTL; must stay above duration + settle margin. */
export const FLOWCHAT_COLLAPSE_INTENT_TTL_MS = 1000;
