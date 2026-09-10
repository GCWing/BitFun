import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const readStyles = (relativePath: string) => readFileSync(
  new URL(relativePath, import.meta.url),
  'utf8',
);

const extractRule = (styles: string, selector: string): string => {
  const start = styles.indexOf(selector);
  if (start < 0) return '';

  const openingBrace = styles.indexOf('{', start);
  let depth = 0;
  for (let index = openingBrace; index < styles.length; index += 1) {
    if (styles[index] === '{') depth += 1;
    if (styles[index] === '}') depth -= 1;
    if (depth === 0) return styles.slice(start, index + 1);
  }

  return '';
};

describe('notification icon presentation', () => {
  it('keeps every toast notification status icon free of circular tiles', () => {
    const rules = [
      extractRule(readStyles('./NotificationItem.scss'), '&__icon'),
      extractRule(readStyles('./ProgressNotification.scss'), '&__icon'),
      extractRule(readStyles('./ProgressNotification.scss'), '&__status-icon'),
      extractRule(readStyles('./LoadingNotification.scss'), '&__icon'),
      extractRule(readStyles('./LoadingNotification.scss'), '&__status-icon'),
    ];

    for (const rule of rules) {
      expect(rule).not.toBe('');
      expect(rule).not.toMatch(/(?:background|border-radius)\s*:/);
    }
  });
});
