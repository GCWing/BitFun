export interface ParsedMarkdownInline {
  id: string;
  type: string;
  text: string;
  url: string;
}

export interface ParsedMarkdownListItem {
  id: string;
  marker: string;
  text: string;
  inlines: ParsedMarkdownInline[];
}

export interface ParsedMarkdownBlock {
  id: string;
  type: string;
  level: number;
  language: string;
  text: string;
  items: ParsedMarkdownListItem[];
  inlines: ParsedMarkdownInline[];
}

export class MarkdownParser {
  static parse(text: string): ParsedMarkdownBlock[] {
    const lines = text.replace(/\r\n/g, '\n').split('\n');
    const blocks: ParsedMarkdownBlock[] = [];
    let index = 0;

    while (index < lines.length) {
      const raw = lines[index];
      const trimmed = raw.trim();
      if (trimmed.length === 0) {
        index += 1;
        continue;
      }

      if (trimmed.indexOf('```') === 0) {
        index = MarkdownParser.readCodeBlock(lines, index, blocks);
      } else if (MarkdownParser.isDivider(trimmed)) {
        MarkdownParser.pushBlock(blocks, 'divider', 0, '', '', []);
        index += 1;
      } else if (MarkdownParser.isHeading(trimmed)) {
        const level = MarkdownParser.headingLevel(trimmed);
        MarkdownParser.pushBlock(blocks, 'heading', level, '', trimmed.slice(level).trim(), []);
        index += 1;
      } else if (MarkdownParser.isQuote(trimmed)) {
        index = MarkdownParser.readQuoteBlock(lines, index, blocks);
      } else if (MarkdownParser.isListLine(trimmed)) {
        index = MarkdownParser.readListBlock(lines, index, blocks);
      } else if (MarkdownParser.isTableStart(lines, index)) {
        index = MarkdownParser.readTableBlock(lines, index, blocks);
      } else {
        index = MarkdownParser.readParagraphBlock(lines, index, blocks);
      }
    }

    if (blocks.length > 0) {
      return blocks;
    }
    MarkdownParser.pushBlock(blocks, 'paragraph', 0, '', text, []);
    return blocks;
  }

