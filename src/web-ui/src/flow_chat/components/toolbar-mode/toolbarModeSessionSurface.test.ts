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
  });
});

/**
 * The one composer the bubble may own is the MiniApp composer: an Agentic
 * MiniApp (PPT Live) claims the bubble as its input surface, so what the user
 * types is handed to that MiniApp instead of being submitted into the host
 * session. It is not a second chat composer — it never talks to FlowChat — and
 * it exists only while a claim is active.
 */
describe('floating mini chat bubble MiniApp composer', () => {
  const source = readSource('../../../app/layout/FloatingMiniChat.tsx');

  it('swaps out the session composer only while a MiniApp holds a claim', () => {
    expect(source).toContain('showChatInput={!activeComposerClaim}');
    expect(source).toContain('activeComposerClaim && (');
  });

  it('hands input to the MiniApp rather than to the session', () => {
    expect(source).toContain('MINIAPP_COMPOSER_MESSAGE_EVENT');
    // Routing is keyed by claim token, never by app id: one app can have two
    // live runners (installed app + draft preview) and only one owns the input.
    expect(source).toContain('detail: { token, text }');
  });

  it('prefills without sending when a MiniApp offers an example prompt', () => {
    expect(source).toContain('MINIAPP_COMPOSER_DRAFT_EVENT');
    expect(source).toContain('setComposerPrefill');
  });
});
