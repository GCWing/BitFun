/**
 * FlowChat scroll layout: floating ChatInput + message list footer / scroll-to-latest.
 * Keep footer spacer and overlay controls aligned on the same geometric model.
 */

/** Matches `.openbitfun-chat-input-drop-zone { bottom: $size-gap-6 }` — viewport inset under the composer. */
export const CHAT_INPUT_DROP_ZONE_BOTTOM_PX = 24;

/** Space between the top edge of the input block and the end of scroll content */
export const FLOWCHAT_MESSAGE_TAIL_CLEARANCE_PX = 24;

/** Space above the scroll-to-latest control (tighter than message tail; sits in overlay) */
export const SCROLL_TO_LATEST_INPUT_CLEARANCE_PX = 6;

const NORMAL_INPUT_BLOCK_SAFE_PX = 96;

/**
 * Distance from the viewport bottom to the top edge of the floating ChatInput.
 * This is also the point where transcript content must be fully transparent.
 */
export function computeFlowChatInputOverlayInsetPx(
  measuredInputHeight: number,
): number {
  const inputBlock = measuredInputHeight > 0
    ? measuredInputHeight
    : NORMAL_INPUT_BLOCK_SAFE_PX;
  return inputBlock + CHAT_INPUT_DROP_ZONE_BOTTOM_PX;
}

/**
 * Height of the footer spacer needed so the last message clears the floating input.
 * `measuredInputHeight` is the drop-zone `offsetHeight` from ChatInput (excluding the viewport bottom inset in `CHAT_INPUT_DROP_ZONE_BOTTOM_PX`).
 *
 * The footer reflects only the current layout. It does not retain an earlier
 * input height or manufacture scroll range to protect a viewport anchor.
 */
export function computeFlowChatInputStackFooterPx(
  measuredInputHeight: number,
): number {
  return computeFlowChatInputOverlayInsetPx(measuredInputHeight)
    + FLOWCHAT_MESSAGE_TAIL_CLEARANCE_PX;
}
