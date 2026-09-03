import { describe, expect, it, vi } from 'vitest';
import type { FileMetadata } from '@/infrastructure/api/service-api/WorkspaceAPI';
import type { ContextItem } from '@/shared/types/context';
import {
  buildExternalFileContexts,
  normalizeExternalFilePath,
  resolveExternalFileIntakeAvailability,
} from './externalFileIntake';

const metadata = (isDir: boolean, size = 12): FileMetadata => ({
  path: '',
  modified: 0,
  size,
  isFile: !isDir,
  isDir,
});

const build = (
  paths: string[],
  existingContexts: ContextItem[] = [],
  loadMetadata = vi.fn(async (path: string) => metadata(path.endsWith('/dir'))),
) => buildExternalFileContexts({
  source: 'drop',
  paths,
  existingContexts,
  workspacePath: '/workspace',
  maxImageCount: 2,
  loadMetadata,
});

describe('external file intake availability', () => {
  it.each([
    [{ desktopRuntime: false, remoteWorkspace: false, peerDevice: false, detachedDispatch: false }, 'web'],
    [{ desktopRuntime: true, remoteWorkspace: true, peerDevice: false, detachedDispatch: false }, 'remote-workspace'],
    [{ desktopRuntime: true, remoteWorkspace: false, peerDevice: true, detachedDispatch: false }, 'peer-device'],
    [{ desktopRuntime: true, remoteWorkspace: false, peerDevice: false, detachedDispatch: true }, 'detached-dispatch'],
  ] as const)('rejects unsupported data planes', (environment, reason) => {
    expect(resolveExternalFileIntakeAvailability(environment)).toEqual({ supported: false, reason });
  });

  it('accepts Desktop local execution', () => {
    expect(resolveExternalFileIntakeAvailability({
      desktopRuntime: true,
      remoteWorkspace: false,
      peerDevice: false,
      detachedDispatch: false,
    })).toEqual({ supported: true });
  });
});

describe('buildExternalFileContexts', () => {
  it('normalizes POSIX and Windows paths for deduplication', async () => {
    expect(normalizeExternalFilePath('C:\\Users\\Me\\file.txt')).toBe('c:/users/me/file.txt');
    expect(normalizeExternalFilePath('/tmp/file.txt/')).toBe('/tmp/file.txt');

    const result = await build(['C:\\Users\\Me\\file.txt', 'c:/users/me/file.txt']);
    expect(result.contexts).toHaveLength(1);
    expect(result.duplicateCount).toBe(1);
  });

  it('classifies files, directories, images, and preserves input order', async () => {
    const loadMetadata = vi.fn(async (path: string) => {
      if (path.endsWith('folder')) return metadata(true);
      return metadata(false, path.endsWith('.png') ? 42 : 7);
    });
    const result = await buildExternalFileContexts({
      source: 'clipboard',
      paths: ['/tmp/a.txt', '/tmp/folder', '/tmp/picture.png'],
      existingContexts: [],
      maxImageCount: 5,
      loadMetadata,
    });

    expect(result.contexts.map((context) => context.type)).toEqual(['file', 'directory', 'image']);
    expect(result.contexts.map((context) => context.metadata?.externalFileSource)).toEqual([
      'clipboard', 'clipboard', 'clipboard',
    ]);
    expect(result.contexts[0]).toMatchObject({ fileName: 'a.txt', mimeType: 'text/plain' });
    expect(result.contexts[2]).toMatchObject({ imagePath: '/tmp/picture.png', fileSize: 42 });
  });

  it('deduplicates paths already present as files, directories, and local images', async () => {
    const existingContexts = [
      { id: 'f', type: 'file', filePath: '/tmp/a', fileName: 'a', timestamp: 0 },
      { id: 'd', type: 'directory', directoryPath: '/tmp/b', directoryName: 'b', recursive: true, timestamp: 0 },
      { id: 'i', type: 'image', imagePath: '/tmp/c.png', imageName: 'c.png', fileSize: 1, mimeType: 'image/png', source: 'file', isLocal: true, timestamp: 0 },
    ] satisfies ContextItem[];
    const result = await build(['/tmp/a', '/tmp/b', '/tmp/c.png', '/tmp/new'], existingContexts);
    expect(result.contexts).toHaveLength(1);
    expect(result.duplicateCount).toBe(3);
  });

  it('keeps successes when metadata fails or a path is invalid', async () => {
    const loadMetadata = vi.fn(async (path: string) => {
      if (path.endsWith('missing')) throw new Error('missing');
      if (path.endsWith('invalid')) return { ...metadata(false), isFile: false };
      return metadata(false);
    });
    const result = await build(['/tmp/ok', '/tmp/missing', '/tmp/invalid'], [], loadMetadata);
    expect(result.contexts.map((context) => context.type)).toEqual(['file']);
    expect(result.failures.map((failure) => failure.reason)).toEqual(['metadata', 'invalid-path']);
  });

  it('enforces only the image count limit', async () => {
    const result = await buildExternalFileContexts({
      source: 'drop',
      paths: ['/tmp/a.png', '/tmp/b.jpg', '/tmp/c.gif', '/tmp/a.txt'],
      existingContexts: [],
      maxImageCount: 2,
      loadMetadata: async () => metadata(false),
    });
    expect(result.contexts.map((context) => context.type)).toEqual(['image', 'image', 'file']);
    expect(result.failures).toEqual([{ path: '/tmp/c.gif', reason: 'image-limit' }]);
  });
});
