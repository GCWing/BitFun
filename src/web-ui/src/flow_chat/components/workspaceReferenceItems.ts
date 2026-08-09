import type { WorkspaceReferenceEntry } from '@/infrastructure/api/service-api/ExternalSourcesAPI';

export interface FileItem {
  path: string;
  name: string;
  isDirectory: boolean;
  relativePath: string;
  referenceStableKey?: string;
  referenceDescription?: string;
}

export function workspaceReferenceItems(
  references: WorkspaceReferenceEntry[],
  query = '',
): FileItem[] {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  return references
    .filter(reference => !reference.hidden)
    .filter(reference => {
      if (!normalizedQuery) return true;
      return [reference.alias, reference.description, reference.path]
        .some(value => value?.toLocaleLowerCase().includes(normalizedQuery));
    })
    .map(reference => ({
      path: reference.path,
      name: reference.alias
        ? `@${reference.alias}`
        : reference.path.replace(/\\/g, '/').split('/').pop() || reference.path,
      isDirectory: true,
      relativePath: reference.path,
      referenceStableKey: reference.stableKey,
      referenceDescription: reference.description,
    }));
}
