import { createContext, useContext } from 'react';
import { getActiveSurfaceId, getActiveSurfaceScope } from '@/infrastructure/peer-device/deviceSurface';
import type { ContentResourceScope } from '@/shared/types/contentResource';
import { workspaceAPI } from '@/infrastructure/api/service-api/WorkspaceAPI';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { monacoModelManager } from './MonacoModelManager';

export interface DocumentSnapshot { content: string; isDirty: boolean; savedContent: string }

/** Document state survives view placement, resource renames and surface switches. */
export class EditorDocument {
  snapshot?: DocumentSnapshot;
  viewState?: unknown;
  markdownMode?: 'ir' | 'source';
  save?: () => Promise<unknown>;
  readonly modelKey: string;
  constructor(readonly id: string, readonly scope: ContentResourceScope, filePath?: string) {
    this.modelKey = `openbitfun-document://model/${encodeURIComponent(id)}/${encodeURIComponent(filePath?.split(/[/\\]/).pop() ?? 'document')}`;
  }
  capture(content: string, isDirty: boolean, savedContent?: string): void {
    this.snapshot = { content, isDirty,
      savedContent: savedContent ?? (isDirty ? this.snapshot?.savedContent ?? content : content) };
  }
  isCurrent(): boolean { return getActiveSurfaceId() === this.scope.surfaceId; }
  private async run<T>(operation: () => Promise<T>): Promise<T> {
    const scope = getActiveSurfaceScope();
    if (getActiveSurfaceId() !== this.scope.surfaceId) throw new Error('The document is on an inactive device.');
    scope.assertCurrent('access document');
    const value = await operation();
    scope.assertCurrent('access document');
    return value;
  }
  readonly files = {
    readFileContent: (path: string, encoding?: string) =>
      this.run(() => workspaceAPI.readFileContent(path, encoding, this.scope.remoteConnectionId)),
    getFileMetadata: (path: string) =>
      this.run(() => workspaceAPI.getFileMetadata(path, this.scope.remoteConnectionId)),
    writeFileContent: (_workspace: string, path: string, content: string) =>
      this.run(() => workspaceAPI.writeFileContent(this.scope.workspacePath ?? '', path, content, this.scope.remoteConnectionId)),
  };
  readonly invoke = <T,>(command: string, args: { request: Record<string, unknown> }): Promise<T> =>
    this.run(() => api.invoke<T>(command, { ...args,
      request: { ...args.request, remoteConnectionId: this.scope.remoteConnectionId } }));
}

const documents = new Map<string, EditorDocument>();
export function getEditorDocument(id: string, scope: ContentResourceScope, filePath?: string): EditorDocument {
  let document = documents.get(id);
  if (!document) { document = new EditorDocument(id, scope, filePath); documents.set(id, document); }
  return document;
}
export function releaseEditorDocument(id: string): void {
  const document = documents.get(id);
  if (document) monacoModelManager.releaseDocumentModel(document.modelKey);
  documents.delete(id);
}
export const EditorDocumentContext = createContext<EditorDocument | null>(null);
export const useEditorDocument = () => useContext(EditorDocumentContext);
