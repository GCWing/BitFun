import { describe, expect, it } from 'vitest';
import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';
import remarkRehype from 'remark-rehype';
import rehypeRaw from 'rehype-raw';
import type { RootContent, PhrasingContent } from 'mdast';
import { analyzeMarkdownEditability, markdownToEditableTiptapDoc, markdownToTiptapDoc, tiptapDocToMarkdown } from './tiptapMarkdown';

// Independent source-AST oracle: compare visible text, mark ranges, destinations,
// titles and block structure, not just the editor DTO (which can omit source data).
const parser = unified().use(remarkParse).use(remarkGfm).use(remarkMath);
function inlineFacts(nodes: PhrasingContent[], marks: string[] = []): unknown[] {
  return nodes.flatMap((node): unknown[] => {
    if (node.type === 'strong' || node.type === 'emphasis' || node.type === 'delete') {
      return inlineFacts(node.children, [...marks, node.type]);
    }
    if (node.type === 'link') {
      return inlineFacts(node.children, [...marks, JSON.stringify(['link', node.url, node.title ?? null])]);
    }
    if (node.type === 'text' || node.type === 'inlineCode') {
      const tags = [...marks, ...(node.type === 'inlineCode' ? ['code'] : [])].sort();
      return Array.from(node.value, character => [character, tags]);
    }
    const { position: _position, ...fact } = node;
    return [{ ...fact, marks: [...marks].sort() }];
  });
}
function blockFacts(node: RootContent): unknown {
  if (node.type === 'paragraph' || node.type === 'heading' || node.type === 'tableCell') {
    return { type: node.type, ...('depth' in node ? { depth: node.depth } : {}), content: inlineFacts(node.children) };
  }
  const { position: _position, ...fact } = node;
  if ('children' in node) {
    const { children: _children, ...attrs } = fact as typeof fact & { children: unknown };
    delete (attrs as Record<string, unknown>).spread;
    return { ...attrs, children: node.children.map(child => blockFacts(child as RootContent)) };
  }
  return fact;
}
function semantics(markdown: string) {
  return parser.parse(markdown).children.map(blockFacts);
}

const samples = [
  '~~**Label**: remaining text~~', '**bold *italic* end**', '*italic **bold** end*',
  '~~**bold** and *italic*~~', '**a** **b**', '*a* *b*', '**a****b**', '~~a~~~~b~~',
  '[**bold** normal](https://example.com)', '**[label](https://example.com)**',
  '[a *b* c](https://example.com)', '[label](https://example.com "Title")',
  '[label](<https://example.com/a b>)', '[label](https://example.com/a\\)b)',
  '[label](https://example.com/a(b)c)', '[label](<file:///a b.md> "a \\"quote\\" & title")',
  '[label](./a\\(b.md "")', '[label](./a.md \'single title\')',
  '``  padded  ``', '`` `tick` ``', '`plain`', '` `', '`  `', '` a`', '`a `',
  '``a`b``', '```a``b```', '**`code`**', '~~`code`~~',
  '**bold**中文', '中文**加粗**内容', '*italic*中文', '~~deleted~~中文',
  '\\# Heading literal', '\\> quote literal', '\\- item literal', '\\+ item literal',
  '1\\. item literal', '12\\) item literal', '\\~literal\\~', '\\~~literal\\~~',
  '&lt;span&gt;', '&amp;copy;', '&#35; literal', '&#x3e; literal',
  '\\$x\\$', 'x\\*y', 'path\\\\file', 'ordinary\n\\# literal',
  'ordinary\n\\---', 'ordinary\n\\===', 'line  \nbreak',
  '![alt](image.png "Title")', '![a \\[b\\]](<a b.png> "a & title")',
  '**one\nline**', '**one  \nline**', '[one  \ntwo](a.md "title")',
  'a_b_c', 'a\\@example.com', 'https\\://example.com',
  '| a | b |\n| --- | --- |\n| text \\| pipe | `a\\|b` |',
];
const contexts: Array<[string, (s: string) => string]> = [
  ['paragraph', s => s], ['blockquote', s => s.split('\n').map(l => '> '+l).join('\n')],
  ['list', s => '- '+s.replace(/\n/g, '\n  ')],
  ['ordered list', s => '3. '+s.replace(/\n/g, '\n   ')],
];

describe('Markdown source semantic preservation', () => {
  for (const [name, wrap] of contexts) {
    it.each(samples)(`${name}: %s`, sample => {
      const markdown = '# Before\n\n'+wrap(sample)+'\n\nAfter';
      const doc = markdownToEditableTiptapDoc(markdown);
      expect(semantics(tiptapDocToMarkdown(doc))).toEqual(semantics(markdown));
      expect(doc.content?.[0].type).toBe('heading');
      expect(doc.content?.at(-1)?.type).toBe('paragraph');
    });
  }
  it.each(samples.filter(s => !s.startsWith('|')) )('keeps supported inline syntax native: %s', markdown => {
    expect(analyzeMarkdownEditability(markdown).semanticEqual).toBe(true);
    expect(markdownToEditableTiptapDoc(markdown)).toMatchObject({ content: [{ type: 'paragraph' }] });
  });
  it('retains a link title in the imported document and after an unrelated edit', () => {
    const doc = markdownToTiptapDoc('[label](./a.md "Title")\n\nAfter');
    expect(doc.content?.[0].content?.[0].marks).toContainEqual({ type: 'link', attrs: { href: './a.md', title: 'Title' } });
    doc.content![1].content![0].text = 'Updated';
    expect(semantics(tiptapDocToMarkdown(doc))).toEqual(semantics('[label](./a.md "Title")\n\nUpdated'));
  });
});

