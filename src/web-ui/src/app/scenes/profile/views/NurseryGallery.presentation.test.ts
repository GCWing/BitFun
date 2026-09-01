import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSibling(filename: string): string {
  return readFileSync(
    fileURLToPath(new URL(filename, import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('Nursery gallery presentation', () => {
  it('uses the curated assistant artwork without loading the unused source pack', () => {
    const source = readSibling('./NurseryGallery.tsx');

    expect(source).toContain('src="/assets/assistant/defaults-illustration.webp"');
    expect(source).toContain('src="/assets/assistant/gallery-companion.webp"');
    expect(source).not.toContain('/panda_1.png');
    expect(source).not.toContain('/panda_wink.png');
  });

  it('keeps the gallery surface white and collapses the decorative column responsively', () => {
    const stylesheet = readSibling('./NurseryView.scss');

    expect(stylesheet).toMatch(
      /\.nursery-gallery \{\s+background: var\(--bf-color-content-on-dark\);/,
    );
    expect(stylesheet).toContain('.nursery-gallery__assistant-showcase--with-companion');
    expect(stylesheet).toContain('grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));');
    expect(stylesheet).toMatch(
      /@media \(max-width: 1100px\)[\s\S]*\.nursery-gallery__companion \{\s+display: none;/,
    );
  });

  it('uses a compact layered pill for the default configuration action', () => {
    const source = readSibling('./NurseryGallery.tsx');
    const stylesheet = readSibling('./NurseryView.scss');
    const actionMarkupEnd = source.indexOf('className="nursery-defaults__action"');
    const actionMarkupStart = source.lastIndexOf('<button', actionMarkupEnd);
    const actionMarkup = source.slice(actionMarkupStart, source.indexOf('</button>', actionMarkupEnd));
    const actionStart = stylesheet.indexOf('&__action {');
    const actionEnd = stylesheet.indexOf('\n  }\n}', actionStart);
    const actionSection = stylesheet.slice(actionStart, actionEnd);

    expect(source).toContain('className="nursery-defaults__action-icon"');
    expect(source).toContain('className="nursery-defaults__action-label"');
    expect(source).toContain('className="nursery-defaults__action-chevron"');
    expect(actionMarkup).toContain('<button');
    expect(actionMarkup).not.toContain('data-bf-component');
    expect(actionMarkup).not.toContain('variant=');
    expect(actionSection).toContain('width: 168px;');
    expect(actionSection).toContain('height: 48px;');
    expect(actionSection).toContain('border: 0;');
    expect(actionSection).toContain('border-radius: var(--bf-radius-pill);');
    expect(actionSection).toContain('width: 44px;');
    expect(actionSection).toContain('background-image: radial-gradient(');
    expect(actionSection).toContain('mask-image: radial-gradient(');
  });

  it('keeps assistant card content and actions in bounded regions', () => {
    const source = readSibling('./AssistantCard.tsx');
    const stylesheet = readSibling('./NurseryView.scss');
    const cardStart = stylesheet.indexOf('.assistant-card {');
    const cardEnd = stylesheet.indexOf('// ── Sub-page chrome', cardStart);
    const cardSection = stylesheet.slice(cardStart, cardEnd);

    expect(cardSection).toContain('&__main {');
    expect(cardSection).toContain('min-height: 168px;');
    expect(cardSection).toContain('padding: var(--bf-space-3) 14px;');
    expect(cardSection).toContain('min-height: 52px;');
    expect(cardSection).toContain('&__session-actions {');
    expect(cardSection).toContain('border-top: 1px solid color-mix(in srgb, var(--bf-color-content-on-light) 12%, transparent);');
    expect(cardSection).not.toContain('min-height: clamp(310px, 23.8vw, 366px);');
    expect(cardSection).not.toContain('height: 100%;');
    expect(cardSection).not.toContain('--assistant-card-action-bg');
    expect(cardSection).not.toContain('&__new-session-btn {');
    expect(cardSection).not.toContain('&__set-primary-btn {');
    expect(cardSection).not.toContain('&__delete-btn {');
    expect(source).toMatch(/import \{[^}]*\bButton\b[^}]*} from '@bitfun\/ui';/);
    expect(source).toContain('leadingIcon={<Icon name="settings"');
    expect(source).toContain('trailingIcon={<Icon name="chevron-right"');
    expect(source).toContain('leadingIcon={<MessageSquarePlus />}');
    expect(source).toContain('className="assistant-card__configure"');
    expect(source).toContain('className="assistant-card__session-actions"');
    expect(source).not.toContain('className="assistant-card__body"');
  });
});
