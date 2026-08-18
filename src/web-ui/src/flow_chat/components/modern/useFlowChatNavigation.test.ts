import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const source = fs.readFileSync(path.resolve(__dirname, 'useFlowChatNavigation.ts'), 'utf8');

describe('useFlowChatNavigation natural navigation contract', () => {
  it('uses natural Turn navigation for resolved focus targets', () => {
    expect(source).toContain('list.navigateToTurn(target.resolvedTurnId');
    expect(source).not.toContain('pinTurnToTop');
  });

  it('does not subscribe to a send-message positioning event', () => {
    expect(source).not.toContain('sticky-latest');
    expect(source).not.toContain('PIN_TURN_TO_TOP');
  });
});
