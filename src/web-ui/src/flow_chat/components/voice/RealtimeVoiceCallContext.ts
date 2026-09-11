import { createContext, useContext } from 'react';
import type { RealtimeVoiceCallController } from './useRealtimeVoiceCall';

export const RealtimeVoiceCallContext = createContext<RealtimeVoiceCallController | null>(null);

export function useRealtimeVoiceCall(): RealtimeVoiceCallController {
  const controller = useContext(RealtimeVoiceCallContext);
  if (!controller) {
    throw new Error('useRealtimeVoiceCall must be used inside RealtimeVoiceCallProvider');
  }
  return controller;
}

/** Tests and isolated composer previews may render without the client shell. */
export function useRealtimeVoiceCallActive(): boolean {
  const controller = useContext(RealtimeVoiceCallContext);
  return Boolean(controller && controller.phase !== 'idle');
}
