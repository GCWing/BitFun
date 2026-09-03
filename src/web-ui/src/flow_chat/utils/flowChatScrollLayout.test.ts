import { describe, expect, it } from 'vitest';
import {
  CHAT_INPUT_DROP_ZONE_BOTTOM_PX,
  computeFlowChatInputOverlayInsetPx,
  computeFlowChatInputStackFooterPx,
  FLOWCHAT_MESSAGE_TAIL_CLEARANCE_PX,
} from './flowChatScrollLayout';

describe('computeFlowChatInputOverlayInsetPx', () => {
  it('locates the ChatInput top edge from its measured height and viewport gap', () => {
    expect(computeFlowChatInputOverlayInsetPx(40)).toBe(
      40 + CHAT_INPUT_DROP_ZONE_BOTTOM_PX,
    );
  });

  it('shares the safe pre-measurement input height with the footer layout', () => {
    expect(computeFlowChatInputOverlayInsetPx(0)).toBe(
      96 + CHAT_INPUT_DROP_ZONE_BOTTOM_PX,
    );
  });
});

describe('computeFlowChatInputStackFooterPx', () => {
  it('keeps the runtime scroll inset aligned with the 24px composer bottom gap', () => {
    expect(CHAT_INPUT_DROP_ZONE_BOTTOM_PX).toBe(24);
  });

  it('uses the current measured input height without retaining an active height', () => {
    expect(computeFlowChatInputStackFooterPx(40)).toBe(
      40 + CHAT_INPUT_DROP_ZONE_BOTTOM_PX + FLOWCHAT_MESSAGE_TAIL_CLEARANCE_PX,
    );
  });

  it('uses a safe layout fallback only before an input height is measured', () => {
    expect(computeFlowChatInputStackFooterPx(0)).toBe(
      96 + CHAT_INPUT_DROP_ZONE_BOTTOM_PX + FLOWCHAT_MESSAGE_TAIL_CLEARANCE_PX,
    );
  });
});
