import { describe, expect, it } from 'vitest';
import { isUserSelectableToolName } from './toolVisibility';

describe('isUserSelectableToolName', () => {
  it.each(['GetToolSpec', 'CallDeferredTool'])(
    'hides the internal gateway tool %s',
    (toolName) => {
      expect(isUserSelectableToolName(toolName)).toBe(false);
    },
  );

  it('keeps regular tools selectable', () => {
    expect(isUserSelectableToolName('Read')).toBe(true);
  });
});
