import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export type LearningProposalSourceKind =
  | 'user_message'
  | 'assistant_text'
  | 'assistant_thinking'
  | 'tool'
  | 'unknown';

export interface LearningProposalSelection {
  selectedText: string;
  turnId: string;
  roundId?: string;
  itemId?: string;
  sourceKind: LearningProposalSourceKind;
}

export interface LearningProposalSource extends LearningProposalSelection {
  sessionId: string;
  workspacePath: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

export type LearningProposalStatus =
  | 'analyzing'
  | 'ready'
  | 'applying'
  | 'applied'
  | 'rejected'
  | 'stale'
  | 'failed';

export type LearningProposalTargetKind = 'memory' | 'skill' | 'agents_md' | 'none';
export type LearningProposalApplyMode = 'memory_note' | 'read_only';

export interface LearningProposalTarget {
  kind: LearningProposalTargetKind;
  applyMode: LearningProposalApplyMode;
  displayName: string;
  identifier?: string;
  filePath?: string;
}

export interface LearningProposalPreview {
  filePath?: string;
  originalContent: string;
  proposedContent: string;
}

export interface LearningProposalError {
  code: string;
  message: string;
}

export interface LearningProposal {
  schemaVersion: number;
  proposalId: string;
  status: LearningProposalStatus;
  source: LearningProposalSource;
  target?: LearningProposalTarget;
  rationale?: string;
  futureUse?: string;
  preview?: LearningProposalPreview;
  baseHash?: string;
  diffHash?: string;
  createdAt: number;
  updatedAt: number;
  error?: LearningProposalError;
}

export interface LearningProposalWorkspaceContext {
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

export interface CreateLearningProposalRequest extends LearningProposalWorkspaceContext {
  sessionId: string;
  workspacePath: string;
  source: LearningProposalSelection;
}

export interface GetLearningProposalRequest extends LearningProposalWorkspaceContext {
  proposalId: string;
}

export interface ApproveLearningProposalRequest extends GetLearningProposalRequest {
  baseHash: string;
  diffHash: string;
}

export interface ListLearningProposalsRequest {
  includeResolved?: boolean;
}

export class LearningProposalAPI {
  async list(request: ListLearningProposalsRequest = {}): Promise<LearningProposal[]> {
    try {
      return await api.invoke<LearningProposal[]>('list_learning_proposals', { request });
    } catch (error) {
      throw createTauriCommandError('list_learning_proposals', error, request);
    }
  }

  async create(request: CreateLearningProposalRequest): Promise<LearningProposal> {
    return this.invoke('create_learning_proposal', request);
  }

  async get(request: GetLearningProposalRequest): Promise<LearningProposal> {
    return this.invoke('get_learning_proposal', request);
  }

  async refresh(request: GetLearningProposalRequest): Promise<LearningProposal> {
    return this.invoke('refresh_learning_proposal', request);
  }

  async approve(request: ApproveLearningProposalRequest): Promise<LearningProposal> {
    return this.invoke('approve_learning_proposal', request);
  }

  async reject(request: GetLearningProposalRequest): Promise<LearningProposal> {
    return this.invoke('reject_learning_proposal', request);
  }

  private async invoke<TRequest extends object>(
    command: string,
    request: TRequest,
  ): Promise<LearningProposal> {
    try {
      return await api.invoke<LearningProposal>(command, { request });
    } catch (error) {
      throw createTauriCommandError(command, error, request);
    }
  }
}

export const learningProposalAPI = new LearningProposalAPI();
