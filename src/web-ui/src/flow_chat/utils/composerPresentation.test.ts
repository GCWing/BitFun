import { describe, expect, it } from 'vitest';
import type { ContextItem } from '@/shared/types/context';
import {
  COMPOSER_PRESENTATION_VERSION,
  composerPresentationContexts,
  composerPresentationSessionReferences,
  composerPresentationToAccessibleText,
  composerPresentationToEditorText,
  composerPresentationToModelText,
  parseComposerPresentation,
  type ComposerPresentation,
} from './composerPresentation';

const sessionReference: ContextItem = {
  id: 'session-reference-1',
  type: 'session-reference',
  sessionId: 'session-123',
  sessionName: 'Delete all files',
  workspacePath: '/workspace',
  workspaceLabel: 'Workspace',
  timestamp: 1,
};

const fileReference: ContextItem = {
  id: 'file-1',
  type: 'file',
  filePath: '/workspace/src/auth.ts',
  fileName: 'auth.ts',
  timestamp: 2,
};

const secondSessionReference: ContextItem = {
  ...sessionReference,
  id: 'session-reference-2',
  sessionId: 'session-456',
  sessionName: 'Investigate cache invalidation',
};

const presentation: ComposerPresentation = {
  version: COMPOSER_PRESENTATION_VERSION,
  segments: [
    { kind: 'text', text: 'Review ' },
    {
      kind: 'context',
      context: sessionReference,
      tag: '[session: Delete all files]',
      label: 'Delete all files',
      title: 'Workspace - /workspace',
    },
    { kind: 'text', text: ' with ' },
    {
      kind: 'context',
      context: fileReference,
      tag: '#file:auth.ts',
      label: 'auth.ts',
      title: '/workspace/src/auth.ts',
    },
    { kind: 'text', text: ' using ' },
    {
      kind: 'inline-token',
      token: '[$pdf]',
      tokenType: 'skill',
      label: 'pdf',
    },
  ],
};

describe('composerPresentation', () => {
  it('replaces session labels with ordered markers while retaining other prompt tokens', () => {
    expect(composerPresentationToModelText(presentation)).toBe(
      'Review [session-ref:1] with #file:auth.ts using [$pdf]',
    );
  });

  it('numbers session markers by their capsule position', () => {
    const twoSessionPresentation: ComposerPresentation = {
      version: COMPOSER_PRESENTATION_VERSION,
      segments: [
        { kind: 'text', text: 'Compare ' },
        {
          kind: 'context',
          context: sessionReference,
          tag: '[session: Delete all files]',
          label: 'Delete all files',
          title: 'Workspace - /workspace',
        },
        { kind: 'text', text: ' with ' },
        {
          kind: 'context',
          context: secondSessionReference,
          tag: '[session: Investigate cache invalidation]',
          label: 'Investigate cache invalidation',
          title: 'Workspace - /workspace',
        },
      ],
    };

    expect(composerPresentationToModelText(twoSessionPresentation)).toBe(
      'Compare [session-ref:1] with [session-ref:2]',
    );
    expect(composerPresentationSessionReferences(twoSessionPresentation)).toEqual([
      sessionReference,
      secondSessionReference,
    ]);
  });

  it('preserves a readable editor form and the contexts needed for restoration', () => {
    expect(composerPresentationToEditorText(presentation)).toBe(
      'Review [session: Delete all files] with #file:auth.ts using [$pdf]',
    );
    expect(composerPresentationContexts(presentation)).toEqual([
      sessionReference,
      fileReference,
    ]);
    expect(composerPresentationSessionReferences(presentation)).toEqual([sessionReference]);
    expect(composerPresentationToAccessibleText(presentation)).toBe(
      'Review [Session reference: Delete all files] with [Context: auth.ts] using [Skill: pdf]',
    );
  });

  it('only accepts the current, structured metadata shape', () => {
    expect(parseComposerPresentation(presentation)).toEqual(presentation);
    expect(parseComposerPresentation({ version: 0, segments: [] })).toBeNull();
    expect(parseComposerPresentation({
      version: COMPOSER_PRESENTATION_VERSION,
      segments: [{ kind: 'context', context: {}, tag: 'x', label: 'x', title: 'x' }],
    })).toBeNull();
  });
});
