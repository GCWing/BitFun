import { beforeEach, describe, expect, it } from 'vitest';

import type { ContextItem } from '@/shared/types/context';
import { sessionComposerStore } from './sessionComposerStore';
import {
  LOCAL_SURFACE_ID,
  activateSurface,
} from '@/infrastructure/peer-device/deviceSurface';

function context(id: string): ContextItem {
  return {
    id,
    type: 'file',
    filePath: `${id}.ts`,
    fileName: `${id}.ts`,
  } as ContextItem;
}

describe('sessionComposerStore', () => {
  beforeEach(() => {
    activateSurface(LOCAL_SURFACE_ID);
    sessionComposerStore.setState({ drafts: {} });
  });

  it('keeps text, contexts, and large paste payloads isolated by session', () => {
    const store = sessionComposerStore.getState();
    store.setValue('session-a', 'draft a');
    store.setContexts('session-a', [context('context-a')]);
    store.setPendingLargePastes('session-a', { '[large paste]': 'full text a' });

    store.setValue('session-b', 'draft b');
    store.setContexts('session-b', [context('context-b')]);

    expect(sessionComposerStore.getState().getDraft('session-a')).toMatchObject({
      value: 'draft a',
      contexts: [{ id: 'context-a' }],
      pendingLargePastes: { '[large paste]': 'full text a' },
    });
    expect(sessionComposerStore.getState().getDraft('session-b')).toMatchObject({
      value: 'draft b',
      contexts: [{ id: 'context-b' }],
      pendingLargePastes: {},
    });
  });

  it('clears only the target session draft', () => {
    const store = sessionComposerStore.getState();
    store.setValue('session-a', 'draft a');
    store.setValue('session-b', 'draft b');

    store.clearDraft('session-a');

    expect(sessionComposerStore.getState().getDraft('session-a').value).toBe('');
    expect(sessionComposerStore.getState().getDraft('session-b').value).toBe('draft b');
  });

  it('saves the previous contexts and restores the complete next draft on activation', () => {
    const store = sessionComposerStore.getState();
    store.setValue('session-a', 'draft a');
    store.setValue('session-b', 'draft b');
    store.setContexts('session-b', [context('context-b')]);
    store.setPendingLargePastes('session-b', { '[large paste]': 'full text b' });

    const nextDraft = store.activateDraft(
      'session-a',
      'session-b',
      [context('context-a')],
    );

    expect(sessionComposerStore.getState().getDraft('session-a').contexts).toMatchObject([
      { id: 'context-a' },
    ]);
    expect(nextDraft).toMatchObject({
      value: 'draft b',
      contexts: [{ id: 'context-b' }],
      pendingLargePastes: { '[large paste]': 'full text b' },
    });
  });

  it('removes drafts for deleted session ids without disturbing others', () => {
    const store = sessionComposerStore.getState();
    store.setValue('session-a', 'draft a');
    store.setValue('session-b', 'draft b');

    store.removeDrafts(['session-a']);

    expect(sessionComposerStore.getState().getDraft('session-a').value).toBe('');
    expect(sessionComposerStore.getState().getDraft('session-b').value).toBe('draft b');
  });

  it('keeps equal session ids isolated across device surfaces', () => {
    sessionComposerStore.getState().setValue('same-session', 'local draft');

    activateSurface('peer-b');
    expect(sessionComposerStore.getState().getDraft('same-session').value).toBe('');
    sessionComposerStore.getState().setValue('same-session', 'peer draft');

    activateSurface(LOCAL_SURFACE_ID);
    expect(sessionComposerStore.getState().getDraft('same-session').value).toBe('local draft');
    activateSurface('peer-b');
    expect(sessionComposerStore.getState().getDraft('same-session').value).toBe('peer draft');
  });

  it('does not save the previous surface contexts into the new surface', () => {
    activateSurface('peer-b');

    sessionComposerStore.getState().activateDraft(
      'local-session',
      'peer-session',
      [context('local-context')],
      false,
    );

    expect(sessionComposerStore.getState().getDraft('local-session').contexts).toEqual([]);
  });
});
