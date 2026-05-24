import { describe, expect, it } from 'vitest';
import { ExplorerModel } from './ExplorerModel';

describe('ExplorerModel path equivalence', () => {
  it('resolves node keys across path separators', () => {
    const model = new ExplorerModel();
    model.reset('/workspace/project');

    expect(model.resolveNodeKey('/workspace/project')).toBe('/workspace/project');
    expect(model.resolveNodeKey('\\workspace\\project')).toBe('/workspace/project');
  });

  it('removes a file using an equivalent path key', () => {
    const model = new ExplorerModel();
    model.reset('/workspace/project');
    model.upsertChildren('/workspace/project', [
      { path: '/workspace/project/readme.md', name: 'readme.md', isDirectory: false },
    ]);

    expect(model.removePath('\\workspace\\project\\readme.md')).toBe(true);
    expect(model.getSnapshot().fileTree[0]?.children ?? []).toHaveLength(0);
  });
});
