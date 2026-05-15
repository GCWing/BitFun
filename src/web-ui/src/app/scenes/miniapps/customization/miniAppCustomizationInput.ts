export interface MiniAppCustomizationKeyEvent {
  key: string;
  shiftKey?: boolean;
  isComposing?: boolean;
  nativeEvent?: {
    isComposing?: boolean;
  };
}

export function shouldSubmitMiniAppCustomizationRequest(
  event: MiniAppCustomizationKeyEvent,
): boolean {
  return event.key === 'Enter'
    && event.shiftKey !== true
    && event.isComposing !== true
    && event.nativeEvent?.isComposing !== true;
}
