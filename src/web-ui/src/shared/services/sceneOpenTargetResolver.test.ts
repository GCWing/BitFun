import { beforeEach, describe, expect, it } from 'vitest';
import { useSceneStore } from '@/app/stores/sceneStore';
import {
  resolveAndFocusOpenTarget,
  resolveOpenTarget,
} from './sceneOpenTargetResolver';

describe('sceneOpenTargetResolver', () => {
  beforeEach(() => {
    useSceneStore.getState().resetForPeerSwitch();
  });

  it('keeps ordinary file opens in the active session auxiliary pane', () => {
    useSceneStore.getState().openScene('session');

    expect(resolveOpenTarget('file')).toMatchObject({
      mode: 'agent',
      targetSceneId: 'session',
    });
  });

  it('routes an explicit project tree selection to the file viewer even from a session', () => {
    useSceneStore.getState().openScene('session');

    expect(resolveOpenTarget('file', { source: 'project-nav' })).toMatchObject({
      mode: 'project',
      targetSceneId: 'file-viewer',
    });
  });

  it('focuses a newly opened file viewer and marks it for queued tab delivery', () => {
    useSceneStore.getState().openScene('session');

    const resolution = resolveAndFocusOpenTarget('file', { source: 'project-nav' });

    expect(resolution).toMatchObject({
      mode: 'project',
      targetSceneId: 'file-viewer',
      sceneJustOpened: true,
    });
    expect(useSceneStore.getState().activeTabId).toBe('file-viewer');
  });
});
