import { describe, expect, it, vi } from 'vitest';
import {
  submitThroughChatInputRegistration,
  type ChatInputSubmission,
} from './chatInputRegistration';

const submission: ChatInputSubmission = {
  text: 'Build a launch deck',
  displayText: 'Build a launch deck',
  contexts: [],
  composerPresentation: null,
  sessionId: 'miniapp-session',
  workspacePath: '/miniapp/deck',
};

describe('shared ChatInput registration transport', () => {
  it('routes through a registered host without invoking the session transport', async () => {
    const registeredSubmit = vi.fn().mockResolvedValue(undefined);
    const sessionSubmit = vi.fn().mockResolvedValue(undefined);

    await expect(submitThroughChatInputRegistration(
      { onSubmit: registeredSubmit },
      submission,
      sessionSubmit,
    )).resolves.toBe('registered');

    expect(registeredSubmit).toHaveBeenCalledWith(submission);
    expect(sessionSubmit).not.toHaveBeenCalled();
  });

  it('uses the normal session transport when no override is registered', async () => {
    const sessionSubmit = vi.fn().mockResolvedValue(undefined);

    await expect(submitThroughChatInputRegistration(
      undefined,
      submission,
      sessionSubmit,
    )).resolves.toBe('session');

    expect(sessionSubmit).toHaveBeenCalledOnce();
  });

  it('propagates registered transport failures so ChatInput can restore its draft', async () => {
    const error = new Error('MiniApp is unavailable');
    const sessionSubmit = vi.fn().mockResolvedValue(undefined);

    await expect(submitThroughChatInputRegistration(
      { onSubmit: vi.fn().mockRejectedValue(error) },
      submission,
      sessionSubmit,
    )).rejects.toBe(error);

    expect(sessionSubmit).not.toHaveBeenCalled();
  });
});
