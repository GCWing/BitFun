import { describe, expect, it } from 'vitest';
import type { WorkspaceReferenceEntry } from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import { workspaceReferenceItems } from './workspaceReferenceItems';

function reference(
  stableKey: string,
  origin: WorkspaceReferenceEntry['origin'],
  path: string,
  alias?: string,
): WorkspaceReferenceEntry {
  return {
    stableKey,
    origin,
    path,
    alias,
    hidden: false,
  };
}

describe('workspaceReferenceItems', () => {
  it('preserves native-first catalog order and distinct aliases for one path', () => {
    const items = workspaceReferenceItems([
      reference('native', 'native', 'D:/shared/native'),
      reference('external-docs', 'external', 'D:/shared/external', 'docs'),
      reference('external-api', 'external', 'D:/shared/external', 'api'),
      { ...reference('hidden', 'external', 'D:/hidden', 'private'), hidden: true },
    ]);

    expect(items.map(item => item.referenceStableKey)).toEqual([
      'native',
      'external-docs',
      'external-api',
    ]);
    expect(items.map(item => item.name)).toEqual(['native', '@docs', '@api']);
  });

  it('matches aliases and descriptions without recursively searching reference roots', () => {
    const items = workspaceReferenceItems([
      {
        ...reference('external-docs', 'external', 'D:/shared/external', 'docs'),
        description: 'Architecture notes',
      },
      reference('external-api', 'external', 'D:/shared/api', 'api'),
    ], 'architecture');

    expect(items.map(item => item.referenceStableKey)).toEqual(['external-docs']);
  });
});
