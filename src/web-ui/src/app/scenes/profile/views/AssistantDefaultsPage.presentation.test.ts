import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readStylesheet(): string {
  return readFileSync(
    fileURLToPath(new URL('./AssistantDefaultsPage.scss', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('Assistant defaults capability presentation', () => {
  it('keeps unavailable rows legible and collapses secondary columns on narrow screens', () => {
    const stylesheet = readStylesheet();

    expect(stylesheet).toMatch(
      /\.assistant-defaults-row\s*\{[\s\S]*&--unavailable\s*\{[\s\S]*opacity:\s*0\.7;/,
    );
    expect(stylesheet).toMatch(
      /@media \(max-width: 760px\)[\s\S]*\.assistant-defaults-row__source,[\s\S]*display:\s*none;/,
    );
  });
});
