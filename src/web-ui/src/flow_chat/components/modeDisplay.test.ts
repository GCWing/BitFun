import { describe, expect, it } from 'vitest';
import { getModeDisplayDescription, getModeDisplayName } from './modeDisplay';

function makeTranslator(values: Record<string, string>) {
  return (key: string) => values[key] ?? '';
}

describe('modeDisplay', () => {
  it('resolves localized IntentCoding mode name and description', () => {
    const t = makeTranslator({
      'chatInput.modeNames.IntentCoding': 'Intent Coding',
      'chatInput.modeDescriptions.IntentCoding': 'Intent-aligned coding',
    });
    const mode = {
      id: 'IntentCoding',
      name: 'Intent Coding backend',
      description: 'backend description',
    };

    expect(getModeDisplayName(t, mode)).toBe('Intent Coding');
    expect(getModeDisplayDescription(t, mode)).toBe('Intent-aligned coding');
  });

  it('falls back to backend values when localization is missing', () => {
    const t = makeTranslator({});
    const mode = {
      id: 'IntentCoding',
      name: 'Intent Coding backend',
      description: 'backend description',
    };

    expect(getModeDisplayName(t, mode)).toBe('Intent Coding backend');
    expect(getModeDisplayDescription(t, mode)).toBe('backend description');
  });

  it('falls back to mode name when description is empty', () => {
    const t = makeTranslator({});
    const mode = {
      id: 'IntentCoding',
      name: 'Intent Coding backend',
      description: '',
    };

    expect(getModeDisplayDescription(t, mode)).toBe('Intent Coding backend');
  });
});

