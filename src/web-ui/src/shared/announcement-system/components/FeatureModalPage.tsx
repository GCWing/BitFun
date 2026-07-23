import React from 'react';
import type { ModalPage } from '../types';
import MediaRenderer from './MediaRenderer';
import { useAnnouncementI18n } from '../hooks/useAnnouncementI18n';

interface Props {
  page: ModalPage;
  active: boolean;
}

/**
 * Renders a single page inside the FeatureModal.
 *
 * Handles i18n key resolution and Markdown body rendering (simple HTML via
 * dangerouslySetInnerHTML – body content is authored internally so this is safe).
 */
const FeatureModalPage: React.FC<Props> = ({ page, active }) => {
  const { t } = useAnnouncementI18n();

  const resolve = (key: string) => (key.startsWith('announcements.') ? t(key) : key);

  /**
   * Lightweight Markdown → HTML for announcement body content.
   *
   * Supported syntax:
   *   - **bold**
   *   - `inline code`
   *   - Pipe tables (|---|)
   *   - Blank-line paragraph breaks
   *   - Single line-break → <br> within a paragraph
   */
  function renderBody(raw: string): string {
    const resolved = resolve(raw);

    // Split into blocks on double newline.
    const blocks = resolved.split(/\n{2,}/);

    const htmlBlocks = blocks.map((block) => {
      const trimmed = block.trim();

      // ── Table detection: block must have at least 3 lines where one is a separator row ──
      const lines = trimmed.split('\n');
      const isSeparatorRow = (l: string) => /^\|[\s|:-]+\|$/.test(l.trim());
      if (lines.length >= 2 && lines.some(isSeparatorRow)) {
        const rows = lines.filter((l) => !isSeparatorRow(l));
        const toTd = (l: string, tag: 'th' | 'td') =>
          l
            .split('|')
            .filter((_, i, a) => i > 0 && i < a.length - 1) // drop leading/trailing empty
            .map((cell) => `<${tag}>${inlineMarkdown(cell.trim())}</${tag}>`)
            .join('');

        const [header, ...body] = rows;
        return [
          '<table class="md-table">',
          `<thead><tr>${toTd(header, 'th')}</tr></thead>`,
          `<tbody>${body.map((r) => `<tr>${toTd(r, 'td')}</tr>`).join('')}</tbody>`,
          '</table>',
        ].join('');
      }

      // ── Regular paragraph ──
      const inner = lines
        .map((l) => inlineMarkdown(l))
        .join('<br>');
      return `<p>${inner}</p>`;
    });

    return htmlBlocks.join('\n');
  }

  /** Apply inline Markdown (bold, code) to a single line. */
  function inlineMarkdown(line: string): string {
    return line
      .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
      .replace(/`([^`]+)`/g, '<code>$1</code>');
  }

  const layoutClass = `feature-modal-page--${page.layout}`;

  return (
    <div className={`feature-modal-page ${layoutClass}`}>
      {page.media && (
        <div className="feature-modal-page__media">
          <MediaRenderer media={page.media} active={active} />
        </div>
      )}
      <div className="feature-modal-page__text">
        <div className="feature-modal-page__eyebrow" aria-hidden />
        <h2 className="feature-modal-page__title">{resolve(page.title)}</h2>
        <div className="feature-modal-page__rule" aria-hidden />
        <div
          className="feature-modal-page__body"
          dangerouslySetInnerHTML={{ __html: renderBody(page.body) }}
        />
      </div>
    </div>
  );
};

export default FeatureModalPage;
