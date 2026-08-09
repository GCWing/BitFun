export enum CodeSyntaxTokenKind {
  Plain = 'plain',
  LineNumber = 'line-number',
  Keyword = 'keyword',
  String = 'string',
  Number = 'number',
  Comment = 'comment',
  Function = 'function',
  Type = 'type',
  Constant = 'constant',
  Property = 'property'
}

export class CodeSyntaxToken {
  readonly id: string;
  readonly text: string;
  readonly kind: CodeSyntaxTokenKind;
  readonly lineNumber: number;

  constructor(id: string, text: string, kind: CodeSyntaxTokenKind, lineNumber: number = 0) {
    this.id = id;
    this.text = text;
    this.kind = kind;
    this.lineNumber = lineNumber;
  }
}

export class CodeSyntaxHighlightCache {
  private text: string = '';
  private fileName: string = '';
  private tokens: CodeSyntaxToken[] = [];
  private initialized: boolean = false;

  tokensFor(text: string, fileName: string): CodeSyntaxToken[] {
    if (this.initialized && this.text === text && this.fileName === fileName) {
      return this.tokens;
    }
    this.text = text;
    this.fileName = fileName;
    this.tokens = CodeSyntaxHighlighter.tokenize(text, fileName);
    this.initialized = true;
    return this.tokens;
  }
}

const HIGHLIGHT_MAX_CHARACTERS: number = 256 * 1024;
const HIGHLIGHT_MAX_TOKENS: number = 12000;

/**
 * A bounded lexical colorizer for the native preview surface. It deliberately
 * avoids building a full syntax tree and falls back to one plain span for
 * large inputs so ArkUI does not have to mount thousands of child spans.
 */
export class CodeSyntaxHighlighter {
  static tokenize(text: string, fileName: string): CodeSyntaxToken[] {
    const extension = CodeSyntaxHighlighter.extension(fileName);
    if (!CodeSyntaxHighlighter.supports(extension) || text.length > HIGHLIGHT_MAX_CHARACTERS) {
      return CodeSyntaxHighlighter.plain(text);
    }

    const tokens: CodeSyntaxToken[] = [];
    const lineCount = CodeSyntaxHighlighter.lineCount(text);
    const lineNumberWidth = String(lineCount).length;
    let lineNumber = 1;
    let index = 0;
    let tokenIndex = 0;
    let needsLineNumber = true;

    while (index < text.length) {
      if (needsLineNumber) {
        tokens.push(new CodeSyntaxToken(
          `syntax-${tokenIndex++}`,
          CodeSyntaxHighlighter.linePrefix(lineNumber, lineNumberWidth),
          CodeSyntaxTokenKind.LineNumber,
          lineNumber
        ));
        needsLineNumber = false;
      }

      const character = text.charAt(index);
      if (character === '\n') {
        tokens.push(new CodeSyntaxToken(
          `syntax-${tokenIndex++}`, '\n', CodeSyntaxTokenKind.Plain, lineNumber
        ));
        index += 1;
        lineNumber += 1;
        needsLineNumber = true;
        continue;
      }

      const lineComment = CodeSyntaxHighlighter.lineCommentAt(text, index, extension);
      if (lineComment.length > 0) {
        const end = CodeSyntaxHighlighter.lineEnd(text, index);
        tokens.push(new CodeSyntaxToken(
          `syntax-${tokenIndex++}`,
          text.slice(index, end),
          CodeSyntaxTokenKind.Comment,
          lineNumber
        ));
        index = end;
        continue;
      }

      const blockCommentEnd = CodeSyntaxHighlighter.blockCommentEnd(text, index, extension);
      if (blockCommentEnd > index) {
        const block = text.slice(index, blockCommentEnd);
        CodeSyntaxHighlighter.pushMultilineToken(
          tokens,
          block,
          CodeSyntaxTokenKind.Comment,
          lineNumberWidth,
          lineNumber,
          tokenIndex
        );
        const consumedLines = CodeSyntaxHighlighter.newlineCount(block);
        tokenIndex = tokens.length;
        lineNumber += consumedLines;
        needsLineNumber = block.endsWith('\n');
        index = blockCommentEnd;
        continue;
      }

      if (character === '\'' || character === '"' || character === '`') {
        const end = CodeSyntaxHighlighter.stringEnd(text, index, character);
        const value = text.slice(index, end);
        CodeSyntaxHighlighter.pushMultilineToken(
          tokens,
          value,
          CodeSyntaxTokenKind.String,
          lineNumberWidth,
          lineNumber,
          tokenIndex
        );
        const consumedLines = CodeSyntaxHighlighter.newlineCount(value);
        tokenIndex = tokens.length;
        lineNumber += consumedLines;
        needsLineNumber = value.endsWith('\n');
        index = end;
        continue;
      }

      if (CodeSyntaxHighlighter.isDigit(character)) {
        const end = CodeSyntaxHighlighter.numberEnd(text, index);
        tokens.push(new CodeSyntaxToken(
          `syntax-${tokenIndex++}`,
          text.slice(index, end),
          CodeSyntaxTokenKind.Number,
          lineNumber
        ));
        index = end;
        continue;
      }

      if (CodeSyntaxHighlighter.isIdentifierStart(character)) {
        const end = CodeSyntaxHighlighter.identifierEnd(text, index);
        const word = text.slice(index, end);
        tokens.push(new CodeSyntaxToken(
          `syntax-${tokenIndex++}`,
          word,
          CodeSyntaxHighlighter.identifierKind(text, end, word, extension),
          lineNumber
        ));
        index = end;
        continue;
      }

      const end = CodeSyntaxHighlighter.plainEnd(text, index);
      tokens.push(new CodeSyntaxToken(
        `syntax-${tokenIndex++}`,
        text.slice(index, end),
        CodeSyntaxTokenKind.Plain,
        lineNumber
      ));
      index = end;

      if (tokens.length > HIGHLIGHT_MAX_TOKENS) {
        return CodeSyntaxHighlighter.plain(text);
      }
    }

    if (text.length === 0 || needsLineNumber) {
      tokens.push(new CodeSyntaxToken(
        `syntax-${tokenIndex}`,
        CodeSyntaxHighlighter.linePrefix(lineNumber, lineNumberWidth),
        CodeSyntaxTokenKind.LineNumber,
        lineNumber
      ));
    }
    return tokens;
  }

