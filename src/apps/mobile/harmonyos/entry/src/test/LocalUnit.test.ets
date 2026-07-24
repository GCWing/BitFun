import { describe, it, expect } from '@ohos/hypium';
import { ChatSessionController, ChatSessionSnapshot } from '../main/ets/services/ChatSessionController';
import { ChatTimelineProjector } from '../main/ets/services/ChatTimelineProjector';
import { ChatTimelineStore } from '../main/ets/services/ChatTimelineStore';
import { Encoding } from '../main/ets/services/Encoding';
import { MarkdownParser } from '../main/ets/services/MarkdownParser';
import { RemoteCommandFactory } from '../main/ets/services/RemoteCommandFactory';
import { RemoteCrypto } from '../main/ets/services/RemoteCrypto';
import { RemoteDescriptorParser } from '../main/ets/services/RemoteDescriptorParser';
import { RemoteResponseMapper } from '../main/ets/services/RemoteResponseMapper';
import { RemoteSessionManager } from '../main/ets/services/RemoteSessionManager';
import { RemoteUiState } from '../main/ets/services/RemoteUiState';
import { TimeFormat } from '../main/ets/services/TimeFormat';
import { X25519 } from '../main/ets/services/X25519';
import { ChatMessage, ImageAttachment, PollSessionResult, RemoteQuestionAnswerPayload } from '../main/ets/model/RemoteModels';

interface CryptoImageFixture {
  id: string;
  data_url: string;
  mime_type: string;
}

interface CryptoCommandFixture {
  cmd: string;
  session_id: string;
  content: string;
  agent_type: string;
  image_contexts: CryptoImageFixture[];
}

interface CryptoSimpleCommandFixture {
  cmd: string;
}

function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let index = 0; index < bytes.length; index++) {
    bytes[index] = Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16);
  }
  return bytes;
}

function bytesToHex(bytes: Uint8Array): string {
  const alphabet = '0123456789abcdef';
  let text = '';
  for (let index = 0; index < bytes.length; index++) {
    const value = bytes[index];
    text += alphabet.charAt((value >> 4) & 15) + alphabet.charAt(value & 15);
  }
  return text;
}

function expectCommandJson(command: Object, expected: string): void {
  expect(JSON.stringify(command)).assertEqual(expected);
}

function connectedPeers(): RemoteCrypto[] {
  const alice = new RemoteCrypto();
  const bob = new RemoteCrypto();
  alice.deriveSharedKey(bob.getPublicKeyBase64());
  bob.deriveSharedKey(alice.getPublicKeyBase64());
  return [alice, bob];
}

function chatMessage(id: string, role: string, text: string, status: string = 'completed'): ChatMessage {
  return {
    id,
    role,
    text,
    status,
    detail: ''
  };
}

function activeChatMessage(turnId: string, text: string, status: string = 'active', renderVersion: number = 0): ChatMessage {
  const message = chatMessage(`active-${turnId}`, 'assistant', text, status);
  message.turnId = turnId;
  message.renderVersion = renderVersion;
  return message;
}

function chatMessageWithImage(id: string, role: string, text: string, imageName: string, imageDataUrl: string): ChatMessage {
  return {
    id,
    role,
    text,
    status: 'completed',
    detail: '',
    images: [imageAttachment(imageName, imageDataUrl)]
  };
}

function imageAttachment(name: string, dataUrl: string): ImageAttachment {
  return {
    name,
    data_url: dataUrl
  };
}

function pollResult(options: PollSessionResult): PollSessionResult {
  return options;
}

function delay(ms: number): Promise<void> {
  return new Promise<void>((resolve: () => void) => {
    setTimeout(() => {
      resolve();
    }, ms);
  });
}

class FakePollSessionManager extends RemoteSessionManager {
  results: PollSessionResult[] = [];

  async pollSession(
    _sessionId: string,
    _sinceVersion: number,
    _knownMessageCount: number,
    _knownModelCatalogVersion: number
  ): Promise<PollSessionResult> {
    const next = this.results.shift();
    if (!next) {
      throw new Error('Missing fake poll result.');
    }
    return next;
  }
}

