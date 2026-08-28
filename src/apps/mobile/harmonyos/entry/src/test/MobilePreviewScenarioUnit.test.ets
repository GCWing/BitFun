import { describe, it, expect } from '@ohos/hypium';
import {
  MobilePreviewScenario,
  MobilePreviewScenarios
} from '../main/ets/generated/MobilePreviewScenarios';

const sensitiveContentPattern: RegExp = /akid|aki|sk-|-----begin|password|secret|token/i;
const base64LookingPattern: RegExp = /[A-Za-z0-9+/=]{40,}/;

function containsSensitiveContent(scenario: MobilePreviewScenario): boolean {
  const fields: string[] = [
    scenario.title,
    scenario.description,
    scenario.headerTitle,
    scenario.headerSubtitle,
    scenario.composerDraft,
    scenario.composerPlaceholder
  ];
  scenario.messages.forEach(message => fields.push(message.text));
  const content = fields.join('\n');
  return sensitiveContentPattern.test(content) || base64LookingPattern.test(content);
}

export default function mobilePreviewScenarioUnitTest() {
  describe('Mobile preview scenarios', () => {
    it('keeps the three scenario ids stable', 0, () => {
      const ids: string[] = [
        MobilePreviewScenarios.connectedConversation.id,
        MobilePreviewScenarios.streamingDark.id,
        MobilePreviewScenarios.reconnectingWide.id
      ];
      expect(ids.length).assertEqual(3);
      expect(ids[0]).assertEqual('connected-conversation');
      expect(ids[1]).assertEqual('streaming-dark');
      expect(ids[2]).assertEqual('reconnecting-wide');
    });

    it('matches the preview contract for appearance, viewport, connection, and streaming', 0, () => {
      const connected = MobilePreviewScenarios.connectedConversation;
      expect(connected.appearance).assertEqual('light');
      expect(connected.viewportWidth).assertEqual(390);
      expect(connected.viewportHeight).assertEqual(844);
      expect(connected.connectionPhase).assertEqual('connected');
      expect(connected.streaming).assertFalse();

      const streaming = MobilePreviewScenarios.streamingDark;
      expect(streaming.appearance).assertEqual('dark');
      expect(streaming.viewportWidth).assertEqual(390);
      expect(streaming.viewportHeight).assertEqual(844);
      expect(streaming.connectionPhase).assertEqual('connected');
      expect(streaming.streaming).assertTrue();

      const reconnecting = MobilePreviewScenarios.reconnectingWide;
      expect(reconnecting.appearance).assertEqual('light');
      expect(reconnecting.viewportWidth).assertEqual(1024);
      expect(reconnecting.viewportHeight).assertEqual(768);
      expect(reconnecting.connectionPhase).assertEqual('reconnecting');
      expect(reconnecting.streaming).assertFalse();
    });

    it('keeps required preview copy and messages populated', 0, () => {
      const scenarios: MobilePreviewScenario[] = [
        MobilePreviewScenarios.connectedConversation,
        MobilePreviewScenarios.streamingDark,
        MobilePreviewScenarios.reconnectingWide
      ];
      scenarios.forEach(scenario => {
        expect(scenario.headerTitle.length > 0).assertTrue();
        expect(scenario.composerPlaceholder.length > 0).assertTrue();
        expect(scenario.messages.length > 0).assertTrue();
        expect(scenario.messages.some(message => message.text.length > 0)).assertTrue();
      });
    });

    it('contains no sensitive content in scenario fields', 0, () => {
      const scenarios: MobilePreviewScenario[] = [
        MobilePreviewScenarios.connectedConversation,
        MobilePreviewScenarios.streamingDark,
        MobilePreviewScenarios.reconnectingWide
      ];
      scenarios.forEach(scenario => {
        expect(containsSensitiveContent(scenario)).assertFalse();
      });
    });
  });
}
