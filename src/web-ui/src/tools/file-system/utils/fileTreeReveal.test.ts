import { describe, expect, it } from 'vitest';
import { findFileTreeRevealIndex, planFileTreeReveal } from './fileTreeReveal';
import type { FlatFileNode } from '../types';

describe('workspace directory reveal', () => {
  it('preserves remote case, literal backslashes and quoted names on any controller', () => {
    expect(planFileTreeReveal('/Repo', '/Repo/a\\b/"quoted"', true)).toEqual({
      pathsToExpand: ['/Repo/a\\b', '/Repo/a\\b/"quoted"'], targetPath: '/Repo/a\\b/"quoted"',
    });
    expect(planFileTreeReveal('/Repo', '/repo/src', true)).toBeNull();
  });
  it('respects Windows target separators and case-insensitive roots', () => {
    expect(planFileTreeReveal('C:\\Repo', 'c:/repo/src/lib')).toEqual({
      pathsToExpand: ['C:\\Repo\\src', 'C:\\Repo\\src\\lib'], targetPath: 'C:\\Repo\\src\\lib',
    });
  });
  it('rejects other workspace prefixes and traversal', () => {
    expect(planFileTreeReveal('/repo', '/repository/src')).toBeNull();
    expect(planFileTreeReveal('/repo', '/repo/../elsewhere')).toBeNull();
  });
  it('handles the workspace itself and a filesystem root', () => {
    expect(planFileTreeReveal('/repo/', '/repo')).toEqual({ pathsToExpand: [], targetPath: '/repo' });
    expect(planFileTreeReveal('/', '/src/lib', true)).toEqual({
      pathsToExpand: ['/src', '/src/lib'], targetPath: '/src/lib',
    });
    expect(planFileTreeReveal('C:\\', 'c:/')?.targetPath).toBe('C:\\');
  });
  it('addresses offscreen rows by index and finds directories inside a compressed row', () => {
    const nodes: FlatFileNode[] = Array.from({ length: 1000 }, (_, index) => ({
      path: `/repo/folder-${index}`, name: `folder-${index}`, parentPath: '/repo',
      isDirectory: true, depth: 0, childrenLoaded: false,
    }));
    expect(findFileTreeRevealIndex(nodes, '/repo/folder-999')).toBe(999);
    const path = '/repo/a/b';
    nodes.push({ ...nodes[0], path, isCompressed: true, originalNode: {
      path, name: 'a/b', isDirectory: true, originalNodes: [
        { path: '/repo/a', name: 'a', isDirectory: true },
        { path, name: 'b', isDirectory: true },
      ],
    } });
    expect(findFileTreeRevealIndex(nodes, '/repo/a')).toBe(1000);
  });
});
