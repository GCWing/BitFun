import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const chatSource = readFileSync(
  new URL('../../../../../mobile-web/src/pages/ChatPage.tsx', import.meta.url),
  'utf8',
);
const modelControlsSource = readFileSync(
  new URL('../../../../../mobile-web/src/components/ChatModelControls.tsx', import.meta.url),
  'utf8',
);

describe('mobile chat settings interaction contracts', () => {
  it('dismisses the combined model panel before either remote selection write', () => {
    for (const [handlerName, remoteCallback] of [
      ['handleSelect', 'onSelect'],
      ['handleSelectReasoning', 'onSelectReasoning'],
    ]) {
      const handlerStart = modelControlsSource.indexOf(`const ${handlerName} =`);
      expect(handlerStart).toBeGreaterThanOrEqual(0);
      const handler = modelControlsSource.slice(
        handlerStart,
        modelControlsSource.indexOf('\n  };', handlerStart),
      );
      expect(handler.indexOf('setOpen(false);')).toBeGreaterThanOrEqual(0);
      expect(handler.indexOf('setOpen(false);')).toBeLessThan(
        handler.indexOf(`void ${remoteCallback}(`),
      );
    }

    expect(modelControlsSource).toMatch(
      /<ReasoningPresetOptions\b[^>]*onSelect=\{handleSelectReasoning\}/,
    );
  });

  it('optimistically updates a reasoning choice and rolls it back on failure', () => {
    const handlerStart = chatSource.indexOf('const handleSelectReasoningPreset');
    const handler = chatSource.slice(
      handlerStart,
      chatSource.indexOf('useEffect(() => {\n    if (!isStreaming)', handlerStart),
    );
    const optimisticUpdate = handler.indexOf(
      'session_reasoning_preset: reasoningPreset,',
    );
    const remoteWrite = handler.indexOf(
      'await sessionMgr.setSessionModelSelection',
    );

    expect(handler).toContain('const previousReasoningPreset =');
    expect(optimisticUpdate).toBeGreaterThanOrEqual(0);
    expect(optimisticUpdate).toBeLessThan(remoteWrite);
    expect(handler).toContain(
      'session_reasoning_preset: previousReasoningPreset,',
    );
  });
});
