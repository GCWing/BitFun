// @vitest-environment node
/**
 * Source-level contract for the R-NS-01 empty-state contract:
 * the Sessions section must not render the "no sessions" empty text.
 *
 * SessionsSection pulls in many stores/contexts, so a full component render
 * is not practical here. These assertions lock the source contract instead:
 *  - the `noSessions` key is no longer referenced by SessionsSection;
 *  - the empty branches keep the inline-list container (no layout jump);
 *  - the loading / loadError branches are preserved.
 */

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8').replace(/\r\n/g, '\n');
}

const sessionsSectionSource = readSource('./SessionsSection.tsx');

describe('SessionsSection empty-state contract', () => {
  it('does not render the no-sessions empty text', () => {
    expect(sessionsSectionSource).not.toContain('nav.sessions.noSessions');
    expect(sessionsSectionSource).not.toContain('__inline-empty');
  });

  it('keeps the empty branch rendered as an empty inline-list container (no layout jump)', () => {
    expect(sessionsSectionSource).toContain(
      'className="bitfun-nav-panel__inline-list" />',
    );
  });

  it('keeps the loading state', () => {
    expect(sessionsSectionSource).toContain("t('nav.sessions.loading')");
    expect(sessionsSectionSource).toContain('bitfun-nav-panel__inline-loading');
  });

  it('keeps the load-error retry state', () => {
    expect(sessionsSectionSource).toContain("t('nav.sessions.loadFailedRetry')");
    expect(sessionsSectionSource).toContain('data-bf-part="retry"');
  });
});
