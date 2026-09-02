import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const source = readFileSync(
  fileURLToPath(new URL('./ModelSettingsPage.tsx', import.meta.url)),
  'utf8',
);

describe('ModelSettingsPage presentation', () => {
  it('uses intentional separators instead of Unicode replacement characters', () => {
    expect(source).not.toContain('\uFFFD');
    expect(source.match(/\{' · '\}/g)).toHaveLength(2);
  });

  it('declares required model fields structurally instead of embedding asterisks in label copy', () => {
    expect(source).not.toMatch(/label=\{`[^`]*\*[^`]*`\}/);
    expect(source.match(/<ConfigPageRow label=\{t\('form\.configName'\)\} required/g)).toHaveLength(2);
    expect(source.match(/<ConfigPageRow label=\{t\('form\.modelSelection'\)\} required/g)).toHaveLength(2);
    expect(source.match(/<ConfigPageRow label=\{t\('form\.baseUrl'\)\} required/g)).toHaveLength(1);
    expect(source).toContain('<ConfigPageRow label={label} required align="center" wide>');
  });
});
