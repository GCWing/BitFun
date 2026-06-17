/**
 * FlowChat scroll layout: floating ChatInput + message list footer / scroll-to-latest.
 * Keep footer spacer and overlay controls aligned on the same geometric model.
 */

/** Matches `.bitfun-chat-input-drop-zone { bottom: … }` — viewport inset under workspace strip. */
export const CHAT_INPUT_DROP_ZONE_BOTTOM_PX = 4;

/** Space between the top edge of the input block and the end of scroll content */
export const FLOWCHAT_MESSAGE_TAIL_CLEARANCE_PX = 24;

/** Space above the scroll-to-latest control (tighter than message tail; sits in overlay) */
export const SCROLL_TO_LATEST_INPUT_CLEARANCE_PX = 6;

const FALLBACK_INPUT_BLOCK_ACTIVE_PX = 96;
const NORMAL_INPUT_BLOCK_SAFE_PX = 96;

/**
 * Height of the Virtuoso footer spacer needed so the last message clears the floating input.
 * `measuredInputHeight` is the drop-zone `offsetHeight` from ChatInput (excluding the viewport bottom inset in `CHAT_INPUT_DROP_ZONE_BOTTOM_PX`).
 *
 * When `isInputActive` transitions from `true` to `false` (user sends a message,
 * input collapses to capsule), the ResizeObserver on the drop-zone fires with the
 * *new* collapsed height on the same microtask. If we use that new height
 * immediately, the Virtuoso footer shrinks by ~40 px in one frame, the browser
 * clamps `scrollTop` downward, and the viewport briefly shows blank space at the
 * top ("white screen"). The user must scroll up to see content again.
 *
 * To avoid this, when the input is *not* active we ignore the measured height
 * and use the active fallback instead. The footer stays large enough to cover
 * the active input block until the next turn's content grows past it, at which
 * point the footer reservation is consumed organically by the grow branch of
 * `measureHeightChange`. The visual cost is a few extra pixels of blank tail
 * space at the bottom — invisible to the user.
 */
export function computeFlowChatInputStackFooterPx(
  measuredInputHeight: number,
  isInputActive: boolean,
): number {
  // When the input is collapsed, always use the active fallback so the footer
  // does not shrink on the same frame as the input collapse transition. This
  // prevents a scrollTop clamp that would push the viewport above the content.
  const effectiveInputHeight = isInputActive
    ? measuredInputHeight
    : FALLBACK_INPUT_BLOCK_ACTIVE_PX;

  const measuredInputBlock = effectiveInputHeight > 0
    ? effectiveInputHeight
    : FALLBACK_INPUT_BLOCK_ACTIVE_PX;
  const inputBlock = Math.max(measuredInputBlock, NORMAL_INPUT_BLOCK_SAFE_PX);
  return inputBlock + CHAT_INPUT_DROP_ZONE_BOTTOM_PX + FLOWCHAT_MESSAGE_TAIL_CLEARANCE_PX;
}
