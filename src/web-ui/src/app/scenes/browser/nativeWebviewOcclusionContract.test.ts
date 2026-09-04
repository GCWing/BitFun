import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { NATIVE_WEBVIEW_OCCLUSION_SELECTOR, rectanglesIntersect } from './useEmbeddedBrowserWebview';

function readSource(relativePath: string): string {
  return readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8')
    .replace(/\r\n?/g, '\n');
}

describe('native browser webview occlusion contract', () => {
  it('recognizes overlays that explicitly occlude native child webviews', () => {
    expect(NATIVE_WEBVIEW_OCCLUSION_SELECTOR).toContain('[data-openbitfun-native-webview-occlusion]');
  });

  it('marks the user-message image lightbox as a native-webview occluder', () => {
    const source = readSource('../../../flow_chat/components/modern/UserMessageItem.tsx');

    expect(source).toContain('data-openbitfun-native-webview-occlusion');
  });

  it('includes the context menu only when its bounds intersect the browser viewport', () => {
    expect(NATIVE_WEBVIEW_OCCLUSION_SELECTOR).toContain('[data-openbitfun-product-component=\'context-menu\']');
    expect(rectanglesIntersect(
      { left: 0, top: 0, right: 100, bottom: 100 },
      { left: 80, top: 80, right: 120, bottom: 120 },
    )).toBe(true);
    expect(rectanglesIntersect(
      { left: 0, top: 0, right: 100, bottom: 100 },
      { left: 100, top: 0, right: 120, bottom: 20 },
    )).toBe(false);
  });
});
