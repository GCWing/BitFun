import { describe, expect, it } from 'vitest';
import type { MiniAppMeta, MiniAppPermissions } from '@/infrastructure/api/service-api/MiniAppAPI';
import {
  projectMiniAppDetailCapabilities,
  resolveMiniAppDetailSource,
} from './miniAppDetailPresentation';

function appWithPermissions(permissions: MiniAppPermissions): Pick<MiniAppMeta, 'permissions'> {
  return { permissions };
}

describe('MiniApp detail presentation', () => {
  it('prioritizes concrete manifest capabilities and fills the remaining slots', () => {
    const capabilities = projectMiniAppDetailCapabilities(appWithPermissions({
      agent: { enabled: true },
      fs: { read: ['{appdata}'], write: ['{appdata}'] },
    }));

    expect(capabilities.map((capability) => capability.kind)).toEqual([
      'ai',
      'storage',
      'surface',
    ]);
  });

  it('keeps the capability row bounded while preserving security-relevant facts', () => {
    const capabilities = projectMiniAppDetailCapabilities(appWithPermissions({
      fs: { read: ['{workspace}'] },
      host: { deck_render: true },
      shell: { allow: ['git', 'node'] },
      net: { allow: ['example.com'] },
      node: { enabled: true },
    }));

    expect(capabilities).toEqual([
      { kind: 'workspace' },
      { kind: 'export' },
      { kind: 'shell', value: 'git, node' },
    ]);
  });

  it('gives legacy permission-free apps a complete, truthful summary', () => {
    expect(projectMiniAppDetailCapabilities(appWithPermissions({}))).toEqual([
      { kind: 'surface' },
      { kind: 'controlled' },
      { kind: 'instant' },
    ]);
  });

  it('distinguishes built-in, marketplace, and other installed sources', () => {
    expect(resolveMiniAppDetailSource('builtin-ppt-live', true)).toBe('builtin');
    expect(resolveMiniAppDetailSource('market-copy', true)).toBe('market');
    expect(resolveMiniAppDetailSource('folder-import', false)).toBe('installed');
  });
});
