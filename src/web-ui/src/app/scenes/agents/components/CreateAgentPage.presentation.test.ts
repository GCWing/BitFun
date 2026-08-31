import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readLocalFile(fileName: string): string {
  return readFileSync(
    fileURLToPath(new URL(fileName, import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('CreateAgentPage presentation contract', () => {
  it('keeps the reference-led header and two-panel information hierarchy', () => {
    const source = readLocalFile('./CreateAgentPage.tsx');

    expect(source).not.toContain('Logo-ICON-128.png');
    expect(source).toContain('th-create-page__panel--definition');
    expect(source).toContain('th-create-page__panel--capabilities');
    expect(source.match(/data-bf-part="column"/g)).toHaveLength(2);
    expect(source).toContain('data-bf-part="tool"');
    expect(source).toContain("t('agentsOverview.form.basicInformation')");
    expect(source).toContain("t('agentsOverview.form.contextPolicy')");
    expect(source).toContain("t('agentsOverview.form.prompt')");
    expect(source.match(/className="th-create-page__divider"/g)).toHaveLength(1);
  });

  it('keeps the primary creation path keyboard- and state-aware', () => {
    const source = readLocalFile('./CreateAgentPage.tsx');

    expect(source).toContain('id="custom-agent-form"');
    expect(source).toContain('form="custom-agent-form"');
    expect(source).toContain('onSubmit={(event) => {');
    expect(source).toContain('aria-pressed={kind === candidateKind}');
    expect(source).toContain('aria-pressed={level === candidateLevel}');
    expect(source).toContain('aria-pressed={isSelected}');
  });

  it('shows runtime context feedback without merging it into the authored prompt', () => {
    const source = readLocalFile('./CreateAgentPage.tsx');

    expect(source).toContain('const selectedContextSections = VISIBLE_CONTEXT_SECTIONS.filter');
    expect(source).toContain('data-bf-part="contextPreview"');
    expect(source).toContain('aria-live="polite"');
    expect(source).toContain('aria-describedby="custom-agent-runtime-context-preview"');
    expect(source).toContain("t('agentsOverview.form.contextPreviewEmpty')");
    expect(source).toContain('value={prompt}');
  });

  it('uses compact two-column cards that collapse cleanly on narrow windows', () => {
    const stylesheet = readLocalFile('./CreateAgentPage.scss');

    expect(stylesheet).toContain(
      'grid-template-columns: minmax(0, 0.98fr) minmax(0, 1.02fr);',
    );
    expect(stylesheet).toMatch(
      /\.th-create-page__panel \{[\s\S]*border: 1px solid[\s\S]*border-radius:/,
    );
    expect(stylesheet).toMatch(
      /\.th-create-page__panel \{[\s\S]*background: var\(--bf-color-surface-scene\);[\s\S]*box-shadow: none;/,
    );
    expect(stylesheet).not.toContain(
      'var(--bf-color-action-neutral-surface) 84%',
    );
    expect(stylesheet).toMatch(
      /\.th-create-panel__context-options \{[\s\S]*display: flex;[\s\S]*flex-wrap: wrap;/,
    );
    expect(stylesheet).toMatch(
      /@media \(max-width: 960px\)[\s\S]*\.th-create-page__columns \{[\s\S]*grid-template-columns: minmax\(0, 1fr\);/,
    );
    expect(stylesheet).toMatch(
      /@media \(max-width: 480px\)[\s\S]*\.th-create-panel__identity-fields,[\s\S]*\.th-create-panel__tool-summary \.tool-group-summary[\s\S]*grid-template-columns: minmax\(0, 1fr\);/,
    );
  });
});
