import { describe, expect, it } from 'vitest';
import {
  joinWorkspaceTargetPath,
  normalizeClipboardLocalPaths,
  resolvePasteTargetDirectory,
} from './workspaceFileTransfer';

describe('workspaceFileTransfer', () => {
  it('joins remote workspace paths with POSIX separators', () => {
    expect(joinWorkspaceTargetPath('/home/user/project/', 'file.txt', true))
      .toBe('/home/user/project/file.txt');
  });

  it('joins local workspace paths with native separators', () => {
    expect(joinWorkspaceTargetPath('/Users/dev/project', 'file.txt', false))
      .toBe('/Users/dev/project/file.txt');
    expect(joinWorkspaceTargetPath('C:\\dev\\project', 'file.txt', false))
      .toBe('C:\\dev\\project\\file.txt');
  });

  it('normalizes clipboard file URLs and deduplicates paths', () => {
    expect(normalizeClipboardLocalPaths([
      'file:///tmp/a.txt',
      ' /tmp/a.txt ',
      '',
    ])).toEqual(['/tmp/a.txt']);

    expect(normalizeClipboardLocalPaths([
      'file:///C:/Users/dev/Documents/report.pdf',
    ])).toEqual(['C:/Users/dev/Documents/report.pdf']);
  });

  it('resolves paste target from selected directory node', () => {
    const fileTree = [
      {
        path: '/tmp/project',
        isDirectory: true,
        children: [
          { path: '/tmp/project/src', isDirectory: true },
        ],
      },
    ];

    const findNode = (nodes: typeof fileTree, path: string) => {
      for (const node of nodes) {
        if (node.path === path) return node;
        if (node.children) {
          const child = node.children.find((entry) => entry.path === path);
          if (child) return child;
        }
      }
      return null;
    };

    expect(resolvePasteTargetDirectory({
      workspacePath: '/tmp/project',
      selectedFile: '/tmp/project/src',
      fileTree,
      findNode,
    })).toBe('/tmp/project/src');
  });
});
