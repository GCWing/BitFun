export interface MarkdownFrontmatterSplit {
  raw: string;
  body: string;
  delimiter: '---' | '+++';
  yaml: string;
  openingLineEnding: '\n' | '\r\n';
  contentLineEnding: '' | '\n' | '\r\n';
  closingLineEnding: '' | '\n' | '\r\n';
}

/**
 * Split leading YAML/TOML-style frontmatter without parsing or normalizing it.
 * `raw` ends after the first line ending following the closing delimiter. Any
 * additional blank lines belong to `body`, so joining `raw + body` preserves
 * the source document byte-for-byte.
 */
export function splitMarkdownFrontmatter(markdown: string): MarkdownFrontmatterSplit | null {
  const openingMatch = markdown.match(/^(---|\+\+\+)(\r?\n)/);
  if (!openingMatch) {
    return null;
  }

  const delimiter = openingMatch[1] as MarkdownFrontmatterSplit['delimiter'];
  const openingLineEnding = openingMatch[2] as MarkdownFrontmatterSplit['openingLineEnding'];
  const afterOpening = markdown.slice(openingMatch[0].length);
  const escapedDelimiter = delimiter.replace(/\+/g, '\\+');
  const closingPattern = new RegExp(`^${escapedDelimiter}(?:((?:\\r?\\n))|$)`, 'm');
  const closingMatch = closingPattern.exec(afterOpening);
  if (!closingMatch) {
    return null;
  }

  const yamlWithStructuralEnding = afterOpening.slice(0, closingMatch.index);
  const contentLineEndingMatch = yamlWithStructuralEnding.match(/(\r?\n)$/);
  const contentLineEnding = (contentLineEndingMatch?.[1] ?? '') as MarkdownFrontmatterSplit['contentLineEnding'];
  const yaml = contentLineEnding
    ? yamlWithStructuralEnding.slice(0, -contentLineEnding.length)
    : yamlWithStructuralEnding;
  const closingLineEnding = (closingMatch[1] ?? '') as MarkdownFrontmatterSplit['closingLineEnding'];
  const rawLength = openingMatch[0].length + closingMatch.index + closingMatch[0].length;
  const raw = markdown.slice(0, rawLength);

  return {
    raw,
    body: markdown.slice(raw.length),
    delimiter,
    yaml,
    openingLineEnding,
    contentLineEnding,
    closingLineEnding,
  };
}

export function serializeMarkdownFrontmatter(
  frontmatter: Pick<
    MarkdownFrontmatterSplit,
    'delimiter' | 'yaml' | 'openingLineEnding' | 'contentLineEnding' | 'closingLineEnding'
  >,
): string {
  const contentLineEnding = frontmatter.contentLineEnding || (
    frontmatter.yaml ? frontmatter.openingLineEnding : ''
  );

  return [
    frontmatter.delimiter,
    frontmatter.openingLineEnding,
    frontmatter.yaml,
    contentLineEnding,
    frontmatter.delimiter,
    frontmatter.closingLineEnding,
  ].join('');
}
