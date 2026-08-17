// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());
const tauriInvokeMock = vi.hoisted(() => vi.fn());

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: {
    invoke: invokeMock,
    listen: vi.fn(() => () => {}),
  },
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: tauriInvokeMock,
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

import { EditorManager } from './EditorManager';
import { resetDeviceSurfaceForTest } from '@/infrastructure/peer-device/deviceSurface';

function fileContent(name: string, content: string) {
  return { name, content, path: `/repo/${name}`, language: 'typescript' } as never;
}

describe('EditorManager device surface teardown', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    tauriInvokeMock.mockReset();
    resetDeviceSurfaceForTest();
  });

  afterEach(() => {
    resetDeviceSurfaceForTest();
  });

  it('sends nothing to the device being left when a switch destroys editor state', async () => {
    const editor = new EditorManager({ autoSave: 'off' });
    const index = await editor.openFile(fileContent('main.ts', 'const a = 1;'));
    editor.updateFileContent(index, 'const a = 2;');
    expect(editor.isFileDirty(index)).toBe(true);
    invokeMock.mockClear();
    tauriInvokeMock.mockClear();

    editor.destroy();

    expect(invokeMock).not.toHaveBeenCalled();
    expect(tauriInvokeMock).not.toHaveBeenCalled();
    expect(editor.getAllFiles()).toEqual([]);
  });

  it('abandons a close whose file list was torn down mid-prompt', async () => {
    const editor = new EditorManager({ autoSave: 'off' });
    const index = await editor.openFile(fileContent('main.ts', 'const a = 1;'));
    editor.updateFileContent(index, 'const a = 2;');

    const closing = editor.closeFile(index);
    editor.destroy();
    await editor.openFile(fileContent('peer.ts', 'export {};'));
    await closing;

    expect(editor.getAllFiles().map(file => file.name)).toEqual(['peer.ts']);
  });
});
