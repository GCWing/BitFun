/**
 * Subscribe to the active session state.
 * Processing status now comes from SessionStateMachine.
 */

import { useState, useEffect } from 'react';
import { flowChatStore } from '../store/FlowChatStore';
import { stateMachineManager } from '../state-machine';
import { ProcessingPhase } from '../state-machine/types';

export interface ActiveSessionState {
  sessionId: string | null;
  isProcessing: boolean;
  processingPhase: ProcessingPhase | null;
  error: string | null;
  status: 'active' | 'idle' | 'error';
}

export const useActiveSessionState = (): ActiveSessionState => {
  const [sessionState, setSessionState] = useState<ActiveSessionState>(() => {
    const session = flowChatStore.getActiveSession();
    const machine = session ? stateMachineManager.get(session.sessionId) : null;
    const isProcessing = machine ? machine.getCurrentState() === 'processing' : false;
    const processingPhase = machine ? machine.getContext().processingPhase : null;
    
    return {
      sessionId: session?.sessionId || null,
      isProcessing,
      processingPhase,
      error: session?.error || null,
      status: session?.status || 'idle'
    };
  });

  useEffect(() => {
    const unsubscribeStore = flowChatStore.subscribe((newState) => {
      const session = newState.sessions.get(newState.activeSessionId || '');
      const machine = session ? stateMachineManager.get(session.sessionId) : null;
      const isProcessing = machine ? machine.getCurrentState() === 'processing' : false;
      const processingPhase = machine ? machine.getContext().processingPhase : null;
      
      setSessionState(prev => {
        const newSessionState: ActiveSessionState = {
          sessionId: session?.sessionId || null,
          isProcessing,
          processingPhase,
          error: session?.error || null,
          status: session?.status || 'idle'
        };
        
        // Shallow compare to avoid unnecessary updates.
        if (
          prev.sessionId === newSessionState.sessionId &&
          prev.isProcessing === newSessionState.isProcessing &&
          prev.processingPhase === newSessionState.processingPhase &&
          prev.error === newSessionState.error &&
          prev.status === newSessionState.status
        ) {
          return prev;
        }
        
        return newSessionState;
      });
    });
    
    // Keep processing fields in sync with the state machine.
    const unsubscribeMachine = stateMachineManager.subscribeGlobal((sessionId, machineSnapshot) => {
      const currentSession = flowChatStore.getActiveSession();
      if (currentSession?.sessionId === sessionId) {
        const state = machineSnapshot.currentState;
        const isProcessing = state === 'processing';
        const processingPhase = machineSnapshot.context.processingPhase;
        setSessionState(prev => {
          if (prev.isProcessing === isProcessing && prev.processingPhase === processingPhase) return prev;
          return { ...prev, isProcessing, processingPhase };
        });
      }
    });

    return () => {
      unsubscribeStore();
      unsubscribeMachine();
    };
  }, []);

  return sessionState;
};