  private static pushMultilineToken(
    tokens: CodeSyntaxToken[],
    value: string,
    kind: CodeSyntaxTokenKind,
    lineNumberWidth: number,
    firstLineNumber: number,
    firstTokenIndex: number
  ): void {
    const parts = value.split('\n');
    let lineNumber = firstLineNumber;
    let tokenIndex = firstTokenIndex;
    parts.forEach((part: string, index: number) => {
      if (part.length > 0) {
        tokens.push(new CodeSyntaxToken(`syntax-${tokenIndex++}`, part, kind, lineNumber));
      }
      if (index < parts.length - 1) {
        tokens.push(new CodeSyntaxToken(
          `syntax-${tokenIndex++}`, '\n', CodeSyntaxTokenKind.Plain, lineNumber
        ));
        lineNumber += 1;
        if (index < parts.length - 2) {
          tokens.push(new CodeSyntaxToken(
            `syntax-${tokenIndex++}`,
            CodeSyntaxHighlighter.linePrefix(lineNumber, lineNumberWidth),
            CodeSyntaxTokenKind.LineNumber,
            lineNumber
          ));
        }
      }
    });
  }

  private static plain(text: string): CodeSyntaxToken[] {
    return [new CodeSyntaxToken('syntax-plain', CodeSyntaxHighlighter.numberedText(text), CodeSyntaxTokenKind.Plain)];
  }

  private static numberedText(text: string): string {
    const lines = text.split('\n');
    const width = String(lines.length).length;
    return lines.map((line: string, index: number): string => {
      return `${CodeSyntaxHighlighter.linePrefix(index + 1, width)}${line}`;
    }).join('\n');
  }