  static formatInlineText(value: string): string {
    let text = value;
    text = text.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '$1 ($2)');
    text = text.replace(/`([^`]+)`/g, '$1');
    text = text.replace(/\*\*([^*]+)\*\*/g, '$1');
    text = text.replace(/__([^_]+)__/g, '$1');
    text = text.replace(/\*([^*]+)\*/g, '$1');
    text = text.replace(/_([^_]+)_/g, '$1');
    return text;
  }

  private static readCodeBlock(lines: string[], start: number, blocks: ParsedMarkdownBlock[]): number {
    const opening = lines[start].trim();
    const language = opening.slice(3).trim();
    const body: string[] = [];
    let index = start + 1;
    while (index < lines.length) {
      const trimmed = lines[index].trim();
      if (trimmed.indexOf('```') === 0) {
        index += 1;
        break;
      }
      body.push(lines[index]);
      index += 1;
    }
    MarkdownParser.pushBlock(blocks, 'code', 0, language, body.join('\n'), []);
    return index;
  }

  private static readQuoteBlock(lines: string[], start: number, blocks: ParsedMarkdownBlock[]): number {
    const body: string[] = [];
    let index = start;
    while (index < lines.length) {
      const trimmed = lines[index].trim();
      if (!MarkdownParser.isQuote(trimmed)) {
        break;
      }
      body.push(trimmed.slice(1).trim());
      index += 1;
    }
    MarkdownParser.pushBlock(blocks, 'quote', 0, '', body.join('\n'), []);
    return index;
  }

  private static readListBlock(lines: string[], start: number, blocks: ParsedMarkdownBlock[]): number {
    const items: ParsedMarkdownListItem[] = [];
    let index = start;
    while (index < lines.length) {
      const trimmed = lines[index].trim();
      if (!MarkdownParser.isListLine(trimmed)) {
        break;
      }
      const marker = MarkdownParser.listMarker(trimmed);
      const text = MarkdownParser.listText(trimmed);
      const item: ParsedMarkdownListItem = {
        id: `li-${start}-${items.length}`,
        marker,
        text: MarkdownParser.formatInlineText(text),
        inlines: MarkdownParser.parseInlineText(text, `li-${start}-${items.length}`)
      };
      items.push(item);
      index += 1;
    }
    MarkdownParser.pushBlock(blocks, 'list', 0, '', '', items);
    return index;
  }

  private static readTableBlock(lines: string[], start: number, blocks: ParsedMarkdownBlock[]): number {
    const rows: string[] = [];
    let index = start;
    while (index < lines.length) {
      const trimmed = lines[index].trim();
      if (!MarkdownParser.hasTablePipes(trimmed)) {
        break;
      }
      rows.push(trimmed);
      index += 1;
    }
    MarkdownParser.pushBlock(blocks, 'table', 0, '', rows.join('\n'), []);
    return index;
  }

  private static readParagraphBlock(lines: string[], start: number, blocks: ParsedMarkdownBlock[]): number {
    const body: string[] = [];
    let index = start;
    while (index < lines.length) {
      const trimmed = lines[index].trim();
      if (trimmed.length === 0 ||
        trimmed.indexOf('```') === 0 ||
        MarkdownParser.isDivider(trimmed) ||
        MarkdownParser.isHeading(trimmed) ||
        MarkdownParser.isQuote(trimmed) ||
        MarkdownParser.isListLine(trimmed) ||
        MarkdownParser.isTableStart(lines, index)) {
        break;
      }
      body.push(trimmed);
      index += 1;
    }
    MarkdownParser.pushBlock(blocks, 'paragraph', 0, '', body.join('\n'), []);
    return index;
  }

  private static pushBlock(
    blocks: ParsedMarkdownBlock[],
    type: string,
    level: number,
    language: string,
    text: string,
    items: ParsedMarkdownListItem[]
  ): void {
    const block: ParsedMarkdownBlock = {
      id: `md-${blocks.length}-${type}`,
      type,
      level,
      language,
      text: MarkdownParser.shouldParseInline(type) ? MarkdownParser.formatInlineText(text) : text,
      items,
      inlines: MarkdownParser.shouldParseInline(type) ? MarkdownParser.parseInlineText(text, `md-${blocks.length}-${type}`) : []
    };
    blocks.push(block);
  }

  static parseInlineText(value: string, prefix: string = 'inline'): ParsedMarkdownInline[] {
    const inlines: ParsedMarkdownInline[] = [];
    let index = 0;
    while (index < value.length) {
      const next = MarkdownParser.nextInlineMarker(value, index);
      if (next < 0) {
        MarkdownParser.pushInline(inlines, prefix, 'text', value.slice(index), '');
        break;
      }
      if (next > index) {
        MarkdownParser.pushInline(inlines, prefix, 'text', value.slice(index, next), '');
      }
      const consumed = MarkdownParser.readInlineAt(value, next, prefix, inlines);
      if (consumed <= next) {
        MarkdownParser.pushInline(inlines, prefix, 'text', value.charAt(next), '');
        index = next + 1;
      } else {
        index = consumed;
      }
    }
    return inlines.filter((inline: ParsedMarkdownInline) => inline.text.length > 0);
  }

  private static readInlineAt(value: string, start: number, prefix: string, inlines: ParsedMarkdownInline[]): number {
    if (value.charAt(start) === '`') {
      const end = value.indexOf('`', start + 1);
      if (end > start + 1) {
        MarkdownParser.pushInline(inlines, prefix, 'code', value.slice(start + 1, end), '');
        return end + 1;
      }
    }
    if (value.slice(start, start + 2) === '**') {
      const end = value.indexOf('**', start + 2);
      if (end > start + 2) {
        MarkdownParser.pushInline(inlines, prefix, 'strong', value.slice(start + 2, end), '');
        return end + 2;
      }
    }
    if (value.slice(start, start + 2) === '__') {
      const end = value.indexOf('__', start + 2);
      if (end > start + 2) {
        MarkdownParser.pushInline(inlines, prefix, 'strong', value.slice(start + 2, end), '');
        return end + 2;
      }
    }
    if (value.charAt(start) === '[') {
      const labelEnd = value.indexOf(']', start + 1);
      if (labelEnd > start + 1 && value.charAt(labelEnd + 1) === '(') {
        const urlEnd = value.indexOf(')', labelEnd + 2);
        if (urlEnd > labelEnd + 2) {
          MarkdownParser.pushInline(inlines, prefix, 'link', value.slice(start + 1, labelEnd), value.slice(labelEnd + 2, urlEnd));
          return urlEnd + 1;
        }
      }
    }
    if (value.charAt(start) === '*') {
      const end = value.indexOf('*', start + 1);
      if (end > start + 1) {
        MarkdownParser.pushInline(inlines, prefix, 'emphasis', value.slice(start + 1, end), '');
        return end + 1;
      }
    }
    if (value.charAt(start) === '_') {
      const end = value.indexOf('_', start + 1);
      if (end > start + 1) {
        MarkdownParser.pushInline(inlines, prefix, 'emphasis', value.slice(start + 1, end), '');
        return end + 1;
      }
    }
    return start;
  }

  private static nextInlineMarker(value: string, start: number): number {
    const markers: string[] = ['`', '**', '__', '[', '*', '_'];
    let next = -1;
    markers.forEach((marker: string) => {
      const index = value.indexOf(marker, start);
      if (index >= 0 && (next < 0 || index < next)) {
        next = index;
      }
    });
    return next;
  }

  private static pushInline(
    inlines: ParsedMarkdownInline[],
    prefix: string,
    type: string,
    text: string,
    url: string
  ): void {
    const inline: ParsedMarkdownInline = {
      id: `${prefix}-inline-${inlines.length}`,
      type,
      text,
      url
    };
    inlines.push(inline);
  }

  private static shouldParseInline(type: string): boolean {
    return type === 'paragraph' || type === 'heading' || type === 'quote';
  }

  private static isHeading(line: string): boolean {
    return /^#{1,4}\s+/.test(line);
  }

  private static headingLevel(line: string): number {
    const match = line.match(/^#{1,4}/);
    return match ? match[0].length : 1;
  }

  private static isDivider(line: string): boolean {
    return /^(-{3,}|\*{3,}|_{3,})$/.test(line);
  }

  private static isQuote(line: string): boolean {
    return line.indexOf('>') === 0;
  }

  private static isListLine(line: string): boolean {
    return /^[-*+]\s+/.test(line) || /^\d+\.\s+/.test(line);
  }

  private static listMarker(line: string): string {
    const ordered = line.match(/^(\d+)\.\s+/);
    if (ordered) {
      return `${ordered[1]}.`;
    }
    return '•';
  }

  private static listText(line: string): string {
    return line.replace(/^[-*+]\s+/, '').replace(/^\d+\.\s+/, '');
  }

  private static isTableStart(lines: string[], index: number): boolean {
    if (index + 1 >= lines.length) {
      return false;
    }
    const current = lines[index].trim();
    const next = lines[index + 1].trim();
    return MarkdownParser.hasTablePipes(current) && /^\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?$/.test(next);
  }

  private static hasTablePipes(line: string): boolean {
    return line.indexOf('|') >= 0 && line.lastIndexOf('|') > line.indexOf('|');
  }
}
