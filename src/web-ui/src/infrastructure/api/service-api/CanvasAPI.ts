import { api } from './ApiClient';

export interface CanvasStateValue {
  canvasId: string;
  sourceRevisionSeen?: string;
  values: Record<string, unknown>;
  updatedAt: number;
  schemaVersion: number;
}

export interface CanvasStateRequest {
  artifactReference: string;
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

export interface SaveCanvasStateRequest extends CanvasStateRequest {
  sourceRevisionSeen?: string;
  values: Record<string, unknown>;
  updatedAt: number;
}

export interface ReportCanvasRuntimeErrorRequest extends CanvasStateRequest {
  sourceRevisionSeen?: string;
  message: string;
  name?: string;
  stack?: string;
}

export interface CanvasStateResponse {
  state?: CanvasStateValue | null;
}

export interface CanvasDiagnosticValue {
  severity?: string;
  category?: string;
  message?: string;
  code?: string;
  line?: number;
  column?: number;
  suggestedFix?: string;
}

export interface CanvasSnapshotValue {
  artifact?: {
    title?: string;
    status?: string;
    sourceRevision?: string;
    latestCompiledRevision?: string;
    lastKnownGoodRevision?: string;
  };
  source?: {
    source?: string;
    filename?: string;
    revision?: string;
  };
  diagnostics?: CanvasDiagnosticValue[];
  compiledPayload?: {
    html?: string;
    sourceRevision?: string;
    contentHash?: string;
  } | null;
  state?: CanvasStateValue | null;
}

export interface CanvasArtifactResponse {
  canvas: CanvasSnapshotValue;
  artifactReference: string;
}

class CanvasAPI {
  async loadArtifact(request: CanvasStateRequest): Promise<CanvasArtifactResponse> {
    return api.invoke('load_canvas_artifact', { request });
  }

  async loadState(request: CanvasStateRequest): Promise<CanvasStateResponse> {
    return api.invoke('load_canvas_state', { request });
  }

  async saveState(request: SaveCanvasStateRequest): Promise<CanvasStateResponse> {
    return api.invoke('save_canvas_state', { request });
  }

  async reportRuntimeError(request: ReportCanvasRuntimeErrorRequest): Promise<CanvasArtifactResponse> {
    return api.invoke('report_canvas_runtime_error', { request });
  }
}

export const canvasAPI = new CanvasAPI();
