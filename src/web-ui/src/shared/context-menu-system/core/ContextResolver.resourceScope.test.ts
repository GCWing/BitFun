// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { ContextResolver } from './ContextResolver';
import { ContextType } from '../types/context.types';

describe('file menu origin', () => {
  it('captures the browsed workspace and SSH connection from the resource owner', () => {
    const panel = document.createElement('div');
    Object.assign(panel.dataset, {
      resourceSurface: 'peer-b', resourceWorkspaceId: 'remote-b',
      resourceWorkspacePath: '/repo', resourceConnectionId: 'ssh-b',
    });
    const tree = document.createElement('div');
    tree.className = 'openbitfun-file-explorer';
    tree.dataset.workspaceRoot = '/repo';
    const node = document.createElement('div');
    node.dataset.filePath = '/repo/test.ts';
    node.dataset.isDirectory = 'false';
    tree.append(node);
    panel.append(tree);
    document.body.append(panel);
    try {
      const event = new MouseEvent('contextmenu', { bubbles: true });
      node.dispatchEvent(event);
      expect(new ContextResolver().resolve(event)).toMatchObject({
        type: ContextType.FILE_NODE,
        filePath: '/repo/test.ts', workspacePath: '/repo',
        resourceScope: { surfaceId: 'peer-b', workspaceId: 'remote-b', workspacePath: '/repo', remoteConnectionId: 'ssh-b' },
      });
    } finally {
      panel.remove();
    }
  });
});
