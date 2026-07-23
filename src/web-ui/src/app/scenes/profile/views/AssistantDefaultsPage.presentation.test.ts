import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readStylesheet(): string {
  return readFileSync(
    fileURLToPath(new URL('./NurseryView.scss', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('Assistant defaults skill presentation', () => {
  it('keeps covered state styling on skill rows and readable on narrow screens', () => {
    const stylesheet = readStylesheet();
    const personaStart = stylesheet.indexOf('.tc-persona-doc-row {');
    const personaEnd = stylesheet.indexOf('.tc-persona-doc-editor', personaStart);
    const skillStart = stylesheet.indexOf('.tc-skill-row {');
    const skillEnd = stylesheet.indexOf('.tc-hero {', skillStart);

    expect(stylesheet.slice(personaStart, personaEnd)).not.toContain('&--covered');

    const skillSection = stylesheet.slice(skillStart, skillEnd);
    expect(skillSection).toContain('&--covered');
    expect(skillSection).toMatch(/@media \(max-width: 720px\)[\s\S]*\.tc-skill-row[\s\S]*white-space: normal;/);
  });
});