  private static linePrefix(lineNumber: number, width: number): string {
    let value = String(lineNumber);
    while (value.length < width) {
      value = ` ${value}`;
    }
    return `${value}  `;
  }

  private static identifierKind(
    text: string,
    end: number,
    word: string,
    extension: string
  ): CodeSyntaxTokenKind {
    if (CodeSyntaxHighlighter.isKeyword(word, extension)) {
      return CodeSyntaxTokenKind.Keyword;
    }
    if ('|true|false|null|undefined|none|nil|self|this|super|'.indexOf(`|${word.toLowerCase()}|`) >= 0) {
      return CodeSyntaxTokenKind.Constant;
    }
    const next = CodeSyntaxHighlighter.nextNonWhitespace(text, end);
    if (next === '(' || next === '!') {
      return CodeSyntaxTokenKind.Function;
    }
    if (next === ':') {
      return CodeSyntaxTokenKind.Property;
    }
    const first = word.charAt(0);
    if (first >= 'A' && first <= 'Z') {
      return CodeSyntaxTokenKind.Type;
    }
    return CodeSyntaxTokenKind.Plain;
  }

  private static isKeyword(word: string, extension: string): boolean {
    const normalized = word.toLowerCase();
    let keywords = '|if|else|for|while|do|switch|case|break|continue|return|throw|try|catch|finally|new|in|of|';
    if ('|js|jsx|ts|tsx|mjs|cjs|ets|vue|svelte|'.indexOf(`|${extension}|`) >= 0) {
      keywords += 'const|let|var|function|class|extends|implements|interface|type|enum|import|export|from|as|async|await|yield|default|delete|instanceof|typeof|void|public|private|protected|readonly|static|get|set|declare|namespace|';
    } else if ('|rs|'.indexOf(`|${extension}|`) >= 0) {
      keywords += 'fn|let|mut|struct|enum|impl|trait|use|mod|pub|crate|where|match|move|ref|async|await|dyn|unsafe|extern|const|static|type|loop|';
    } else if ('|py|pyw|pyi|'.indexOf(`|${extension}|`) >= 0) {
      keywords += 'def|class|import|from|as|lambda|with|async|await|yield|raise|pass|global|nonlocal|assert|del|elif|except|finally|is|not|and|or|';
    } else if ('|go|'.indexOf(`|${extension}|`) >= 0) {
      keywords += 'func|package|import|defer|go|select|chan|map|range|struct|interface|type|var|const|fallthrough|';
    } else if ('|java|kt|kts|scala|groovy|c|cpp|cc|cxx|h|hpp|hxx|hh|cs|swift|'.indexOf(`|${extension}|`) >= 0) {
      keywords += 'class|struct|interface|enum|namespace|using|import|package|public|private|protected|static|final|virtual|override|abstract|const|var|val|fun|func|operator|template|typename|extends|implements|throws|';
    } else if ('|sh|bash|zsh|fish|ps1|bat|cmd|'.indexOf(`|${extension}|`) >= 0) {
      keywords += 'then|fi|elif|done|function|select|until|export|local|readonly|declare|set|unset|';
    } else if ('|sql|'.indexOf(`|${extension}|`) >= 0) {
      keywords += 'select|insert|update|delete|create|alter|drop|from|join|inner|left|right|on|where|group|order|by|having|limit|offset|union|all|distinct|into|values|table|index|view|and|or|not|null|';
    }
    return keywords.indexOf(`|${normalized}|`) >= 0;
  }

  private static lineCommentAt(text: string, index: number, extension: string): string {
    if ('|py|pyw|pyi|rb|sh|bash|zsh|fish|yaml|yml|toml|conf|cfg|ini|'.indexOf(`|${extension}|`) >= 0 &&
      text.charAt(index) === '#') {
      return '#';
    }
    if (extension === 'sql' && text.slice(index, index + 2) === '--') {
      return '--';
    }
    if (text.slice(index, index + 2) === '//') {
      return '//';
    }
    return '';
  }

