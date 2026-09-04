import { describe, expect, it } from 'vitest';
import {
  shouldRouteVoiceTaskToMiniApp,
  type VoiceMiniAppCallTarget,
} from './voiceClientContext';

const target: VoiceMiniAppCallTarget = {
  kind: 'miniapp',
  appId: 'builtin-ppt-live',
  appName: 'PPT Live',
  claimToken: 'builtin-ppt-live#1',
  sessionId: 'miniapp-session',
};

describe('realtime voice task routing', () => {
  it('keeps an unqualified task in the MiniApp conversation that launched voice', () => {
    expect(shouldRouteVoiceTaskToMiniApp(target)).toBe(true);
    expect(shouldRouteVoiceTaskToMiniApp(target, '   ')).toBe(true);
  });

  it('lets an explicit workspace override the captured MiniApp target', () => {
    expect(shouldRouteVoiceTaskToMiniApp(target, 'workspace-1')).toBe(false);
    expect(shouldRouteVoiceTaskToMiniApp(null)).toBe(false);
  });
});
