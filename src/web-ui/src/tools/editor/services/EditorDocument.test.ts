import { beforeEach, describe, expect, it, vi } from 'vitest';
import { activateSurface } from '@/infrastructure/peer-device/deviceSurface';

const files = vi.hoisted(() => ({ readFileContent: vi.fn(), getFileMetadata: vi.fn(), writeFileContent: vi.fn() }));
vi.mock('@/infrastructure/api/service-api/WorkspaceAPI', () => ({ workspaceAPI: files }));
vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({ api: { invoke: vi.fn() } }));
import { EditorDocument } from './EditorDocument';

describe('editor document origin and lifetime', () => {
  beforeEach(() => { activateSurface('local'); vi.clearAllMocks(); });
  it('keeps unsaved content independent of its view and preserves the saved baseline', () => {
    const document = new EditorDocument('file-a', { surfaceId: 'local' });
    document.capture('saved', false);
    document.capture('draft', true);
    expect(document.snapshot).toEqual({ content: 'draft', isDirty: true, savedContent: 'saved' });
  });
  it('uses the captured workspace and connection for reads, metadata and writes', async () => {
    const document = new EditorDocument('file-a', { surfaceId: 'local', workspacePath: '/origin', remoteConnectionId: 'ssh-a' });
    await document.files.readFileContent('/origin/a.ts');
    await document.files.getFileMetadata('/origin/a.ts');
    await document.files.writeFileContent('/unrelated-active-workspace', '/origin/a.ts', 'content');
    expect(files.readFileContent).toHaveBeenCalledWith('/origin/a.ts', undefined, 'ssh-a');
    expect(files.getFileMetadata).toHaveBeenCalledWith('/origin/a.ts', 'ssh-a');
    expect(files.writeFileContent).toHaveBeenCalledWith('/origin', '/origin/a.ts', 'content', 'ssh-a');
  });
  it('rejects a stale read after the device changed and never writes through the new transport', async () => {
    let resolve!: (value: string) => void;
    files.readFileContent.mockImplementationOnce(() => new Promise<string>(done => { resolve = done; }));
    const document = new EditorDocument('file-a', { surfaceId: 'local' });
    const read = document.files.readFileContent('/a.ts');
    activateSurface('peer');
    resolve('old host response');
    await expect(read).rejects.toMatchObject({ isSurfaceChangedError: true });
    await expect(document.files.writeFileContent('', '/a.ts', 'draft')).rejects.toThrow('inactive device');
    expect(files.writeFileContent).not.toHaveBeenCalled();
  });
});
