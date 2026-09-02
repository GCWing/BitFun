import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ToolAPI } from './ToolAPI';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({
  api: { invoke: invokeMock },
}));

describe('ToolAPI user-question responses', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it('carries the owning Session across a Peer HostInvoke boundary', async () => {
    const toolAPI = new ToolAPI();

    await toolAPI.submitUserAnswers(
      'ask-tool-1',
      { 0: 'Focused' },
      'session-1',
    );

    expect(invokeMock).toHaveBeenCalledWith('submit_user_answers', {
      toolId: 'ask-tool-1',
      answers: { 0: 'Focused' },
      sessionId: 'session-1',
    });
  });

  it('keeps the legacy Desktop call shape when no Session is available', async () => {
    const toolAPI = new ToolAPI();

    await toolAPI.submitUserAnswers('ask-tool-1', { 0: 'Focused' });

    expect(invokeMock).toHaveBeenCalledWith('submit_user_answers', {
      toolId: 'ask-tool-1',
      answers: { 0: 'Focused' },
    });
  });
});
