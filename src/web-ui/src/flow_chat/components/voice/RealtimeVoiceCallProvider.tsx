import type { PropsWithChildren } from 'react';
import { useRealtimeVoiceCallController } from './useRealtimeVoiceCall';
import { RealtimeVoiceCallContext } from './RealtimeVoiceCallContext';

/**
 * Owns one realtime voice call for the whole client. Keeping this above the
 * scene/workspace views lets a call and its Agent task survive navigation and
 * workspace switches.
 */
export function RealtimeVoiceCallProvider({ children }: PropsWithChildren) {
  const controller = useRealtimeVoiceCallController();
  return (
    <RealtimeVoiceCallContext.Provider value={controller}>
      {children}
    </RealtimeVoiceCallContext.Provider>
  );
}
