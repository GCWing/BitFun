import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8')
    .replace(/\r\n/g, '\n');
}

describe('FileExplorer rendering contract', () => {
  it('keeps one virtual tree renderer as the visible node count changes', () => {
    const source = readSource('./FileExplorer.tsx');

    expect(source).toContain("import { VirtualFileTree } from './VirtualFileTree';");
    expect(source).toContain('<VirtualFileTree');
    expect(source).not.toContain('VIRTUAL_SCROLL_THRESHOLD');
    expect(source).not.toContain("import { FileTree } from './FileTree';");
    expect(source).not.toContain('useVirtualScroll ?');
  });

  it('reserves the virtual scroller gutter so row width stays stable', () => {
    const stylesheet = readSource('../styles/FileExplorer.scss');

    expect(stylesheet).toContain('.openbitfun-file-explorer__tree--virtual > div');
    expect(stylesheet).toContain('scrollbar-gutter: stable;');
  });
});
