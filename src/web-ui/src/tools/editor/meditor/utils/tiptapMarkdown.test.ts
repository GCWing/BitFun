import { describe, expect, it } from 'vitest';
import {
  analyzeMarkdownEditability,
  canRoundTripMarkdownWithTiptap,
  getUnsupportedTiptapMarkdownFeatures,
  markdownToTiptapDoc,
  tiptapDocToMarkdown,
  tiptapDocToTopLevelMarkdownBlocks,
} from './tiptapMarkdown';

describe('tiptap markdown compatibility', () => {
  it('supports gfm tables without falling back', () => {
    const markdown = [
      '| name | value |',
      '| --- | --- |',
      '| foo | bar |',
    ].join('\n');

    const doc = markdownToTiptapDoc(markdown);

    expect(canRoundTripMarkdownWithTiptap(markdown)).toBe(true);
    expect(getUnsupportedTiptapMarkdownFeatures(markdown)).toEqual([]);
    expect(tiptapDocToMarkdown(doc)).toBe(markdown);
    expect(analyzeMarkdownEditability(markdown).containsRawHtmlBlocks).toBe(false);
  });

  it('supports escaped markdown literals without losing semantics', () => {
    const markdown = String.raw`Show literal \*asterisks\* here.`;

    const doc = markdownToTiptapDoc(markdown);

    expect(canRoundTripMarkdownWithTiptap(markdown)).toBe(true);
    expect(getUnsupportedTiptapMarkdownFeatures(markdown)).toEqual([]);
    expect(tiptapDocToMarkdown(doc)).toBe(markdown);
  });

  it('preserves a preferred trailing newline during serialization', () => {
    const markdown = '# Title\n\nBody\n';
    const doc = markdownToTiptapDoc(markdown);

    expect(tiptapDocToMarkdown(doc, { preserveTrailingNewline: true })).toBe(markdown);
    expect(tiptapDocToMarkdown(doc)).toBe('# Title\n\nBody');
  });

  it('allows simple markdown documents to use the tiptap engine', () => {
    const markdown = [
      '# Title',
      '',
      '- item one',
      '- item two',
      '',
      'Regular paragraph.',
    ].join('\n');

    expect(canRoundTripMarkdownWithTiptap(markdown)).toBe(true);
    expect(getUnsupportedTiptapMarkdownFeatures(markdown)).toEqual([]);
  });

  it('parses gfm email autolinks without lookbehind-dependent behavior', () => {
    const markdown = 'Contact support@example.com or see /admin@example.com.';

    const doc = markdownToTiptapDoc(markdown);
    const paragraph = doc.content?.[0];
    const emailNode = paragraph?.content?.find(node =>
      node.marks?.some(mark =>
        mark.type === 'link' && mark.attrs?.href === 'mailto:support@example.com'
      )
    );
    const slashEmailNode = paragraph?.content?.find(node =>
      node.marks?.some(mark =>
        mark.type === 'link' && mark.attrs?.href === 'mailto:admin@example.com'
      )
    );

    expect(emailNode?.text).toBe('support@example.com');
    expect(slashEmailNode).toBeUndefined();
    expect(tiptapDocToMarkdown(doc)).toBe(
      'Contact [support@example.com](mailto:support@example.com) or see /admin@example.com.'
    );
  });

  it('supports deep heading levels', () => {
    const markdown = '#### Deep heading';

    const doc = markdownToTiptapDoc(markdown);

    expect(canRoundTripMarkdownWithTiptap(markdown)).toBe(true);
    expect(getUnsupportedTiptapMarkdownFeatures(markdown)).toEqual([]);
    expect(tiptapDocToMarkdown(doc)).toBe(markdown);
  });

  it('preserves inline code wrapped by strong emphasis', () => {
    const markdown = '可自行设置 `OPENSSL_DIR` 为 ZIP 内 **`x64`** 目录。';

    const doc = markdownToTiptapDoc(markdown);
    const serialized = tiptapDocToMarkdown(doc);
    const analysis = analyzeMarkdownEditability(markdown);

    expect(serialized).toBe(markdown);
    expect(analysis.mode).toBe('lossless');
    expect(analysis.semanticEqual).toBe(true);
  });

  it('supports aligned html wrapper sections used by the project README header', () => {
    const markdown = [
      '<div align="center">',
      '',
      '![OpenBitFun](./png/OpenBitFun_title.png)',
      '',
      '**AI assistant with personality and memory**',
      '',
      'Hand over the work, keep the life',
      '',
      '</div>',
      '<div align="center">',
      '',
      '[![Website](https://img.shields.io/badge/Website-openbitfun.com-6f42c1?style=flat-square)](https://openbitfun.com/)',
      '',
      '</div>',
    ].join('\n');

    const doc = markdownToTiptapDoc(markdown);
    const serialized = tiptapDocToMarkdown(doc);

    expect(serialized).toBe(markdown);
    expect(canRoundTripMarkdownWithTiptap(markdown)).toBe(true);
    expect(getUnsupportedTiptapMarkdownFeatures(markdown)).toEqual([]);
  });

  it('keeps canonicalizable nested lists in m-editor without requiring lossless round-trip', () => {
    const markdown = [
      '- parent',
      '  - child',
      '',
      'Register in `agentic/tools/registry.rs`:',
      '1. Implement `Tool` trait',
      '2. Define input/output types',
    ].join('\n');

    const analysis = analyzeMarkdownEditability(markdown);

    expect(analysis.mode).toBe('canonicalizable');
    expect(analysis.semanticEqual).toBe(true);
    expect(analysis.textEqual).toBe(false);
    expect(analysis.hardIssues).toEqual([]);
    expect(canRoundTripMarkdownWithTiptap(markdown)).toBe(false);
    expect(getUnsupportedTiptapMarkdownFeatures(markdown)).toContain('roundTripMismatch');
  });

  it('preserves frontmatter as an editable IR block', () => {
    const markdown = [
      '---',
      'name: Demo',
      'creature: Cat',
      'vibe: Calm',
      'emoji: 🙂',
      '---',
      '',
      '# Body',
    ].join('\n');

    const analysis = analyzeMarkdownEditability(markdown);

    const doc = markdownToTiptapDoc(markdown);

    expect(analysis.mode).toBe('lossless');
    expect(analysis.hardIssues).toEqual([]);
    expect(analysis.softIssues).toEqual([]);
    expect(analysis.canonicalMarkdown).toBe(markdown);
    expect(doc.content?.[0]).toMatchObject({
      type: 'frontmatter',
      attrs: {
        yaml: expect.stringContaining('name: Demo'),
        delimiter: '---',
        bodyPrefix: '\n',
      },
    });
    expect(tiptapDocToMarkdown(doc)).toBe(markdown);
    expect(tiptapDocToTopLevelMarkdownBlocks(doc)).toEqual([
      expect.objectContaining({ markdown: '# Body' }),
    ]);
  });

  it('preserves TOML-style frontmatter delimiters and CRLF inside the envelope', () => {
    const markdown = '+++\r\ntitle = "Demo"\r\n+++\r\n\r\n# Body\r\n';
    const doc = markdownToTiptapDoc(markdown);

    expect(doc.content?.[0]?.type).toBe('frontmatter');
    expect(doc.content?.[0]?.attrs).toMatchObject({
      yaml: 'title = "Demo"',
      delimiter: '+++',
      openingLineEnding: '\r\n',
      contentLineEnding: '\r\n',
      closingLineEnding: '\r\n',
      bodyPrefix: '\r\n',
    });
    expect(tiptapDocToMarkdown(doc, { preserveTrailingNewline: true })).toBe(
      '+++\r\ntitle = "Demo"\r\n+++\r\n\r\n# Body\n',
    );
  });

  it('rebuilds structural delimiters around edited frontmatter content', () => {
    const doc = markdownToTiptapDoc('---\nname: Demo\n---\n\n# Body');
    const frontmatter = doc.content?.[0];

    expect(frontmatter?.attrs).toBeDefined();
    frontmatter!.attrs!.yaml = 'name: Updated';

    expect(tiptapDocToMarkdown(doc)).toBe('---\nname: Updated\n---\n\n# Body');
  });

  it('upgrades simple details regions into structured details nodes', () => {
    const markdown = [
      '<details>',
      '<summary>Open me</summary>',
      '',
      'Body',
      '',
      '</details>',
    ].join('\n');

    const analysis = analyzeMarkdownEditability(markdown);
    const doc = markdownToTiptapDoc(markdown);

    expect(tiptapDocToMarkdown(doc)).toBe(markdown);
    expect(analysis.mode).toBe('lossless');
    expect(analysis.containsRenderOnlyBlocks).toBe(false);
    expect(analysis.containsRawHtmlBlocks).toBe(false);
    expect(analysis.hardIssues).toEqual([]);
    expect(doc.content?.[0]?.type).toBe('details');
  });

  it('preserves blockquotes nested inside ordered list items', () => {
    const markdown = [
      '1. Contribute good ideas/creativity (features, interactions, visuals, etc.) by opening issues',
      '   > Product managers and UI designers are welcome to submit ideas quickly via PI. We will help refine them for development.',
      '2. Improve the Agent system and overall quality',
    ].join('\n');

    const doc = markdownToTiptapDoc(markdown);
    const serialized = tiptapDocToMarkdown(doc);
    const analysis = analyzeMarkdownEditability(markdown);

    expect(serialized).toBe(markdown);
    expect(analysis.mode).toBe('lossless');
    expect(analysis.semanticEqual).toBe(true);
  });

  it('preserves nested bullet lists inside ordered list items', () => {
    const markdown = [
      '1. Parent item',
      '   - child one',
      '   - child two',
      '2. Sibling item',
    ].join('\n');

    const doc = markdownToTiptapDoc(markdown);
    const serialized = tiptapDocToMarkdown(doc);
    const analysis = analyzeMarkdownEditability(markdown);

    expect(serialized).toBe(markdown);
    expect(analysis.mode).toBe('lossless');
    expect(analysis.semanticEqual).toBe(true);
  });

  it('preserves html-rich markdown mixed with surrounding markdown blocks', () => {
    const markdown = [
      '# Intro',
      '',
      'Before the HTML block.',
      '',
      '<details>',
      '<summary>Expand</summary>',
      '',
      'Protected **markdown** body',
      '',
      '</details>',
      '',
      'After the HTML block.',
    ].join('\n');

    const doc = markdownToTiptapDoc(markdown);
    const serialized = tiptapDocToMarkdown(doc);
    const analysis = analyzeMarkdownEditability(markdown);

    expect(serialized).toBe(markdown);
    expect(analysis.mode).toBe('lossless');
    expect(analysis.containsRawHtmlBlocks).toBe(false);
  });

  it('preserves inline raw html fragments inside markdown paragraphs', () => {
    const markdown = 'Mix <span data-x="1">inline</span> HTML into markdown.';

    const doc = markdownToTiptapDoc(markdown);
    const serialized = tiptapDocToMarkdown(doc);
    const analysis = analyzeMarkdownEditability(markdown);

    expect(serialized).toBe(markdown);
    expect(analysis.mode).toBe('lossless');
    expect(analysis.containsRawHtmlInlines).toBe(true);
    expect(analysis.containsRawHtmlBlocks).toBe(false);
  });

  it('canonicalizes supported inline html tags into standard markdown syntax', () => {
    const markdown = 'Mix <strong>bold</strong>, <em>italics</em>, <code>code</code>, <a href="https://example.com">link</a><br><img src="/x.png" alt="x">.';

    const doc = markdownToTiptapDoc(markdown);
    const serialized = tiptapDocToMarkdown(doc);
    const analysis = analyzeMarkdownEditability(markdown);

    expect(serialized).toBe('Mix **bold**, *italics*, `code`, [link](https://example.com)  \n![x](/x.png).');
    expect(analysis.mode).toBe('canonicalizable');
    expect(analysis.containsRawHtmlBlocks).toBe(false);
    expect(analysis.containsRawHtmlInlines).toBe(false);
    expect(analysis.semanticEqual).toBe(true);
  });

  it('canonicalizes p align html blocks into aligned markdown content', () => {
    const markdown = '<p align="center">Hello <strong>world</strong><br><img src="/x.png" alt="x"></p>';

    const doc = markdownToTiptapDoc(markdown);
    const serialized = tiptapDocToMarkdown(doc);
    const analysis = analyzeMarkdownEditability(markdown);

    expect(serialized).toBe('<div align="center">\n\nHello **world**  \n![x](/x.png)\n\n</div>');
    expect(analysis.mode).toBe('canonicalizable');
    expect(analysis.containsRawHtmlBlocks).toBe(false);
    expect(analysis.containsRawHtmlInlines).toBe(false);
    expect(analysis.semanticEqual).toBe(true);
  });

  it('upgrades rich but safe details summaries into editable details nodes', () => {
    const markdown = [
      '<details open>',
      '<summary><strong>Open me</strong></summary>',
      '',
      'Body',
      '',
      '</details>',
    ].join('\n');

    const doc = markdownToTiptapDoc(markdown);
    const serialized = tiptapDocToMarkdown(doc);
    const analysis = analyzeMarkdownEditability(markdown);

    expect(serialized).toBe(markdown);
    expect(analysis.mode).toBe('lossless');
    expect(analysis.containsRenderOnlyBlocks).toBe(false);
    expect(analysis.containsRawHtmlBlocks).toBe(false);
    expect(doc.content?.[0]?.type).toBe('details');
  });

  it('upgrades linked details summaries into editable details nodes', () => {
    const markdown = [
      '<details>',
      '<summary>You can also go to the <a href="https://github.com/openai/codex/releases/latest">latest GitHub Release</a> and download the appropriate binary for your platform.</summary>',
      '',
      'Each GitHub Release contains many executables, but in practice, you likely want one of these:',
      '',
      '- macOS',
      '  - Apple Silicon/arm64: `codex-aarch64-apple-darwin.tar.gz`',
      '  - x86_64 (older Mac hardware): `codex-x86_64-apple-darwin.tar.gz`',
      '- Linux',
      '  - x86_64: `codex-x86_64-unknown-linux-musl.tar.gz`',
      '  - arm64: `codex-aarch64-unknown-linux-musl.tar.gz`',
      '',
      'Each archive contains a single entry with the platform baked into the name (e.g., `codex-x86_64-unknown-linux-musl`), so you likely want to rename it to `codex` after extracting it.',
      '',
      '</details>',
    ].join('\n');

    const doc = markdownToTiptapDoc(markdown);
    const serialized = tiptapDocToMarkdown(doc);
    const analysis = analyzeMarkdownEditability(markdown);

    expect(serialized).toBe(markdown);
    expect(analysis.mode).toBe('lossless');
    expect(analysis.containsRenderOnlyBlocks).toBe(false);
    expect(analysis.containsRawHtmlBlocks).toBe(false);
    expect(doc.content?.[0]?.type).toBe('details');
  });

  it('keeps unsafe details content as source-only raw html', () => {
    const markdown = [
      '<details>',
      '<summary><a href="javascript:alert(1)">Open me</a></summary>',
      '',
      'Body',
      '',
      '</details>',
    ].join('\n');

    const doc = markdownToTiptapDoc(markdown);
    const serialized = tiptapDocToMarkdown(doc);
    const analysis = analyzeMarkdownEditability(markdown);

    expect(serialized).toBe(markdown);
    expect(analysis.mode).toBe('lossless');
    expect(analysis.containsRawHtmlBlocks).toBe(true);
    expect(doc.content?.[0]?.type).toBe('rawHtmlBlock');
  });
});
