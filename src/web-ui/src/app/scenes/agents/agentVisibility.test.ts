import { describe, expect, it } from 'vitest';
import { isAgentInOverviewZone } from './agentVisibility';

describe('agentVisibility', () => {
  it('hides review agents from backend-provided hidden ids', () => {
    expect(
      isAgentInOverviewZone(
        { id: 'ReviewDocs' },
        new Set(['ReviewDocs']),
      ),
    ).toBe(false);
  });
});
