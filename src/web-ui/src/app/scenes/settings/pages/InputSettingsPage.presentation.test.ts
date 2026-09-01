import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    'utf8',
  );
}

describe('InputSettingsPage structure', () => {
  it('keeps voice input and shortcuts in one continuous page without tabs', () => {
    const source = readSource('./InputSettingsPage.tsx');
    const voiceIndex = source.indexOf('<VoiceInputConfig />');
    const shortcutsIndex = source.indexOf('<KeyboardShortcutsTab />');

    expect(voiceIndex).toBeGreaterThan(-1);
    expect(shortcutsIndex).toBeGreaterThan(voiceIndex);
    expect(source).not.toContain('SettingsViewPage');
    expect(source).not.toContain('<Tabs');
    expect(source).not.toContain('<TabPane');
  });

  it('uses whitespace instead of a divider between voice input and shortcuts', () => {
    const styles = readSource('./InputSettingsPage.scss');
    const shortcutsRule = styles.match(/&--shortcuts\s*{([^}]*)}/)?.[1] ?? '';

    expect(shortcutsRule).toContain('margin-top');
    expect(shortcutsRule).not.toContain('border');
  });

  it('opens compact local model management from the voice status instead of inline', () => {
    const voiceSource = readSource(
      '../../../../infrastructure/config/components/VoiceInputConfig.tsx',
    );
    const dialogSource = readSource(
      '../../../../infrastructure/config/components/LocalVoiceModelsConfig.tsx',
    );

    expect(voiceSource).toContain('setLocalModelsOpen(true)');
    expect(voiceSource).toContain('isOpen={localModelsOpen}');
    expect(voiceSource).not.toContain('local-models-anchor');
    expect(dialogSource).toContain('<Dialog');
    expect(dialogSource).not.toContain('<ConfigPageSection');
    expect(dialogSource).not.toContain('{model.provider}');
    expect(dialogSource).not.toContain('{model.version}');
    expect(dialogSource).not.toContain('{model.description}');
  });
});
