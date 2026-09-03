import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const stylesheets = [
  './BrowserPanel.scss',
  './BrowserScene.scss',
].map(path => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8'));

function addressWrapperRule(stylesheet: string): string {
  const match = stylesheet.match(/&__address\s*\{([\s\S]*?)\n\s*}\n\n\s*&__address-field/);
  expect(match).not.toBeNull();
  return match?.[1] ?? '';
}

describe('browser address field presentation', () => {
  it.each(stylesheets)('leaves the field surface to the shared Input component', (stylesheet) => {
    const rule = addressWrapperRule(stylesheet);

    expect(rule).not.toMatch(/\b(?:background|border|border-radius|box-shadow|height|padding)\s*:/);
    expect(rule).not.toContain('input {');
    expect(rule).not.toContain('overflow: hidden');
  });
});
