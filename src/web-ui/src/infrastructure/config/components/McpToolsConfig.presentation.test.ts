import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readStylesheet(): string {
  return readFileSync(
    fileURLToPath(new URL('./McpToolsConfig.scss', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('MCP settings presentation', () => {
  it('preserves shared row gutters and gives the JSON editor an inset gutter', () => {
    const stylesheet = readStylesheet();
    const editorStart = stylesheet.indexOf('&__json-editor {');
    const editorEnd = stylesheet.indexOf('&__json-editor-header', editorStart);

    expect(stylesheet).not.toContain('.bitfun-config-page-row {');
    expect(stylesheet.slice(editorStart, editorEnd)).toContain('padding: var(--bf-space-4);');
  });
});
