import { FileInfo, ReadFileChunkResult, ReadFileResult } from '../model/RemoteModels';

/** Stable mobile-side port for files owned by the active remote workspace. */
export interface RemoteWorkspaceFileClient {
  getFileInfo(path: string, sessionId?: string): Promise<FileInfo>;
  readFileChunk(path: string, offset: number, limit: number, sessionId?: string): Promise<ReadFileChunkResult>;
  readFile(
    path: string,
    sessionId?: string,
    onProgress?: (downloaded: number, total: number) => void
  ): Promise<ReadFileResult>;
}
