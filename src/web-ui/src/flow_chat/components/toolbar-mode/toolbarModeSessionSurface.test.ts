/**
 * The floating chat surfaces must render the main window session surface
 * itself, never a reduced copy of it. These assertions exist so the parallel
 * conversation/composer implementations that used to live in each one do not
 * creep back in — those duplicates were always feature subsets and doubled the
 * maintenance cost of every chat change.
 */

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8').replace(/\r\n?/g, '\n');
}

const SURFACES = [
  { name: 'floating window mode (ToolbarMode)', path: './ToolbarMode.tsx' },
  { name: 'floating mini chat bubble', path: '../../../app/layout/FloatingMiniChat.tsx' },
];

describe.each(SURFACES)('$name session surface', ({ path }) => {
  const source = readSource(path);

  it('renders the main window ChatPane with its full composer', () => {
    expect(source).toContain("from '@/app/scenes/session/ChatPane'");
    expect(source).toContain('<ChatPane');
    expect(source).toContain('showChatInput');
  });

  it('keeps no private conversation view or session composer of its own', () => {
    expect(source).not.toContain('ModernFlowChatContainer');
    expect(source).not.toContain('<input');
    expect(source).not.toContain('toolbar-send-message');
    // Nothing here may submit into the host session: that is ChatInput's job.
    expect(source).not.toContain('sendMessage');
  });

  it('takes its session affordances from the shared SessionMenu', () => {
    expect(source).toContain('<SessionMenu');
    expect(source).toContain('useFlowChatSessions');
    // The "+" must open the shared menu rather than creating a session
    // directly, and the title must stay a plain display — the two surfaces
    // diverged on exactly these before.
    expect(source).not.toContain('title-btn');
    expect(source).not.toContain('ChevronDown');
    expect(source).not.toContain("CustomEvent('toolbar-create-session'");
    expect(source).toContain('title-display');
  });
});

describe('session surface composition', () => {
  it('reuses the session scene chat surface that ChatPane already composes', () => {
    const chatPaneSource = readSource('../../../app/scenes/session/ChatPane.tsx');

    expect(chatPaneSource).toContain(
      "from '../../../flow_chat/components/modern/ModernFlowChatContainer'"
    );
    expect(chatPaneSource).toContain("from '../../../flow_chat/components/ChatInput'");
    expect(chatPaneSource).toContain('registration={chatInputRegistration}');
  });
});

describe('floating mini chat bubble MiniApp registration', () => {
  const source = readSource('../../../app/layout/FloatingMiniChat.tsx');
  const styles = readSource('../../../app/layout/FloatingMiniChat.scss');

  it('keeps the full shared composer while a MiniApp holds a registration', () => {
    expect(source).toContain('showChatInput');
    expect(source).toContain('chatInputRegistration={chatInputRegistration}');
    expect(source).not.toContain('showChatInput={!activeComposerClaim}');
    expect(source).not.toContain('const MiniAppComposer');
    expect(source).not.toContain('<MiniAppComposer');
    expect(source).not.toContain('<textarea');
    expect(source).not.toContain('miniapp-composer');
    expect(styles).not.toContain('miniapp-composer');
    expect(styles).not.toContain('panel--miniapp');
  });

  it('registers a token-scoped submit route without replacing ChatInput', () => {
    expect(source).toContain('MINIAPP_COMPOSER_MESSAGE_EVENT');
    // Routing is keyed by claim token, never by app id: one app can have two
    // live runners (installed app + draft preview) and only one owns the input.
    expect(source).toContain('token: activeComposerToken');
    expect(source).toContain('text: submission.text');
    expect(source).toContain('contexts: submission.contexts');
    expect(source).toContain('registrationId: activeComposerClaim.token');
    expect(source).toContain('onSubmit: handleMiniAppSubmit');
  });

  it('prefills without sending when a MiniApp offers an example prompt', () => {
    expect(source).toContain('MINIAPP_COMPOSER_DRAFT_EVENT');
    expect(source).toContain('setMiniAppComposerDraft');
    expect(source).toContain('onDraftConsumed: handleMiniAppDraftConsumed');
  });

  it('activates the freshly bound topic before opening a MiniApp draft', () => {
    expect(source).toContain('const liveClaim = useMiniAppStore.getState().composerClaims');
    expect(source).toContain('const sessionId = liveClaim.sessionId || detail.sessionId');

    const rememberIndex = source.indexOf('rememberPreviousHostSession(detail.token);');
    const activateIndex = source.indexOf('activateMiniAppSession(draftClaim);');
    const draftIndex = source.indexOf(
      'setMiniAppComposerDraft(detail.text, detail.token, sessionId);',
    );
    const openIndex = source.indexOf('handleOpen();', draftIndex);
    expect(rememberIndex).toBeGreaterThan(-1);
    expect(activateIndex).toBeGreaterThan(rememberIndex);
    expect(draftIndex).toBeGreaterThan(activateIndex);
    expect(openIndex).toBeGreaterThan(draftIndex);
  });

  it('does not clear a draft that already belongs to the newly focused session', () => {
    expect(source).toContain('claimToken: string');
    expect(source).toContain('sessionId?: string');
    expect(source).toContain('current.claimToken !== activeComposerToken');
    expect(source).toContain('current.sessionId !== activeComposerSessionId');
  });

  it('fails closed around an Agentic MiniApp and shows only its exact topic session', () => {
    expect(source).toContain('activeComposerClaim?.sessionId');
    expect(source).toContain('isFloatingMiniChatIsolated({');
    expect(source).toContain('canRenderFloatingMiniChatSession({');
    expect(source).toContain('surfaceMounted && isMiniAppSessionReady');
    expect(source).toContain(
      'surfaceMounted && isMiniAppBubbleIsolated && !isMiniAppSessionReady'
    );
    expect(source).toContain('previousHostSessionRef');
    expect(source).toContain('restorePreviousHostSession(activeComposerToken)');
    expect(source).toContain('restorePreviousHostSession();');
  });

  it('renders the MiniApp entry model against the topic session workspace', () => {
    expect(source).toContain('<MiniAppBubbleWelcome');
    expect(source).toContain('customization={bubbleCustomization}');
    expect(source).toContain('workspacePath={displayedSession?.workspacePath}');
    expect(source).toContain('emptyState={activeComposerClaim ? (');
    expect(source).toContain('renderMiniAppIcon');
    expect(source).not.toContain('getMiniAppIconGradient');
    // The ordinary project workspace remains valid only for the host session.
    expect(source).toContain(
      'isMiniAppBubbleIsolated\n                  ? displayedSession?.workspacePath\n                  : workspacePath'
    );
  });

  it('hydrates a restored hidden topic session instead of substituting the latest chat', () => {
    const bridgeSource = readSource(
      '../../../app/scenes/miniapps/hooks/useMiniAppBridge.ts'
    );

    expect(bridgeSource).toContain('if (!result.created)');
    expect(bridgeSource).toContain('flowChatStore.loadSessionHistory(');
    expect(bridgeSource).toContain('{ includeInternal: true }');
  });

  it('forwards a MiniApp agent turn user-facing label separately from its prompt', () => {
    const bridgeSource = readSource(
      '../../../app/scenes/miniapps/hooks/useMiniAppBridge.ts'
    );

    expect(bridgeSource).toContain(
      "typeof params.displayText === 'string' ? params.displayText : undefined"
    );
  });
});
