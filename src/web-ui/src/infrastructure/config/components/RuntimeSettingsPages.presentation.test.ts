import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    'utf8',
  );
}

describe('RuntimeSettingsPages combined execution page', () => {
  it('renders execution before device control in one page without tabs', () => {
    const source = readSource('./RuntimeSettingsPages.tsx');
    const appearance = readSource('./RuntimeSettingsPages.appearance.ts');
    const executionIndex = source.indexOf("{page === 'execution' || page === 'execution-control' ? (");
    const deviceControlIndex = source.indexOf("{page === 'device-control' || page === 'execution-control' ? (");

    expect(executionIndex).toBeGreaterThan(-1);
    expect(deviceControlIndex).toBeGreaterThan(executionIndex);
    expect(source).toContain('export function ExecutionControlSettingsPage()');
    expect(source).not.toContain('<Tabs');
    expect(source).not.toContain('<TabPane');
    expect(appearance).toContain("'execution-control'");
  });
});
