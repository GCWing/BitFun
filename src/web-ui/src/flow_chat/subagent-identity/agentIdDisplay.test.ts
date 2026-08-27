import { describe, expect, it } from 'vitest';

import { formatAgentIdForDisplay } from './agentIdDisplay';

describe('formatAgentIdForDisplay', () => {
  it('turns agent ID separators into spaces and capitalizes the first character', () => {
    expect(formatAgentIdForDisplay('parser_review-worker_2')).toBe('Parser review worker 2');
    expect(formatAgentIdForDisplay('docs--audit__worker')).toBe('Docs audit worker');
  });
});
