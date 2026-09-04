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

    expect(stylesheet).not.toContain('.openbitfun-config-page-row {');
    expect(stylesheet.slice(editorStart, editorEnd)).toContain('padding: var(--openbitfun-space-4);');
  });

  it('presents JSON as an advanced text-labelled mode without the extensions overview', () => {
    const source = readFileSync(
      fileURLToPath(new URL('./McpToolsConfig.tsx', import.meta.url)),
      'utf8',
    );

    expect(source).toContain("title={showJsonEditor ? tMcp('jsonEditor.title') : tMcp('section.serverList.title')}");
    expect(source).toContain("{showJsonEditor ? tMcp('actions.backToList') : tMcp('actions.jsonConfig')}");
    expect(source).toContain('{!showJsonEditor && <ExternalMcpOverview />}');
    expect(source).not.toContain('<h3>{tMcp(\'jsonEditor.title\')}</h3>');
  });
});
