import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { NATIVE_WEBVIEW_OCCLUSION_SELECTOR } from './useEmbeddedBrowserWebview';

function readSource(relativePath: string): string {
  return readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8')
    .replace(/\r\n?/g, '\n');
}

describe('native browser webview occlusion contract', () => {
  it('recognizes overlays that explicitly occlude native child webviews', () => {
    expect(NATIVE_WEBVIEW_OCCLUSION_SELECTOR).toContain('[data-bf-native-webview-occlusion]');
  });

  it('marks the user-message image lightbox as a native-webview occluder', () => {
    const source = readSource('../../../flow_chat/components/modern/UserMessageItem.tsx');

    expect(source).toContain('data-bf-native-webview-occlusion');
  });
});
