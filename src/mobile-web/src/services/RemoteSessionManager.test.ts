import { describe, expect, it } from 'vitest';
import {
  RemoteSessionManager,
  type RemoteUserInputIdentity,
} from './RemoteSessionManager';
import type { RelayHttpClient } from './RelayHttpClient';

const ANSWER_QUESTION_IDENTITY_CAPABILITY = 'answer_question_identity_v1';
const QUESTION_IDENTITY: RemoteUserInputIdentity = {
  session_id: 'session-1',
  turn_id: 'turn-1',
  tool_id: 'tool-1',
  registration_sequence: 7,
};

function fakeClient(response: object = { resp: 'ok' }): {
  client: RelayHttpClient;
  commands: object[];
} {
  const commands: object[] = [];
  const client = {
    controlTargetEpoch: 0,
    getControlTargetSnapshot: () => ({
      deviceId: null,
      homeDeviceId: null,
      epoch: 0,
    }),
    isControlTargetCurrent: () => true,
    onControlTargetChange: () => () => undefined,
    sendCommand: async (command: object) => {
      commands.push(command);
      return response;
    },
  } as unknown as RelayHttpClient;
  return { client, commands };
}

describe('RemoteSessionManager answer-question compatibility', () => {
  it('blocks locally without negotiated identity capability', async () => {
    const { client, commands } = fakeClient();
    const manager = new RemoteSessionManager(client);

    await expect(manager.answerQuestion(QUESTION_IDENTITY, { 0: 'yes' }))
      .rejects.toThrow(
        'Upgrade Desktop, reconnect, and answer the question again.',
      );
    expect(commands).toHaveLength(0);
  });

  it('sends the complete registration identity when capability is negotiated', async () => {
    const { client, commands } = fakeClient();
    const manager = new RemoteSessionManager(
      client,
      [ANSWER_QUESTION_IDENTITY_CAPABILITY],
    );

    await manager.answerQuestion(QUESTION_IDENTITY, { 0: 'yes' });

    expect(commands).toHaveLength(1);
    expect(commands[0]).toMatchObject({
      cmd: 'answer_question',
      ...QUESTION_IDENTITY,
      answers: { 0: 'yes' },
    });
  });

  it('negotiates identity capability for an account-device target via workspace info', async () => {
    const { client, commands } = fakeClient({
      resp: 'workspace_info',
      has_workspace: false,
      capabilities: [ANSWER_QUESTION_IDENTITY_CAPABILITY],
    });
    const manager = new RemoteSessionManager(client);

    await manager.getWorkspaceInfo();
    await manager.answerQuestion(QUESTION_IDENTITY, { 0: 'yes' });

    expect(commands).toHaveLength(2);
    expect(commands[1]).toMatchObject({
      cmd: 'answer_question',
      ...QUESTION_IDENTITY,
    });
  });

  it('surfaces a structured host upgrade response with recovery guidance', async () => {
    const { client } = fakeClient({
      resp: 'error',
      code: 'upgrade_required',
      capability: ANSWER_QUESTION_IDENTITY_CAPABILITY,
      message: 'This Desktop version cannot safely answer this question.',
      recovery: 'Upgrade Desktop and reconnect.',
    });
    const manager = new RemoteSessionManager(
      client,
      [ANSWER_QUESTION_IDENTITY_CAPABILITY],
    );

    await expect(manager.answerQuestion(QUESTION_IDENTITY, { 0: 'yes' }))
      .rejects.toThrow(
        'This Desktop version cannot safely answer this question. Upgrade Desktop and reconnect.',
      );
  });
});
