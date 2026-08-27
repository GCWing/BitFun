import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    'utf8',
  );
}

describe('VoiceInputDiagnostics responsive layout', () => {
  it('gives diagnostic controls enough room and delegates label layout to the shared button', () => {
    const source = readSource('./VoiceInputDiagnostics.tsx');
    const styles = readSource('./VoiceInputConfig.scss');
    const rowRule = styles.match(/&__diagnostic-row\s*{([^}]*)}/)?.[1] ?? '';
    const buttonRule = styles.match(/&__diagnostic-button\s*{([^}]*)}/)?.[1] ?? '';

    expect(source.match(/className="voice-input-config__diagnostic-row"/g)).toHaveLength(2);
    expect(source).toMatch(/import \{[^}]*\bButton\b[^}]*} from '@bitfun\/ui';/);
    expect(source).toContain('className="voice-input-config__diagnostic-button"');
    expect(rowRule).toContain('--row-grid-cols: minmax(0, 2fr) minmax(0, 3fr)');
    expect(buttonRule).toContain('flex: 0 0 auto');
  });
});
