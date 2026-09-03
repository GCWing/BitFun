import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    'utf8',
  ).replace(/\r\n?/g, '\n');
}

describe('FlowChat transcript edge fade', () => {
  it('positions a viewport alpha mask from the live ChatInput top edge', () => {
    const component = readSource('./VirtualMessageList.tsx');
    const stylesheet = readSource('./VirtualMessageList.scss');

    expect(component).toContain(
      'const inputOverlayInsetPx = computeFlowChatInputOverlayInsetPx(inputHeight);',
    );
    expect(component).toContain(
      "'--_flow-chat-input-overlay-inset': `${inputOverlayInsetPx}px`",
    );
    expect(stylesheet).toContain('-webkit-mask-image: linear-gradient(');
    expect(stylesheet).toContain('mask-image: linear-gradient(');
    expect(stylesheet).toContain(
      '100% - var(--_flow-chat-input-overlay-inset) - var(--bf-space-12)',
    );
    expect(stylesheet).toContain(
      'transparent calc(100% - var(--_flow-chat-input-overlay-inset))',
    );
    expect(stylesheet).not.toContain('backdrop-filter');
  });

  it('removes the mask in forced-colors mode', () => {
    const stylesheet = readSource('./VirtualMessageList.scss');
    const forcedColors = stylesheet.slice(stylesheet.indexOf('@media (forced-colors: active)'));

    expect(forcedColors).toContain('.virtual-message-list__scroller');
    expect(forcedColors).toContain('-webkit-mask-image: none;');
    expect(forcedColors).toContain('mask-image: none;');
  });
});
