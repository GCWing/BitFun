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

  it('keeps no private conversation view or composer of its own', () => {
    expect(source).not.toContain('ModernFlowChatContainer');
    expect(source).not.toContain('<input');
    expect(source).not.toContain('toolbar-send-message');
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
