import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    'utf8',
  );
}

describe('VoiceInputDiagnostics presentation', () => {
  it('refreshes microphones from the device dropdown without a separate refresh action', () => {
    const source = readSource('./VoiceInputDiagnostics.tsx');
    const deviceSelect = source.match(
      /<Select[\s\S]*?data-openbitfun-part="deviceSelect"[\s\S]*?\/>/,
    )?.[0] ?? '';

    expect(deviceSelect).toContain('onPointerDown={() => void loadMicrophones()}');
    expect(source).not.toContain('IconButton');
    expect(source).not.toContain('Tooltip');
    expect(source).not.toContain('diagnostics.microphone.refresh');
    expect(source).not.toContain('devicesLoading');
  });

  it('shows the live waveform only while recording and places feedback below the row', () => {
    const source = readSource('./VoiceInputDiagnostics.tsx');
    const styles = readSource('./VoiceInputConfig.scss');
    const balancedRowRule = styles.match(/&__balanced-row\s*{([^}]*)}/)?.[1] ?? '';
    const buttonRule = styles.match(/&__diagnostic-button\s*{([^}]*)}/)?.[1] ?? '';
    const waveformRule = styles.match(/&__waveform\s*{([^}]*)}/)?.[1] ?? '';
    const waveformBarRule = styles.match(/&__waveform-bar\s*{([^}]*)}/)?.[1] ?? '';
    const feedbackRule = styles.match(/&__recognition-feedback\s*{([^}]*)}/)?.[1] ?? '';
    const resultRule = styles.match(/&__recognition-result\s*{([^}]*)}/)?.[1] ?? '';
    const recognitionRow = source.match(
      /<ConfigPageRow\s+label=\{t\('diagnostics\.recognition\.label'\)\}[\s\S]*?<\/ConfigPageRow>/,
    )?.[0] ?? '';
    const microphoneRow = source.match(
      /<ConfigPageRow[\s\S]*?label=\{t\('diagnostics\.microphone\.label'\)\}[\s\S]*?<\/ConfigPageRow>/,
    )?.[0] ?? '';

    expect(source.match(/className="voice-input-config__balanced-row"/g)).toHaveLength(1);
    expect(microphoneRow).not.toContain('voice-input-config__balanced-row');
    expect(source).toMatch(/import \{[^}]*\bButton\b[^}]*} from '@openbitfun\/ui';/);
    expect(source).toContain('className="voice-input-config__diagnostic-button"');
    expect(source).not.toContain('Activity');
    expect(source).not.toContain('voice-input-config__level');
    expect(source).not.toContain('recognitionUnavailableReason');
    expect(source).not.toContain('voice-input-config__diagnostic-note');
    expect(recognitionRow).toContain("phase === 'recording' ? (");
    expect(recognitionRow).toContain('className="voice-input-config__waveform"');
    expect(recognitionRow).not.toContain('voice-input-config__recognition-result');
    expect(source).toMatch(
      /<\/ConfigPageRow>\s*<div\s+className="voice-input-config__recognition-feedback"/,
    );
    expect(source).toContain("t('diagnostics.recognition.preparing')");
    expect(source).toContain("t('diagnostics.recognition.transcribing')");
    expect(balancedRowRule).toContain('--row-grid-cols: minmax(0, 2fr) minmax(0, 3fr)');
    expect(buttonRule).toContain('flex: 0 0 auto');
    expect(waveformRule).toContain('color: var(--openbitfun-color-accent-default)');
    expect(waveformBarRule).toContain('transition: transform 80ms linear');
    expect(feedbackRule).toContain('padding: 0 var(--openbitfun-space-5) var(--openbitfun-space-4)');
    expect(feedbackRule).toContain('text-align: start');
    expect(resultRule).toContain('font-family: var(--openbitfun-type-body-sm-font-family)');
    expect(resultRule).toContain('font-weight: var(--openbitfun-type-body-sm-font-weight)');
    expect(resultRule).not.toContain('border-left');
    expect(styles).toMatch(/prefers-reduced-motion:[\s\S]*?&?__waveform-bar|prefers-reduced-motion:[\s\S]*?\.voice-input-config__waveform-bar/);
  });
});