export default function localUnitTest() {
  describe('RemoteDescriptorParser', () => {
    it('parses hash route URLs', 0, () => {
      const descriptor = RemoteDescriptorParser.parse('https://relay.example.com/r/mobile#/pair?room=room-a&pk=public-key');
      expect(descriptor.roomId).assertEqual('room-a');
      expect(descriptor.publicKey).assertEqual('public-key');
      expect(descriptor.relayUrl).assertEqual('https://relay.example.com');
    });

    it('normalizes relay websocket URLs', 0, () => {
      const descriptor = RemoteDescriptorParser.parse('https://app.example.com/#/pair?room=room-b&pk=key-b&relay=wss%3A%2F%2Frelay.example.com%2Fws');
      expect(descriptor.relayUrl).assertEqual('https://relay.example.com');
    });

    it('parses raw query strings', 0, () => {
      const descriptor = RemoteDescriptorParser.parse('room=room-c&pk=key%2Bc%3D&relay=http%3A%2F%2F127.0.0.1%3A30333');
      expect(descriptor.roomId).assertEqual('room-c');
      expect(descriptor.publicKey).assertEqual('key+c=');
      expect(descriptor.relayUrl).assertEqual('http://127.0.0.1:30333');
    });

    it('rejects URLs missing room', 0, () => {
      let didThrow = false;
      try {
        RemoteDescriptorParser.parse('https://relay.example.com/#/pair?pk=key-only');
      } catch (_err) {
        didThrow = true;
      }
      expect(didThrow).assertEqual(true);
    });

    it('rejects URLs missing pk', 0, () => {
      let didThrow = false;
      try {
        RemoteDescriptorParser.parse('https://relay.example.com/#/pair?room=room-only');
      } catch (_err) {
        didThrow = true;
      }
      expect(didThrow).assertEqual(true);
    });
  });

  describe('X25519', () => {
    const alicePrivate = '77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a';
    const alicePublic = '8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a';
    const bobPrivate = '5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb';
    const bobPublic = 'de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f';
    const sharedSecret = '4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742';
    const mobileWebPrivate = '000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f';
    const mobileWebPublic = '8f40c5adb68f25624ae5b214ea767a6ec94d829d3d7b5e1ad1ba6f3e2138285f';
    const desktopPrivate = 'f0e1d2c3b4a5968778695a4b3c2d1e0f00112233445566778899aabbccddeeff';
    const desktopPublic = 'ed99ba8df1a6a8c6396beafc3a42b8212fde389a7663217e170e82b68377e858';
    const mobileWebSharedSecret = '251b4e394babbef8dc30269e5a065946c08d89d6df010376549baa469c552f4b';

    it('matches RFC 7748 public key vectors', 0, () => {
      expect(bytesToHex(X25519.scalarMultBase(hexToBytes(alicePrivate)))).assertEqual(alicePublic);
      expect(bytesToHex(X25519.scalarMultBase(hexToBytes(bobPrivate)))).assertEqual(bobPublic);
    });

    it('matches RFC 7748 shared secret vector from both peers', 0, () => {
      expect(bytesToHex(X25519.scalarMult(hexToBytes(alicePrivate), hexToBytes(bobPublic)))).assertEqual(sharedSecret);
      expect(bytesToHex(X25519.scalarMult(hexToBytes(bobPrivate), hexToBytes(alicePublic)))).assertEqual(sharedSecret);
    });

    it('matches the mobile-web noble-curves shared secret shape', 0, () => {
      expect(bytesToHex(X25519.scalarMultBase(hexToBytes(mobileWebPrivate)))).assertEqual(mobileWebPublic);
      expect(bytesToHex(X25519.scalarMultBase(hexToBytes(desktopPrivate)))).assertEqual(desktopPublic);
      expect(bytesToHex(X25519.scalarMult(hexToBytes(mobileWebPrivate), hexToBytes(desktopPublic))))
        .assertEqual(mobileWebSharedSecret);
      expect(bytesToHex(X25519.scalarMult(hexToBytes(desktopPrivate), hexToBytes(mobileWebPublic))))
        .assertEqual(mobileWebSharedSecret);
    });
  });

  describe('RemoteCrypto', () => {
    it('encrypts and decrypts JSON between derived peers', 0, async () => {
      const peers = connectedPeers();
      const alice = peers[0];
      const bob = peers[1];
      const command: CryptoCommandFixture = {
        cmd: 'send_message',
        session_id: 'session-1',
        content: 'hello from mobile',
        agent_type: 'code',
        image_contexts: [{
          id: 'image-1',
          data_url: 'data:image/png;base64,aGVsbG8=',
          mime_type: 'image/png'
        }]
      };

      const encrypted = await alice.encryptJson(command);
      const decrypted = await bob.decryptJson<CryptoCommandFixture>(encrypted);

      expect(decrypted.cmd).assertEqual(command.cmd);
      expect(decrypted.session_id).assertEqual(command.session_id);
      expect(decrypted.content).assertEqual(command.content);
      expect(decrypted.agent_type).assertEqual(command.agent_type);
      expect(decrypted.image_contexts.length).assertEqual(1);
      expect(decrypted.image_contexts[0].id).assertEqual('image-1');
      expect(decrypted.image_contexts[0].mime_type).assertEqual('image/png');
    });

    it('produces relay-compatible encrypted payload shape', 0, async () => {
      const peers = connectedPeers();
      const command: CryptoSimpleCommandFixture = {
        cmd: 'get_workspace_info'
      };
      const encrypted = await peers[0].encryptJson(command);
      const keys = Object.keys(encrypted).sort().join(',');
      const nonce = Encoding.base64ToBytes(encrypted.nonce);
      const encryptedData = Encoding.base64ToBytes(encrypted.encrypted_data);

      expect(keys).assertEqual('encrypted_data,nonce');
      expect(nonce.length).assertEqual(12);
      expect(encryptedData.length).assertLarger(16);
      expect(encrypted.encrypted_data.indexOf('get_workspace_info')).assertEqual(-1);
      const decrypted = await peers[1].decryptJson<CryptoCommandFixture>(encrypted);
      expect(decrypted.cmd).assertEqual('get_workspace_info');
    });

    it('rejects use before shared-key derivation', 0, async () => {
      let didThrow = false;
      try {
        const command: CryptoSimpleCommandFixture = {
          cmd: 'ping'
        };
        await new RemoteCrypto().encryptJson(command);
      } catch (_err) {
        didThrow = true;
      }
      expect(didThrow).assertEqual(true);
    });
  });

  describe('TimeFormat', () => {
    const now = Date.parse('2026-06-10T12:00:00.000Z');

    it('formats recent timestamps as just now', 0, () => {
      expect(TimeFormat.relative('2026-06-10T11:59:45.000Z', now)).assertEqual('刚刚');
    });

    it('formats minute and hour relative timestamps', 0, () => {
      expect(TimeFormat.relative('2026-06-10T11:42:00.000Z', now)).assertEqual('18 分钟前');
      expect(TimeFormat.relative('2026-06-10T09:00:00.000Z', now)).assertEqual('3 小时前');
    });

    it('formats older timestamps as days or dates', 0, () => {
      expect(TimeFormat.relative('2026-06-08T12:00:00.000Z', now)).assertEqual('2 天前');
      expect(TimeFormat.relative('2026-05-30T12:00:00.000Z', now)).assertEqual('2026-05-30');
    });

    it('does not show invalid timestamps as just now', 0, () => {
      expect(TimeFormat.relative('', now)).assertEqual('时间未知');
      expect(TimeFormat.relative('not-a-date', now)).assertEqual('时间未知');
    });

    it('formats numeric timestamps and future timestamps safely', 0, () => {
      expect(TimeFormat.relative('1781091720', now)).assertEqual('18 分钟前');
      expect(TimeFormat.relative('2026-06-12T12:00:00.000Z', now)).assertEqual('2026-06-12');
    });
  });

  describe('RemoteCommandFactory', () => {
    it('builds workspace and assistant commands', 0, () => {
      expectCommandJson(RemoteCommandFactory.getWorkspaceInfo(), '{"cmd":"get_workspace_info"}');
      expectCommandJson(RemoteCommandFactory.listRecentWorkspaces(), '{"cmd":"list_recent_workspaces"}');
      expectCommandJson(RemoteCommandFactory.setWorkspace('/repo'), '{"cmd":"set_workspace","path":"/repo"}');
      expectCommandJson(RemoteCommandFactory.listAssistants(), '{"cmd":"list_assistants"}');
      expectCommandJson(RemoteCommandFactory.setAssistant('/assistant'), '{"cmd":"set_assistant","path":"/assistant"}');
    });

    it('builds session list and creation commands without unsupported fields', 0, () => {
      const listCommand = RemoteCommandFactory.listSessions('/repo', 8, 16, '  auth  ');
      expectCommandJson(listCommand, '{"cmd":"list_sessions","workspace_path":"/repo","limit":8,"offset":16,"query":"auth"}');
      expect(JSON.stringify(listCommand).indexOf('agent_type')).assertEqual(-1);

      const createCommand = RemoteCommandFactory.createSession({
        agentType: 'cowork',
        title: '',
        instruction: 'hello'
      }, '/repo');
      expectCommandJson(createCommand, '{"cmd":"create_session","agent_type":"cowork","session_name":"Remote Cowork Session","workspace_path":"/repo"}');
      expect(JSON.stringify(createCommand).indexOf('run_mode')).assertEqual(-1);
    });

    it('builds message and polling commands', 0, () => {
      expectCommandJson(RemoteCommandFactory.sendMessage('s1', 'hello', 'code'), '{"cmd":"send_message","session_id":"s1","content":"hello","agent_type":"code"}');
      expectCommandJson(RemoteCommandFactory.sendMessage('s1', 'look', 'code', [{
        id: 'img1',
        data_url: 'data:image/png;base64,abc',
        mime_type: 'image/png'
      }]), '{"cmd":"send_message","session_id":"s1","content":"look","agent_type":"code","image_contexts":[{"id":"img1","data_url":"data:image/png;base64,abc","mime_type":"image/png"}]}');
      expectCommandJson(RemoteCommandFactory.getSessionMessages('s1'), '{"cmd":"get_session_messages","session_id":"s1"}');
      expectCommandJson(RemoteCommandFactory.pollSession('s1', 7, 3, 2), '{"cmd":"poll_session","session_id":"s1","since_version":7,"known_msg_count":3,"known_model_catalog_version":2}');
    });

    it('builds model and session lifecycle commands', 0, () => {
      expectCommandJson(RemoteCommandFactory.getModelCatalog('s1'), '{"cmd":"get_model_catalog","session_id":"s1"}');
      expectCommandJson(RemoteCommandFactory.setSessionModel('s1', 'model-a'), '{"cmd":"set_session_model","session_id":"s1","model_id":"model-a"}');
      expectCommandJson(RemoteCommandFactory.cancelTask('s1'), '{"cmd":"cancel_task","session_id":"s1"}');
      expectCommandJson(RemoteCommandFactory.cancelTask('s1', 'turn-9'), '{"cmd":"cancel_task","session_id":"s1","turn_id":"turn-9"}');
      expectCommandJson(RemoteCommandFactory.renameSession('s1', 'Next'), '{"cmd":"update_session_title","session_id":"s1","title":"Next"}');
      expectCommandJson(RemoteCommandFactory.deleteSession('s1'), '{"cmd":"delete_session","session_id":"s1"}');
      expectCommandJson(RemoteCommandFactory.ping(), '{"cmd":"ping"}');
    });

    it('builds tool and file commands', 0, () => {
      const answers: RemoteQuestionAnswerPayload = { answer: 'yes', '0': 'yes' };

      expectCommandJson(RemoteCommandFactory.confirmTool('tool-1'), '{"cmd":"confirm_tool","tool_id":"tool-1"}');
      expectCommandJson(RemoteCommandFactory.rejectTool('tool-1', 'no'), '{"cmd":"reject_tool","tool_id":"tool-1","reason":"no"}');
      expectCommandJson(RemoteCommandFactory.cancelTool('tool-1', 'stop'), '{"cmd":"cancel_tool","tool_id":"tool-1","reason":"stop"}');
      expectCommandJson(RemoteCommandFactory.answerQuestion('tool-2', answers), '{"cmd":"answer_question","tool_id":"tool-2","answers":{"0":"yes","answer":"yes"}}');
      expectCommandJson(RemoteCommandFactory.getFileInfo('/tmp/a.txt', 's1'), '{"cmd":"get_file_info","path":"/tmp/a.txt","session_id":"s1"}');
      expectCommandJson(RemoteCommandFactory.readFileChunk('/tmp/a.txt', 12, 64, 's1'), '{"cmd":"read_file_chunk","path":"/tmp/a.txt","offset":12,"limit":64,"session_id":"s1"}');
    });
  });

  describe('RemoteResponseMapper', () => {
    it('maps session response items', 0, () => {
      const sessions = RemoteResponseMapper.sessions([{
        session_id: 'session-abcdef',
        name: 'Fallback title',
        agent_type: 'agentic',
        status: 'running',
        updated_at: '2026-06-10T12:00:00.000Z',
        created_at: '2026-06-09T12:00:00.000Z',
        message_count: 4,
        workspace_path: '/repo',
        workspace_name: 'Repo'
      }]);
      expect(sessions.length).assertEqual(1);
      expect(sessions[0].id).assertEqual('session-abcdef');
      expect(sessions[0].title).assertEqual('Fallback title');
      expect(sessions[0].agentType).assertEqual('agentic');
      expect(sessions[0].messageCount).assertEqual(4);
      expect(sessions[0].workspaceName).assertEqual('Repo');
    });

    it('maps alternate session time fields', 0, () => {
      const sessions = RemoteResponseMapper.sessions([{
        session_id: 'session-time',
        title: 'Alternate time fields',
        last_message_at: 1781092800000,
        createdAt: '2026-06-09 12:00:00',
        message_count: 1
      }]);
      expect(sessions.length).assertEqual(1);
      expect(sessions[0].updatedAt).assertEqual('1781092800000');
      expect(sessions[0].createdAt).assertEqual('2026-06-09 12:00:00');
    });

    it('maps assistant messages and nested tools', 0, () => {
      const message = RemoteResponseMapper.chatMessage({
        id: 'm1',
        role: 'assistant',
        content: '',
        thinking: 'Thinking out loud',
        timestamp: '2026-06-10T12:00:00.000Z',
        items: [{
          type: 'tool',
          tool: {
            id: 'tool-1',
            name: 'shell',
            status: 'pending'
          },
          subItems: [{
            type: 'tool',
            tool: {
              id: 'tool-2',
              name: 'edit',
              status: 'done'
            }
          }]
        }]
      });
      expect(message.id).assertEqual('m1');
      expect(message.text).assertEqual('');
      expect(message.thinking || '').assertEqual('Thinking out loud');
      expect(message.status).assertEqual('done');
      expect(message.detail).assertEqual('shell · pending\nedit · done');
      expect(message.tools ? message.tools.length : 0).assertEqual(2);
    });

    it('uses fallback ids for message rendering keys', 0, () => {
      const fromMessageId = RemoteResponseMapper.chatMessage({
        message_id: 'desktop-message-1',
        role: 'assistant',
        content: 'Mapped from alternate id',
        timestamp: '2026-06-10T12:00:00.000Z'
      });
      const generated = RemoteResponseMapper.chatMessage({
        role: 'user',
        content: 'No id in response',
        timestamp: '2026-06-10T12:01:00.000Z'
      });

      expect(fromMessageId.id).assertEqual('desktop-message-1');
      expect(generated.id.length > 0).assertTrue();
      expect(generated.id.indexOf('user-2026-06-10T12:01:00.000Z-')).assertEqual(0);
    });

    it('maps active turns with item fallback text and tool detail', 0, () => {
      const message = RemoteResponseMapper.activeTurnToMessage({
        turn_id: 'turn-1',
        status: 'active',
        items: [{
          type: 'text',
          content: 'Working on it'
        }],
        tools: [{
          id: 'tool-3',
          name: 'read_file',
          status: 'running'
        }]
      });
      expect(message.id).assertEqual('active-turn-1');
      expect(message.turnId || '').assertEqual('turn-1');
      expect(message.role).assertEqual('assistant');
      expect(message.text).assertEqual('Working on it');
      expect(message.status).assertEqual('active');
      expect(message.detail).assertEqual('read_file · running');
    });

    it('does not promote subagent progress to active turn text', 0, () => {
      const message = RemoteResponseMapper.activeTurnToMessage({
        turn_id: 'turn-structured',
        status: 'active',
        items: [{
          type: 'subagent',
          is_subagent: true,
          content: 'Subagent check',
          subItems: [{
            type: 'text',
            content: 'Nested progress'
          }, {
            type: 'tool',
            tool: {
              id: 'tool-nested-1',
              name: 'inspect',
              status: 'running'
            }
          }]
        }]
      });

      expect(message.text).assertEqual('');
      expect(message.tools ? message.tools.length : 0).assertEqual(1);
      expect(message.detail).assertEqual('inspect · running');
    });

    it('does not use thinking as active turn final text', 0, () => {
      const message = RemoteResponseMapper.activeTurnToMessage({
        turn_id: 'turn-thinking-only',
        status: 'completed',
        thinking: 'Still reasoning',
        items: [{
          type: 'thinking',
          content: 'Still reasoning'
        }]
      });

      expect(message.text).assertEqual('');
      expect(message.thinking || '').assertEqual('Still reasoning');
      expect(message.status).assertEqual('completed');
    });
  });

  describe('ChatTimelineProjector', () => {
    it('filters seed messages and returns empty state for empty real history', 0, () => {
      const items = ChatTimelineProjector.project([
        chatMessage('system-seed', 'assistant', 'Welcome')
      ], [], chatMessage('', 'assistant', ''), false);

      expect(items.length).assertEqual(1);
      expect(items[0].type).assertEqual('empty_state');
    });

    it('deduplicates optimistic user messages replaced by persisted messages', 0, () => {
      const items = ChatTimelineProjector.project([
        chatMessage('remote-user-1', 'user', 'Explain this file')
      ], [
        chatMessage('msg-local-1', 'user', ' Explain this file ')
      ], chatMessage('', 'assistant', ''), false);

      expect(items.length).assertEqual(1);
      expect(items[0].id).assertEqual('message-remote-user-1');
      expect(items[0].type).assertEqual('user_message');
    });

    it('keeps optimistic image messages when persisted image payload differs', 0, () => {
      const items = ChatTimelineProjector.project([
        chatMessageWithImage('remote-user-1', 'user', 'Analyze image', 'remote.png', 'data:image/png;base64,remote')
      ], [
        chatMessageWithImage('msg-local-1', 'user', 'Analyze image', 'local.png', 'data:image/png;base64,local')
      ], chatMessage('', 'assistant', ''), false);

      expect(items.length).assertEqual(2);
      expect(items[0].id).assertEqual('message-remote-user-1');
      expect(items[1].id).assertEqual('pending-msg-local-1');
      expect(items[1].type).assertEqual('optimistic_user_message');
    });

    it('keeps active turn until matching assistant message is persisted', 0, () => {
      const activeTurn = chatMessage('active-turn-1', 'assistant', 'Done with the edit', 'completed');
      const beforeFinal = ChatTimelineProjector.project([
        chatMessage('remote-user-1', 'user', 'Please edit')
      ], [], activeTurn, false);
      const afterFinal = ChatTimelineProjector.project([
        chatMessage('remote-user-1', 'user', 'Please edit'),
        chatMessage('assistant-final-1', 'assistant', 'Done with the edit')
      ], [], activeTurn, false);

      expect(beforeFinal.length).assertEqual(2);
      expect(beforeFinal[1].type).assertEqual('assistant_live_turn');
      expect(beforeFinal[1].isFinalizing).assertEqual(true);
      expect(afterFinal.length).assertEqual(2);
      expect(afterFinal[1].id).assertEqual('message-assistant-final-1');
      expect(afterFinal[1].type).assertEqual('assistant_message');
    });

    it('updates active turn item id when visible content changes', 0, () => {
      const first = activeChatMessage('turn-stable-1', 'abc', 'active', 1);
      const second = activeChatMessage('turn-stable-1', 'abcdef', 'active', 2);
      const firstItems = ChatTimelineProjector.project([], [], first, false);
      const secondItems = ChatTimelineProjector.project([], [], second, false);

      expect(firstItems.length).assertEqual(1);
      expect(secondItems.length).assertEqual(1);
      expect(firstItems[0].id.indexOf('active-turn-stable-1-')).assertEqual(0);
      expect(secondItems[0].id.indexOf('active-turn-stable-1-')).assertEqual(0);
      expect(secondItems[0].id === firstItems[0].id).assertFalse();
    });

    it('hides active turn when final assistant id matches turn id', 0, () => {
      const activeTurn = activeChatMessage('turn-final-1', 'partial active text', 'completed');
      const items = ChatTimelineProjector.project([
        chatMessage('turn-final-1_assistant', 'assistant', 'rewritten final text')
      ], [], activeTurn, false);

      expect(items.length).assertEqual(1);
      expect(items[0].id).assertEqual('message-turn-final-1_assistant');
      expect(items[0].type).assertEqual('assistant_message');
    });

    it('keeps active turn when matching persisted assistant has no final text yet', 0, () => {
      const activeTurn = activeChatMessage('turn-final-wait-1', 'final answer', 'completed');
      const pendingFinal = chatMessage('turn-final-wait-1_assistant', 'assistant', '');
      pendingFinal.text = 'Still reasoning';
      pendingFinal.thinking = 'Still reasoning';
      const items = ChatTimelineProjector.project([pendingFinal], [], activeTurn, false);

      expect(items.length).assertEqual(2);
      expect(items[1].type).assertEqual('assistant_live_turn');
      expect(items[1].isFinalizing).assertEqual(true);
    });
  });

  describe('ChatTimelineStore', () => {
    it('updates same-id persisted messages when final content arrives later', 0, () => {
      const pendingFinal = chatMessage('assistant-final-same-id', 'assistant', '');
      pendingFinal.text = 'Still reasoning';
      pendingFinal.thinking = 'Still reasoning';
      const merged = RemoteUiState.mergeMessages([pendingFinal], [
        chatMessage('assistant-final-same-id', 'assistant', 'Final answer')
      ]);

      expect(merged.length).assertEqual(1);
      expect(merged[0].text).assertEqual('Final answer');
      expect(merged[0].thinking || '').assertEqual('');
    });

    it('owns optimistic merge and active turn cleanup rules', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      store.appendOptimisticMessage(chatMessage('msg-local-1', 'user', 'Please edit'));
      store.setActiveTurn(chatMessage('active-turn-1', 'assistant', 'Final text', 'completed'));
      store.mergePersistedMessages([
        chatMessage('remote-user-1', 'user', 'Please edit')
      ]);

      let state = store.snapshot();
      expect(state.persistedMessages.length).assertEqual(1);
      expect(state.optimisticMessages.length).assertEqual(0);
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('active-turn-1');
      expect(state.syncPhase).assertEqual('finalizing');

      store.mergePersistedMessages([
        chatMessage('assistant-final-1', 'assistant', 'Final text')
      ]);
      state = store.snapshot();
      expect(state.persistedMessages.length).assertEqual(2);
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('');
    });

    it('projects empty state through the store', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-empty');
      const items = store.project(false);

      expect(items.length).assertEqual(1);
      expect(items[0].type).assertEqual('empty_state');
    });

    it('creates a local active turn from send response turn ids', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      store.setLocalActiveTurn(' turn-preview-1 ');

      let state = store.snapshot();
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('active-turn-preview-1');
      expect(state.activeTurn ? (state.activeTurn.turnId || '') : '').assertEqual('turn-preview-1');
      expect(state.activeTurn ? state.activeTurn.status : '').assertEqual('active');
      expect(state.syncPhase).assertEqual('streaming');

      store.setLocalActiveTurn('turn-preview-2');
      state = store.snapshot();
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('active-turn-preview-1');
    });

    it('replaces pending active turns with remote turn ids', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      const pendingId = store.setPendingActiveTurn('msg-local-1');

      let state = store.snapshot();
      expect(pendingId).assertEqual('active-pending-msg-local-1');
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('active-pending-msg-local-1');
      expect(state.syncPhase).assertEqual('streaming');

      store.setLocalActiveTurn('turn-remote-1');
      state = store.snapshot();
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('active-turn-remote-1');
      expect(state.activeTurn ? (state.activeTurn.turnId || '') : '').assertEqual('turn-remote-1');
    });

    it('keeps streamed active turn text when send response arrives later', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      const active = chatMessage('active-turn-remote-1', 'assistant', 'partial text', 'active');
      active.turnId = 'turn-remote-1';
      store.setActiveTurn(active);

      store.setLocalActiveTurn('turn-remote-1');

      const state = store.snapshot();
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('active-turn-remote-1');
      expect(state.activeTurn ? state.activeTurn.text : '').assertEqual('partial text');
      expect(state.activeTurn ? (state.activeTurn.turnId || '') : '').assertEqual('turn-remote-1');
    });

    it('keeps active turn text from going backwards for the same turn id', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      store.setActiveTurn(activeChatMessage('turn-stream-1', 'abcdef'));
      store.setActiveTurn(activeChatMessage('turn-stream-1', 'abc', 'active', 2));

      const state = store.snapshot();
      expect(state.activeTurn ? state.activeTurn.text : '').assertEqual('abcdef');
      expect(state.activeTurn ? (state.activeTurn.turnId || '') : '').assertEqual('turn-stream-1');
      expect(state.activeTurn ? (state.activeTurn.renderVersion || 0) : 0).assertEqual(2);
    });

    it('monotonically merges structured text items for active turns', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      const first = activeChatMessage('turn-items-1', '');
      first.items = [{
        type: 'text',
        content: 'abcdef'
      }];
      const second = activeChatMessage('turn-items-1', '', 'active', 2);
      second.items = [{
        type: 'text',
        content: 'abc'
      }];

      store.setActiveTurn(first);
      store.setActiveTurn(second);

      const state = store.snapshot();
      const items = state.activeTurn && state.activeTurn.items ? state.activeTurn.items : [];
      expect(items.length).assertEqual(1);
      expect(items[0].content || '').assertEqual('abcdef');
    });

    it('updates active tool status from latest structured item snapshot', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      const first = activeChatMessage('turn-tool-1', '');
      first.items = [{
        type: 'tool',
        tool: {
          id: 'tool-1',
          name: 'read_file',
          status: 'running'
        }
      }];
      const second = activeChatMessage('turn-tool-1', '', 'active', 2);
      second.items = [{
        type: 'tool',
        tool: {
          id: 'tool-1',
          name: 'read_file',
          status: 'completed',
          duration_ms: 50
        }
      }];

      store.setActiveTurn(first);
      store.setActiveTurn(second);

      const state = store.snapshot();
      const items = state.activeTurn && state.activeTurn.items ? state.activeTurn.items : [];
      expect(items.length).assertEqual(1);
      expect(items[0].tool ? (items[0].tool.status || '') : '').assertEqual('completed');
      expect(items[0].tool ? (items[0].tool.duration_ms || 0) : 0).assertEqual(50);
    });

    it('clears active turn when persisted assistant id matches turn id', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      store.setActiveTurn(activeChatMessage('turn-final-store-1', 'partial active text', 'completed'));
      store.mergePersistedMessages([
        chatMessage('turn-final-store-1_assistant', 'assistant', 'rewritten final text')
      ]);

      const state = store.snapshot();
      expect(state.persistedMessages.length).assertEqual(1);
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('');
    });

    it('holds completed active turn until persisted assistant has final content', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      store.setActiveTurn(activeChatMessage('turn-final-store-wait-1', 'final answer', 'completed'));

      const pendingFinal = chatMessage('turn-final-store-wait-1_assistant', 'assistant', '');
      pendingFinal.text = 'Still reasoning';
      pendingFinal.thinking = 'Still reasoning';
      store.mergePersistedMessages([pendingFinal]);
      store.setActiveTurn(undefined);

      let state = store.snapshot();
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('active-turn-final-store-wait-1');
      expect(state.syncPhase).assertEqual('finalizing');

      store.mergePersistedMessages([
        chatMessage('turn-final-store-wait-1_assistant', 'assistant', 'final answer')
      ]);
      state = store.snapshot();
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('');
    });
  });

  describe('MarkdownParser', () => {
    it('parses common markdown blocks into stable block types', 0, () => {
      const blocks = MarkdownParser.parse('# Title\n\n- first\n- **second**\n\n> quote\n\n```ts\nconst value = 1;\n```\n\n| A | B |\n| --- | --- |\n| 1 | 2 |');

      expect(blocks.length).assertEqual(5);
      expect(blocks[0].type).assertEqual('heading');
      expect(blocks[0].level).assertEqual(1);
      expect(blocks[1].type).assertEqual('list');
      expect(blocks[1].items.length).assertEqual(2);
      expect(blocks[1].items[1].text).assertEqual('second');
      expect(blocks[2].type).assertEqual('quote');
      expect(blocks[3].type).assertEqual('code');
      expect(blocks[3].language).assertEqual('ts');
      expect(blocks[4].type).assertEqual('table');
    });

    it('formats inline code, emphasis, and links without markdown control text', 0, () => {
      const blocks = MarkdownParser.parse('Use `pnpm test`, **bold text**, and [docs](https://example.test).');

      expect(blocks.length).assertEqual(1);
      expect(blocks[0].type).assertEqual('paragraph');
      expect(blocks[0].text).assertEqual('Use pnpm test, bold text, and docs (https://example.test).');
      expect(blocks[0].inlines.length).assertEqual(7);
      expect(blocks[0].inlines[1].type).assertEqual('code');
      expect(blocks[0].inlines[3].type).assertEqual('strong');
      expect(blocks[0].inlines[5].type).assertEqual('link');
      expect(blocks[0].inlines[5].url).assertEqual('https://example.test');
    });
  });

  describe('ChatSessionController', () => {
    it('preserves active turn when unchanged poll omits active turn', 0, async () => {
      const manager = new FakePollSessionManager();
      const snapshots: ChatSessionSnapshot[] = [];
      manager.results = [pollResult({
        version: 1,
        changed: false,
        sessionState: 'running',
        title: 'Session',
        newMessages: [],
        totalMessageCount: 1
      })];
      const controller = new ChatSessionController(manager, {
        onSnapshot: (snapshot: ChatSessionSnapshot) => {
          snapshots.push(snapshot);
        },
        onError: (_error: Object) => {},
        canPoll: (_sessionId: string) => true
      });

      controller.start('session-1', {
        pollVersion: 1,
        knownMessageCount: 1,
        knownModelCatalogVersion: 0
      }, chatMessage('active-turn-1', 'assistant', 'Working', 'active'));
      await delay(20);
      controller.stop(false);

      expect(snapshots.length).assertEqual(1);
      expect(snapshots[0].changed).assertEqual(false);
      expect(snapshots[0].activeTurn ? snapshots[0].activeTurn.id : '').assertEqual('active-turn-1');
    });

    it('keeps completed active turn until final assistant message arrives', 0, async () => {
      const manager = new FakePollSessionManager();
      const snapshots: ChatSessionSnapshot[] = [];
      const controller = new ChatSessionController(manager, {
        onSnapshot: (snapshot: ChatSessionSnapshot) => {
          snapshots.push(snapshot);
        },
        onError: (_error: Object) => {},
        canPoll: (_sessionId: string) => true
      });

      manager.results = [pollResult({
        version: 2,
        changed: true,
        sessionState: 'idle',
        title: 'Session',
        newMessages: [],
        totalMessageCount: 1,
        activeTurn: chatMessage('active-turn-2', 'assistant', 'Final text', 'completed')
      })];
      controller.start('session-1', {
        pollVersion: 1,
        knownMessageCount: 1,
        knownModelCatalogVersion: 0
      }, chatMessage('active-turn-2', 'assistant', 'Final text', 'active'));
      await delay(20);

      manager.results = [pollResult({
        version: 3,
        changed: true,
        sessionState: 'idle',
        title: 'Session',
        newMessages: [],
        totalMessageCount: 1
      })];
      await controller.pollNow();

      manager.results = [pollResult({
        version: 4,
        changed: true,
        sessionState: 'idle',
        title: 'Session',
        newMessages: [
          chatMessage('assistant-final-1', 'assistant', 'Final text')
        ],
        totalMessageCount: 2
      })];
      await controller.pollNow();
      controller.stop(false);

      expect(snapshots.length).assertEqual(3);
      expect(snapshots[0].activeTurn ? snapshots[0].activeTurn.status : '').assertEqual('completed');
      expect(snapshots[1].activeTurn ? snapshots[1].activeTurn.id : '').assertEqual('active-turn-2');
      expect(snapshots[2].activeTurn ? snapshots[2].activeTurn.id : '').assertEqual('');
    });
  });
}