const htmlParser = unified().use(remarkParse).use(remarkGfm)
  .use(remarkRehype, { allowDangerousHtml: true }).use(rehypeRaw);
function renderedInlineFacts(markdown: string): unknown[] {
  const tree = htmlParser.runSync(htmlParser.parse(markdown));
  const paragraphs = tree.children.filter(node => node.type === 'element');
  expect(paragraphs.map(node => node.type === 'element' ? node.tagName : '')).toEqual(['p']);
  const walk = (node: (typeof tree.children)[number], marks: string[] = []): unknown[] => {
    if (node.type === 'text') return Array.from(node.value, character => [character, [...marks].sort()]);
    if (node.type !== 'element') return [];
    const tag = ({ strong: 'strong', em: 'emphasis', del: 'delete', code: 'code', u: 'underline' } as Record<string, string>)[node.tagName];
    return node.children.flatMap(child => walk(child, tag ? [...marks, tag] : marks));
  };
  return paragraphs.flatMap(node => walk(node));
}

const formattingNames = ['bold', 'italic', 'strike', 'code'];
const semanticNames = ['strong', 'emphasis', 'delete', 'code'];
function maskMarks(mask: number) {
  return formattingNames.flatMap((type, bit) => mask & (1 << bit) ? [{ type }] : []);
}
const triples = Array.from({ length: 512 }, (_, value) => [value & 7, (value >> 3) & 7, (value >> 6) & 7]);
describe('rich-text formatting combinations', () => {
  for (const words of [['a', 'b', 'c'], ['left ', ' middle ', ' right'], ['字', '*_~<&', '文']]) {
    it.each(triples)(`preserves ${JSON.stringify(words)} with masks %s/%s/%s`, (a, b, c) => {
      const masks = [a, b, c];
      const content = words.map((text, index) => ({ type: 'text', text, marks: maskMarks(masks[index]) }));
      const markdown = tiptapDocToMarkdown({ type: 'doc', content: [{ type: 'paragraph', content }] });
      const expected = words.flatMap((text, index) => Array.from(text, character => [character,
        semanticNames.filter((_, bit) => masks[index] & (1 << bit)).sort()]));
      expect(renderedInlineFacts(markdown), markdown).toEqual(expected);
      const reloaded = markdownToEditableTiptapDoc(markdown);
      expect(reloaded.content?.[0].type, markdown).toBe('paragraph');
      expect(renderedInlineFacts(tiptapDocToMarkdown(reloaded)), markdown).toEqual(expected);
    });
  }
  it.each(Array.from({ length: 256 }, (_, value) => [value & 15, value >> 4]))(
    'preserves code mixed with formatting %s/%s', (left, right) => {
      const words = ['a`b', ' c '];
      const masks = [left, right];
      const markdown = tiptapDocToMarkdown({ type: 'doc', content: [{ type: 'paragraph',
        content: words.map((text, index) => ({ type: 'text', text, marks: maskMarks(masks[index]) })),
      }] });
      const expected = words.flatMap((text, index) => Array.from(text, character => [character,
        semanticNames.filter((_, bit) => masks[index] & (1 << bit)).sort()]));
      expect(renderedInlineFacts(markdown), markdown).toEqual(expected);
      const reloaded = markdownToEditableTiptapDoc(markdown);
      expect(reloaded.content?.[0].type, markdown).toBe('paragraph');
      expect(renderedInlineFacts(tiptapDocToMarkdown(reloaded)), markdown).toEqual(expected);
    },
  );
});

