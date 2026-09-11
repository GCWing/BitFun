import { describe, expect, it } from 'vitest';
import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';
import { remarkAutolinkBoundaries } from './remarkAutolinkBoundaries';

const url = 'http://127.0.0.1:8000';
const open = '\uff08';
const close = '\uff09';
const label = '\u94fe\u63a5';

function parse(content: string, math = false) {
  const processor = unified().use(remarkParse).use(remarkGfm);
  if (math) processor.use(remarkMath);
  processor.use(remarkAutolinkBoundaries);
  const tree = processor.runSync(processor.parse(content), { value: content });
  const links: string[] = [];
  const read = (node: { type: string; url?: string; value?: string; children?: typeof tree.children }): string => {
    if (node.type === 'link') links.push(node.url!);
    return node.value ?? (node.children ?? []).map(read).join('');
  };
  return { text: read(tree), links };
}

describe('bare URL prose boundaries', () => {
  it.each([false, true])('separates the reported adjacent labels (math=%s)', math => {
    for (const bare of [url, url.replace(':', '\\:')]) {
      const content = `${open}${label}1 ${bare}${close}${open}${label}2 [${url}](${url}) ${close}`;
      expect(parse(content, math)).toEqual({
        links: [url, url],
        text: `${open}${label}1 ${url}${close}${open}${label}2 ${url} ${close}`,
      });
    }
  });

  it.each(['\uff0c', '\u3002', '\u3001', '\uff1b', '\uff01', '\uff1f', '\u201d', '\u300b', '\u3011'])('returns punctuation and following prose to text: %s', boundary => {
    const content = `${url}${boundary}${label}`;
    expect(parse(content)).toEqual({ links: [url], text: content });
  });

  it('preserves adjacent bare links and www links', () => {
    const content = `${open}${url}${close}${open}https://example.com/path${close}\uff0cwww.example.com/path\u3002`;
    expect(parse(content)).toEqual({ links: [url, 'https://example.com/path', 'http://www.example.com/path'], text: content });
  });

  it('preserves sibling order in a dense paragraph mixing bare and authored links', () => {
    const entries = Array.from({ length: 1000 }, (_, index) => {
      const destination = `${url}/${index}`;
      return index % 2 === 0
        ? { markdown: `${destination}${close}${label}`, text: `${destination}${close}${label}`, destination }
        : { markdown: `[${label}](${destination}${close})`, text: label, destination: `${destination}${close}` };
    });
    expect(parse(entries.map(entry => entry.markdown).join(' '))).toEqual({
      links: entries.map(entry => entry.destination),
      text: entries.map(entry => entry.text).join(' '),
    });
  });

  it('repairs links in nested blocks and inline formatting without changing their order', () => {
    const content = `> - **${url}/first${close}** and *${url}/second${close}*\n>   - ${url}/third${close}\n\n| Link |\n| --- |\n| ${url}/fourth${close} |`;
    const result = parse(content);
    expect(result.links).toEqual(['first', 'second', 'third', 'fourth'].map(path => `${url}/${path}`));
    expect(result.text.match(/\uff09/g)).toHaveLength(4);
  });

  it.each([
    'https://example.com/\u4e2d\u6587?q=\u641c\u7d22#\u7ae0\u8282',
    'https://example.com/wiki/Function_(mathematics)',
    'https://example.com/%EF%BC%89',
  ])('preserves valid URL content: %s', value => {
    expect(parse(`${open}${value}${close}`).links).toEqual([value]);
  });

  it('preserves authored inline, angle and reference links with punctuation in the URL', () => {
    const destination = `https://example.com/a${close}b`;
    const content = `[${destination}](${destination}) <${destination}> [${destination}][ref]\n\n[ref]: ${destination}`;
    expect(parse(content).links).toEqual([destination, destination]);
    const baseline = unified().use(remarkParse).use(remarkGfm);
    const original = baseline.parse(content);
    expect(baseline().use(remarkAutolinkBoundaries).runSync(baseline.parse(content), { value: content })).toEqual(original);
  });

  it('preserves explicitly linked IPv6 URLs', () => {
    const destination = 'http://[::1]:8000/path?q=1&next=2#section';
    expect(parse(`[IPv6](${destination})`).links).toEqual([destination]);
  });

  it('does not link code or image labels', () => {
    const content = `\`${url}${close}\`\n\n\`\`\`\n${url}${close}\n\`\`\`\n\n![${url}${close}](image.png)`;
    expect(parse(content).links).toEqual([]);
  });

  it('retains math and prose links in the same document', () => {
    expect(parse(`$x^2$\n\n${url}${close}`, true).links).toEqual([url]);
  });
});
