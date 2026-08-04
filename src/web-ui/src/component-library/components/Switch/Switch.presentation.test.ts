import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readStylesheet(): string {
  return readFileSync(
    fileURLToPath(new URL('./Switch.scss', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('Switch presentation', () => {
  it('keeps the checked thumb inside the track and independent from primary button colors', () => {
    const stylesheet = readStylesheet();
    const checkedStart = stylesheet.indexOf('&__input:checked + &__track &__thumb');
    const checkedEnd = stylesheet.indexOf("data-bf-appearance-mode='dark'", checkedStart);
    const checkedRule = stylesheet.slice(checkedStart, checkedEnd);

    expect(checkedRule).toContain('transform: translateX(16px);');
    expect(checkedRule).toContain('var(--bf-appearance-token-color-accent-500)');
    expect(checkedRule).not.toContain('left:');
    expect(checkedRule).not.toContain('btn-primary');
  });
});
