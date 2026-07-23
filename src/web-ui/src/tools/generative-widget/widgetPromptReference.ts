export interface WidgetPromptReferenceTokenPayload {
  promptText: string;
  displayText: string;
  summary: string;
  sectionSummary?: string;
  filePath?: string;
  line?: number;
}

const TOKEN_PREFIX = '[[bitfun-widget-ref:';
const TOKEN_SUFFIX = ']]';
const TOKEN_PATTERN = /\[\[bitfun-widget-ref:([^[\]]+)\]\]/g;

function encodePayload(payload: WidgetPromptReferenceTokenPayload): string {
  return encodeURIComponent(JSON.stringify(payload));
}

function decodePayload(encoded: string): WidgetPromptReferenceTokenPayload | null {
  try {
    const parsed = JSON.parse(decodeURIComponent(encoded)) as WidgetPromptReferenceTokenPayload;
    if (!parsed || typeof parsed.promptText !== 'string' || typeof parsed.displayText !== 'string') {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

export function createWidgetPromptReferenceToken(
  payload: WidgetPromptReferenceTokenPayload,
): string {
  return `${TOKEN_PREFIX}${encodePayload(payload)}${TOKEN_SUFFIX}`;
}

export function parseWidgetPromptReferenceToken(
  token: string,
): WidgetPromptReferenceTokenPayload | null {
  const match = token.match(/^\[\[bitfun-widget-ref:([^[\]]+)\]\]$/);
  if (!match) {
    return null;
  }
  return decodePayload(match[1]);
}

export function getWidgetPromptReferenceMatches(text: string): Array<{
  token: string;
  start: number;
  end: number;
  payload: WidgetPromptReferenceTokenPayload;
}> {
  const matches: Array<{
    token: string;
    start: number;
    end: number;
    payload: WidgetPromptReferenceTokenPayload;
  }> = [];

  TOKEN_PATTERN.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = TOKEN_PATTERN.exec(text)) !== null) {
    const payload = decodePayload(match[1]);
    if (!payload) {
      continue;
    }
    matches.push({
      token: match[0],
      start: match.index,
      end: match.index + match[0].length,
      payload,
    });
  }

  return matches;
}

export function expandWidgetPromptReferenceTokens(text: string): string {
  return text.replace(TOKEN_PATTERN, (token) => {
    const payload = parseWidgetPromptReferenceToken(token);
    return payload?.promptText || token;
  });
}