const blockSamples = [
  '````js\nconst fence = "```";\n````',
  '~~~~lang`name\n~~~\n~~~~',
  '- first paragraph\n\n  second paragraph\n\n- next item',
  '1. first\n\n   another paragraph\n\n2. second',
  '- item\n\n  > quote\n  >\n  > another\n\n  continuation',
  '- first\n  - nested\n\n    extra paragraph\n- last',
  '<p class="note">Styled text</p>',
  '<div align="center" class="note">\n\nText\n\n</div>',
  'Text <strong class="note">formatted</strong> after.',
  '<img src="a.png" width="200">',
  'Text <a href="a.html" data-id="x">link</a> after.',
  '- [x] checked\n- plain\n- [ ] pending',
  '- plain\n- [x] checked\n- last',
  '> - [x] checked\n> - plain',
  '- parent\n  - [x] checked\n  - plain',
  '1. [x] checked\n2. plain',
  '[a](x)[b](x)',
  '<details><summary title="tip">Title</summary>\n\nBody\n\n</details>',
  '[ref][a]\n\n[a]: ./a.md "Title"',
  'Footnote[^a]\n\n[^a]: Meaning',
];
describe('block boundaries and unsupported attributes', () => {
  it.each(blockSamples)('preserves source semantics and neighboring native blocks: %s', sample => {
    const source = '# Before\n\n'+sample+'\n\nAfter';
    const doc = markdownToEditableTiptapDoc(source);
    expect(doc.content?.[0].type).toBe('heading');
    expect(doc.content?.at(-1)?.type).toBe('paragraph');
    expect(semantics(tiptapDocToMarkdown(doc))).toEqual(semantics(source));
  });
});

 it('decodes entities when upgrading a simple HTML paragraph', () => {
  const markdown = tiptapDocToMarkdown(markdownToEditableTiptapDoc('<p>A &amp; B</p>'));
  expect(renderedInlineFacts(markdown)).toEqual(Array.from('A & B', character => [character, []]));
});

const destinations = ['./a.md', 'https://example.com/a b', 'a)b', 'a(b)c', 'a\\b', 'a&copy;b', 'a?x=1&y=2', 'a"b', 'a<b>', '', '#part'];
const titles = [null, '', 'Tooltip', 'a "quote"', "a 'quote'", 'a\\b', '&copy; & <tag>', 'two\nlines'];
describe('link and image attributes', () => {
  for (const url of destinations) {
    it.each(titles)('preserves '+JSON.stringify(url)+' and title %s', title => {
      const doc = { type: 'doc', content: [{ type: 'paragraph', content: [
        { type: 'text', text: 'label', marks: [{ type: 'link', attrs: { href: url, title } }] },
        { type: 'text', text: ' after ' },
        { type: 'markdownImage', attrs: { src: url, alt: 'alt [x] & text', title } },
      ] }] };
      const serialized = tiptapDocToMarkdown(doc);
      if (title === '') {
        const tree = htmlParser.runSync(htmlParser.parse(serialized));
        const paragraph = tree.children.find(n => n.type === 'element');
        if (paragraph?.type !== 'element') throw new Error('Expected paragraph');
        expect(paragraph.children.find(n => n.type === 'element' && n.tagName === 'a')).toMatchObject({ properties: { href: url, title: '' } });
        expect(paragraph.children.find(n => n.type === 'element' && n.tagName === 'img')).toMatchObject({ properties: { src: url, title: '', alt: 'alt [x] & text' } });
        expect(markdownToEditableTiptapDoc(serialized).content?.[0].type).toBe('paragraph');
        return;
      }
      const parsed = parser.parse(serialized).children[0];
      expect(parsed.type).toBe('paragraph');
      if (parsed.type !== 'paragraph') return;
      expect(parsed.children.find(n => n.type === 'link')).toMatchObject({ url, title });
      expect(parsed.children.find(n => n.type === 'image')).toMatchObject({ url, title, alt: 'alt [x] & text' });
    });
  }
});

it('isolates an unsafe region while preserving frontmatter and neighboring native blocks', () => {
  const block = 'Text <b><strong>redundant nested marks</strong></b> after.';
  expect(analyzeMarkdownEditability(block).mode).toBe('unsafe');
  const markdown = '---\ntitle: Test\n---\n\n# Before\n\n'+block+'\n\nAfter';
  const doc = markdownToEditableTiptapDoc(markdown);
  expect(doc.content?.map(n => n.type)).toEqual(['frontmatter', 'heading', 'renderOnlyBlock', 'paragraph']);
  expect(doc.content?.[2].attrs?.markdown).toBe(block);
  doc.content![3].content![0].text = 'Updated';
  expect(tiptapDocToMarkdown(doc)).toBe(markdown.replace(/After$/, 'Updated'));
});

it.each(Array.from({ length: 16 }, (_, mask) => mask))('preserves underline with formatting mask %s', mask => {
  const content = [{ type: 'text', text: ' styled ', marks: [...maskMarks(mask), { type: 'underline' }] }];
  const markdown = tiptapDocToMarkdown({ type: 'doc', content: [{ type: 'paragraph', content }] });
  const expected = Array.from(' styled ', character => [character, [...semanticNames.filter((_, bit) => mask & (1 << bit)), 'underline'].sort()]);
  expect(renderedInlineFacts(markdown)).toEqual(expected);
  const doc = markdownToEditableTiptapDoc(markdown);
  expect(doc.content?.[0].type).toBe('paragraph');
  expect(renderedInlineFacts(tiptapDocToMarkdown(doc))).toEqual(expected);
});

it('emits literal markup safely when preserving rich-text-only ranges', () => {
  const text = '<img src=x onerror=alert(1)> &copy;';
  const doc = { type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text, marks: [{ type: 'underline' }] }] }] };
  const saved = tiptapDocToMarkdown(doc);
  expect(renderedInlineFacts(saved)).toEqual(Array.from(text, character => [character, ['underline']]));
  expect(saved).not.toContain('<img');
});
