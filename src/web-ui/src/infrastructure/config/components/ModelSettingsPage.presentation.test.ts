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
});
