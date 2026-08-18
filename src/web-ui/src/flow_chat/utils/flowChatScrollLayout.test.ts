import { describe, expect, it } from 'vitest';
import {
  CHAT_INPUT_DROP_ZONE_BOTTOM_PX,
  computeFlowChatInputStackFooterPx,
  FLOWCHAT_MESSAGE_TAIL_CLEARANCE_PX,
} from './flowChatScrollLayout';

describe('computeFlowChatInputStackFooterPx', () => {
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
