import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import type { DialogTurn, Session } from '@/flow_chat/types/flow-chat';
import { isFloatingMiniChatSessionExecuting } from './floatingMiniChatActivity';

function sessionWithStatuses(
  ...statuses: DialogTurn['status'][]
): Pick<Session, 'dialogTurns'> {
  return {
    dialogTurns: statuses.map((status) => ({ status }) as DialogTurn),
  };
}

function readSource(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('floating MiniApp chat activity', () => {
  it('uses the design-system launcher with the catalog mic for Hello', () => {
    const component = readSource('./FloatingMiniChat.tsx');
    expect(component).toContain('IconButton, LauncherButton, Tooltip');
    expect(component).toContain('<LauncherButton');
    expect(component).toContain('leadingIcon={<Icon name="mic" />}');
    expect(component).toContain("tVoice('voiceCall.call.launcherLabel')");
    expect(component).not.toContain('className="bitfun-fmc__button-label"');
  });

  it('leaves the standalone Hello visual states with LauncherButton', () => {
    const stylesheet = readSource('./FloatingMiniChat.scss');

    expect(stylesheet).not.toContain('--bf-color-control-launcher');
    expect(stylesheet).not.toContain('--bf-color-control-highlight');
  });

  it.each([
    'pending',
    'image_analyzing',
    'processing',
    'finishing',
    'cancelling',
  ] satisfies DialogTurn['status'][])(
    'treats %s as an executing turn',
    (status) => {
      expect(
        isFloatingMiniChatSessionExecuting(sessionWithStatuses(status)),
      ).toBe(true);
    },
  );

  it.each([
    'completed',
    'cancelled',
    'error',
  ] satisfies DialogTurn['status'][])(
    'treats %s as a settled turn',
    (status) => {
      expect(
        isFloatingMiniChatSessionExecuting(sessionWithStatuses(status)),
      ).toBe(false);
    },
  );

  it('uses only the latest turn and handles an absent session', () => {
    expect(
      isFloatingMiniChatSessionExecuting(
        sessionWithStatuses('processing', 'completed'),
      ),
    ).toBe(false);
    expect(isFloatingMiniChatSessionExecuting(undefined)).toBe(false);
  });

  it('binds the indicator to the tracked MiniApp session with reduced-motion fallback', () => {
    const component = readSource('./FloatingMiniChat.tsx');
    const stylesheet = readSource('./FloatingMiniChat.scss');

    expect(component).toContain('trackedSession: activeMiniAppSession');
    expect(component).toContain('isMiniAppSessionExecuting && (');
    expect(component).toContain('className="bitfun-fmc__button-activity"');
    expect(component).toContain('aria-busy={isMiniAppSessionExecuting || undefined}');
    expect(stylesheet).toContain('@keyframes fmc-button-activity-spin');
    expect(stylesheet).toContain('@keyframes fmc-button-activity-glow');
    expect(stylesheet).toContain('@media (prefers-reduced-motion: reduce)');
  });
});