  private static blockCommentEnd(text: string, index: number, extension: string): number {
    if (text.slice(index, index + 4) === '<!--') {
      const htmlEnd = text.indexOf('-->', index + 4);
      return htmlEnd >= 0 ? htmlEnd + 3 : text.length;
    }
    if (extension !== 'py' && text.slice(index, index + 2) === '/*') {
      const end = text.indexOf('*/', index + 2);
      return end >= 0 ? end + 2 : text.length;
    }
    return index;
  }

  private static stringEnd(text: string, start: number, quote: string): number {
    let index = start + 1;
    let escaped = false;
    while (index < text.length) {
      const character = text.charAt(index);
      if (escaped) {
        escaped = false;
      } else if (character === '\\') {
        escaped = true;
      } else if (character === quote) {
        return index + 1;
      } else if (character === '\n' && quote !== '`') {
        return index;
      }
      index += 1;
    }
    return text.length;
  }

  private static numberEnd(text: string, start: number): number {
    let index = start + 1;
    while (index < text.length) {
      const character = text.charAt(index);
      if (!CodeSyntaxHighlighter.isDigit(character) && character !== '.' && character !== '_' &&
        character.toLowerCase() !== 'x' && character.toLowerCase() !== 'b' &&
        !(character.toLowerCase() >= 'a' && character.toLowerCase() <= 'f')) {
        break;
      }
      index += 1;
    }
    return index;
  }

  private static identifierEnd(text: string, start: number): number {
    let index = start + 1;
    while (index < text.length && CodeSyntaxHighlighter.isIdentifierPart(text.charAt(index))) {
      index += 1;
    }
    return index;
  }

  private static plainEnd(text: string, start: number): number {
    let index = start + 1;
    while (index < text.length) {
      const character = text.charAt(index);
      if (character === '\n' || character === '\'' || character === '"' || character === '`' ||
        CodeSyntaxHighlighter.isDigit(character) || CodeSyntaxHighlighter.isIdentifierStart(character) ||
        text.slice(index, index + 2) === '//' || text.slice(index, index + 2) === '/*' ||
        text.slice(index, index + 2) === '--' || character === '#') {
        break;
      }
      index += 1;
    }
    return index;
  }

  private static lineEnd(text: string, start: number): number {
    const end = text.indexOf('\n', start);
    return end >= 0 ? end : text.length;
  }

  private static nextNonWhitespace(text: string, start: number): string {
    let index = start;
    while (index < text.length && (text.charAt(index) === ' ' || text.charAt(index) === '\t')) {
      index += 1;
    }
    return index < text.length ? text.charAt(index) : '';
  }

  private static lineCount(text: string): number {
    return CodeSyntaxHighlighter.newlineCount(text) + 1;
  }

  private static newlineCount(text: string): number {
    let count = 0;
    for (let index = 0; index < text.length; index++) {
      if (text.charAt(index) === '\n') {
        count += 1;
      }
    }
    return count;
  }

  private static isDigit(character: string): boolean {
    return character >= '0' && character <= '9';
  }

  private static isIdentifierStart(character: string): boolean {
    return (character >= 'a' && character <= 'z') || (character >= 'A' && character <= 'Z') ||
      character === '_' || character === '$';
  }

  private static isIdentifierPart(character: string): boolean {
    return CodeSyntaxHighlighter.isIdentifierStart(character) || CodeSyntaxHighlighter.isDigit(character);
  }

  private static extension(fileName: string): string {
    const normalized = fileName.replace(/\\/g, '/').split('/').pop() || '';
    const dot = normalized.lastIndexOf('.');
    return dot >= 0 ? normalized.slice(dot + 1).toLowerCase() : '';
  }

  private static supports(extension: string): boolean {
    return '|js|jsx|ts|tsx|mjs|cjs|ets|vue|svelte|rs|py|pyw|pyi|rb|go|java|kt|kts|scala|groovy|c|cpp|cc|cxx|h|hpp|hxx|hh|cs|swift|php|css|scss|less|json|jsonc|yaml|yml|toml|xml|html|htm|sh|bash|zsh|fish|ps1|bat|cmd|sql|graphql|gql|proto|'.indexOf(`|${extension}|`) >= 0;
  }
}
