import { describe, expect, it } from 'vitest';
import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import remarkRehype from 'remark-rehype';
import rehypeSanitize from 'rehype-sanitize';
import rehypeRaw from 'rehype-raw';
import { rehypeSourceRange } from './rehypeSourceRange';

function renderRegions(regions: string[], idPrefix = 'editor-1-') {
  const content = regions.join('\n\n');
  let start = 0;
  return regions.map(region => {
    const processor = unified().use(remarkParse).use(remarkGfm).use(remarkRehype)
      .use(rehypeRaw).use(rehypeSanitize).use(rehypeSourceRange, { start, end: start + region.length, idPrefix });
    start += region.length + 2;
    return processor.runSync(processor.parse(content));
  });
}

function elements(tree: any, predicate: (node: any) => boolean): any[] {
  return [ ...(predicate(tree) ? [tree] : []), ...(tree.children ?? []).flatMap((node: any) => elements(node, predicate)) ];
}

describe('Markdown source region rendering', () => {
  it('shares numbering and matching anchors without duplicating footnote bodies', () => {
    const regions = ['First[^b].', 'Second[^a] and again[^b].', '[^a]: Alpha', '[^b]: Beta'];
    const trees = renderRegions(regions);
    const refs = trees.flatMap(tree => elements(tree, node => node.properties?.dataFootnoteRef !== undefined));
    expect(refs.map(node => node.children[0].value)).toEqual(['1', '2', '1']);
    const definitions = trees.flatMap(tree => elements(tree, node => node.tagName === 'li'));
    expect(definitions).toHaveLength(2);
    expect(definitions.map(node => node.properties.value)).toEqual([2, 1]);
    const allIds = trees.flatMap(tree => elements(tree, node => typeof node.properties?.id === 'string'))
      .map(node => node.properties.id);
    expect(new Set(allIds).size).toBe(allIds.length);
    const links = trees.flatMap(tree => elements(tree, node => node.tagName === 'a'));
    links.forEach(node => expect(allIds).toContain(node.properties.href.slice(1)));
    expect(elements(trees[0], node => node.tagName === 'section')).toHaveLength(0);
    expect(JSON.stringify(trees[2])).not.toContain('Beta');
    expect(JSON.stringify(trees[3])).not.toContain('Alpha');
    const otherIds = renderRegions(regions, 'editor-2-').flatMap(tree =>
      elements(tree, node => typeof node.properties?.id === 'string')).map(node => node.properties.id);
    expect(otherIds.some(id => allIds.includes(id))).toBe(false);
  });

  it('resolves reference links and images from definitions outside the region', () => {
    const [tree] = renderRegions(['[Guide][g] ![Image][img]', '[g]: https://example.com', '[img]: photo.png']);
    expect(elements(tree, node => node.tagName === 'a')[0].properties.href).toBe('https://example.com');
    expect(elements(tree, node => node.tagName === 'img')[0].properties.src).toBe('photo.png');
  });
});
