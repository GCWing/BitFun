/**
 * @vitest-environment jsdom
 */

import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ComposerVoiceInputButton } from './ComposerVoiceInputButton';
import type { ComposerVoiceInputController } from './useComposerVoiceInput';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

function createRecordingController(): ComposerVoiceInputController {
  return {
    enabled: true,
    disabled: false,
    phase: 'recording',
    completionMode: null,
    audioLevel: 0,
    lowVolumeWarning: false,
    downloadProgress: null,
    setupMessage: 'Set up voice input',
    setupActionLabel: 'Install',
    setupCancelTooltip: 'Cancel setup',
    lowVolumeTooltip: 'Low volume',
    tooltip: 'Recording',
    cancelTooltip: 'Cancel recording',
    transcribeTooltip: 'Transcribe',
    sendTooltip: 'Transcribe and send',
    toggle: vi.fn(),
    installAndStart: vi.fn(),
    dismissSetup: vi.fn(),
    cancel: vi.fn(),
    transcribe: vi.fn(),
    transcribeAndSend: vi.fn(),
  };
}

describe('ComposerVoiceInputButton', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.append(container);
    root = createRoot(container);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
  });

  it('uses the composer action hierarchy for recording controls', async () => {
    await act(async () => {
      root.render(<ComposerVoiceInputButton controller={createRecordingController()} />);
    });

    const cancel = container.querySelector<HTMLButtonElement>('[data-openbitfun-action="cancel"] button');
    const transcribe = container.querySelector<HTMLButtonElement>('[data-openbitfun-action="transcribe"] button');
    const send = container.querySelector<HTMLButtonElement>('[data-openbitfun-action="send"] button');
    const actionShells = container.querySelectorAll<HTMLElement>('[data-openbitfun-part="action"]');

    expect(actionShells).toHaveLength(3);
    actionShells.forEach((actionShell) => {
      expect(actionShell.classList.contains('openbitfun-chat-input__voice-pill-action-shell')).toBe(true);
    });

    expect(cancel).toMatchObject({
      dataset: expect.objectContaining({
        openbitfunRole: 'composer-action',
        openbitfunShape: 'circle',
        openbitfunVariant: 'quiet',
      }),
    });
    expect(transcribe).toMatchObject({
      dataset: expect.objectContaining({
        openbitfunRole: 'composer-action',
        openbitfunShape: 'circle',
        openbitfunVariant: 'fill',
      }),
    });
    expect(send).toMatchObject({
      dataset: expect.objectContaining({
        openbitfunRole: 'composer-action',
        openbitfunShape: 'circle',
        openbitfunTone: 'danger',
        openbitfunVariant: 'primary',
      }),
    });
  });
});
