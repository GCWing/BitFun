/**
 * Image analysis helpers for dialog turns.
 */

import type { FlowChatContext, DialogTurn } from './types';
import type { ImageContext } from '@/shared/types/context';
import type { ImageAnalysisResult } from '../../types/flow-chat';

export function addDialogTurn(
  context: FlowChatContext,
  sessionId: string,
  dialogTurn: DialogTurn
): void {
  context.flowChatStore.addDialogTurn(sessionId, dialogTurn);
}

export function addImageAnalysisPhase(
  context: FlowChatContext,
  sessionId: string,
  dialogTurnId: string,
  imageContexts: ImageContext[]
): void {
  context.flowChatStore.addImageAnalysisPhase(sessionId, dialogTurnId, imageContexts);
}

export function updateImageAnalysisResults(
  context: FlowChatContext,
  sessionId: string,
  dialogTurnId: string,
  results: ImageAnalysisResult[]
): void {
  context.flowChatStore.updateImageAnalysisResults(sessionId, dialogTurnId, results);
}

/**
 * Update a single image analysis item, typically for error handling.
 */
export function updateImageAnalysisItem(
  context: FlowChatContext,
  sessionId: string,
  dialogTurnId: string,
  imageId: string,
  updates: { 
    status?: 'analyzing' | 'completed' | 'error'; 
    error?: string; 
    result?: any 
  }
): void {
  context.flowChatStore.updateImageAnalysisItem(sessionId, dialogTurnId, imageId, updates);
}
