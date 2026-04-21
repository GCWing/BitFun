import { toolAPI } from '@/infrastructure/api';
import { useDesignTokensStore, type DesignTokenProposal, type DesignTokensDocument } from './store/designTokensStore';
import { canonicalScopeKey } from './tokensSchema';

interface TokensResponse {
  success?: boolean;
  tokens_event?: string;
  data?: {
    tokens?: DesignTokensDocument;
    path?: string;
    items?: Array<{ path: string; tokens: DesignTokensDocument }>;
    selection_status?: string;
    committed_id?: string;
  };
  error?: string;
}

function parseTokensResult(raw: any): TokensResponse {
  if (!raw) return {};
  if (raw.success === false || raw.error || raw.validation_error || raw.validationError) {
    return {
      success: false,
      error: raw.error || raw.validation_error || raw.validationError || 'DesignTokens failed',
    };
  }
  return (raw.result || raw) as TokensResponse;
}

async function invoke(
  parameters: Record<string, unknown>,
  workspacePath?: string,
  artifactId?: string
): Promise<TokensResponse> {
  const raw = await toolAPI.executeTool({
    toolName: 'DesignTokens',
    parameters,
    workspacePath,
  } as any);
  const result = parseTokensResult(raw);
  if (result.success === false) {
    throw new Error(result.error || 'DesignTokens failed');
  }
  const payload = result.data;
  if (payload?.tokens) {
    const scopeKey = canonicalScopeKey({
      explicitPath: payload.path,
      workspacePath,
      artifactId: artifactId || (parameters.artifact_id as string | undefined),
    });
    useDesignTokensStore.getState().upsert(scopeKey, payload.tokens);
  }
  if (payload?.items) {
    for (const item of payload.items) {
      useDesignTokensStore
        .getState()
        .upsert(canonicalScopeKey({ explicitPath: item.path, workspacePath }), item.tokens);
    }
  }
  return result;
}

export const designTokensAPI = {
  propose(proposals: unknown[], artifactId?: string, workspacePath?: string) {
    return invoke({ action: 'propose', artifact_id: artifactId, proposals }, workspacePath, artifactId);
  },
  /**
   * Resume/await selection on an already-proposed document. Use when reconnecting
   * a UI to a previous `propose` call whose oneshot channel was dropped.
   */
  awaitSelection(artifactId?: string, workspacePath?: string) {
    return invoke({ action: 'await_selection', artifact_id: artifactId }, workspacePath, artifactId);
  },
  commit(proposalId: string, artifactId?: string, workspacePath?: string) {
    return invoke(
      { action: 'commit', artifact_id: artifactId, proposal_id: proposalId },
      workspacePath,
      artifactId
    );
  },
  updateProposal(proposal: DesignTokenProposal, artifactId?: string, workspacePath?: string) {
    return invoke(
      { action: 'update', artifact_id: artifactId, proposal },
      workspacePath,
      artifactId
    );
  },
  preview(artifactId?: string, workspacePath?: string) {
    return invoke({ action: 'preview', artifact_id: artifactId }, workspacePath, artifactId);
  },
  /** Alias — `get` is the preferred verb in the Rust tool for reading without inferring. */
  get(artifactId?: string, workspacePath?: string) {
    return invoke({ action: 'get', artifact_id: artifactId }, workspacePath, artifactId);
  },
  list(workspacePath?: string) {
    return invoke({ action: 'list' }, workspacePath);
  },
};
