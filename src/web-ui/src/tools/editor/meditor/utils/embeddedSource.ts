/** Present the editable payload while retaining the Markdown wrapper on disk. */
export function embeddedSource(markdown: string, kind?: string | null) {
  const fenced = markdown.match(/^( {0,3})(`{3,}|~{3,})([^\r\n]*)(\r?\n)([\s\S]*?)(\r?\n)( {0,3})(`{3,}|~{3,})([ \t]*)$/);
  if (fenced && fenced[2][0] === fenced[8][0] && fenced[8].length >= fenced[2].length) {
    const [, indent, fence, info, openingNewline, source, closingNewline, closingIndent, closingFence, trailing] = fenced;
    return {
      source,
      language: info.trim().split(/\s/)[0] || 'code',
      wrap: (value: string) => {
        // Pasting a fence into a code block must not terminate that block.
        const runs = value.match(fence[0] === '`' ? /^ {0,3}`{3,}/gm : /^ {0,3}~{3,}/gm) ?? [];
        const length = Math.max(fence.length, ...runs.map(run => run.trim().length + 1));
        const opening = fence[0].repeat(length);
        const closing = fence[0].repeat(Math.max(closingFence.length, length));
        return `${indent}${opening}${info}${openingNewline}${value}${closingNewline}${closingIndent}${closing}${trailing}`;
      },
    };
  }
  if (kind === 'math' || kind === 'inlineMath') {
    const math = markdown.match(/^(\${1,})([\s\S]*?)\1$/);
    if (math) {
      const [, delimiter, body] = math;
      const opening = body.match(/^\r?\n/)?.[0] ?? '';
      const closing = body.match(/\r?\n$/)?.[0] ?? '';
      const source = body.slice(opening.length, closing ? -closing.length : undefined);
      return {
        source,
        language: 'math',
        wrap: (value: string) => `${delimiter}${opening}${value}${closing}${delimiter}`,
      };
    }
  }
  return { source: markdown, language: kind ?? 'markdown', wrap: (value: string) => value };
}
