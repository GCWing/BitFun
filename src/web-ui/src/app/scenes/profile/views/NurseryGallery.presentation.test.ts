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
  it('does not render decorative assistant artwork', () => {
    const source = readSibling('./NurseryGallery.tsx');

    expect(source).not.toContain('defaults-illustration.webp');
    expect(source).not.toContain('gallery-companion.webp');
    expect(source).not.toContain('nursery-defaults__artwork');
    expect(source).not.toContain('nursery-gallery__companion');
  });

  it('uses theme-aware surface and content tokens throughout the gallery', () => {
    const stylesheet = readSibling('./NurseryView.scss');
    const galleryStart = stylesheet.indexOf('.nursery-gallery {');
    const galleryEnd = stylesheet.indexOf('// ── Sub-page chrome', galleryStart);
    const gallerySection = stylesheet.slice(galleryStart, galleryEnd);

    expect(gallerySection).toMatch(
      /\.nursery-gallery \{\s+background: var\(--openbitfun-color-surface-scene\);/,
    );
    expect(gallerySection).toContain('background: var(--openbitfun-color-surface-tertiary);');
    expect(gallerySection).toContain('color: var(--openbitfun-color-content-primary);');
    expect(gallerySection).toContain('color: var(--openbitfun-color-content-muted);');
    expect(gallerySection).toContain('border-radius: var(--openbitfun-layout-field-group-radius);');
    expect(gallerySection).not.toContain('--openbitfun-color-content-on-dark');
    expect(gallerySection).not.toContain('--openbitfun-color-content-on-light');
    expect(gallerySection).not.toContain('--openbitfun-color-overlay-scrim');
  });

  it('uses the shared button for default configuration', () => {
    const source = readSibling('./NurseryGallery.tsx');
    const end = source.indexOf('className="nursery-defaults__action"');
    const action = source.slice(source.lastIndexOf('<Button', end), source.indexOf('</Button>', end));
    expect(action).toContain('variant="outline"');
    expect(action).toContain('size="sm"');
    expect(action).toContain('onClick={openDefaults}');
  });

  it('keeps assistant card content and actions in bounded regions', () => {
    const source = readSibling('./AssistantCard.tsx');
    const stylesheet = readSibling('./NurseryView.scss');
    const cardStart = stylesheet.indexOf('.assistant-card {');
    const cardEnd = stylesheet.indexOf('// ── Sub-page chrome', cardStart);
    const cardSection = stylesheet.slice(cardStart, cardEnd);

    expect(cardSection).toContain('&__main {');
    expect(cardSection).toContain('min-height: 148px;');
    expect(cardSection).toContain('padding: var(--openbitfun-space-4) var(--openbitfun-space-5);');
    expect(cardSection).toContain('min-height: 52px;');
    expect(cardSection).toContain('&__session-actions {');
    expect(cardSection).toContain('border-top: 1px solid var(--openbitfun-color-border-subtle);');
    expect(cardSection).not.toContain('min-height: clamp(310px, 23.8vw, 366px);');
    expect(cardSection).not.toContain('height: 100%;');
    expect(cardSection).not.toContain('--assistant-card-action-bg');
    expect(cardSection).not.toContain('&__new-session-btn {');
    expect(cardSection).not.toContain('&__set-primary-btn {');
    expect(cardSection).not.toContain('&__delete-btn {');
    expect(source).toMatch(/import \{[^}]*\bButton\b[^}]*} from '@openbitfun\/ui';/);
    expect(source).toContain('leadingIcon={<Icon name="settings"');
    expect(source).toContain('trailingIcon={<Icon name="chevron-right"');
    expect(source).toContain('leadingIcon={<Icon name="side-chat" size="sm" />}');
    expect(source).toContain('className="assistant-card__configure"');
    expect(source).toContain('className="assistant-card__session-actions"');
    expect(source).not.toContain('className="assistant-card__body"');
  });
});
