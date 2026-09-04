const TOKEN_PATTERN = /\[\[openbitfun-additional-mode:([a-z][a-z0-9-]*)\]\]/g;

const ADDITIONAL_MODE_DEFINITIONS = {
  review: {
    displayText: 'Review',
    promptCommand: '/review',
  },
} as const;

export type AdditionalModePromptReferenceId = keyof typeof ADDITIONAL_MODE_DEFINITIONS;

export interface AdditionalModePromptReferencePayload {
  id: AdditionalModePromptReferenceId;
  displayText: string;
  promptCommand: string;
}

function isAdditionalModeId(value: string): value is AdditionalModePromptReferenceId {
  return Object.prototype.hasOwnProperty.call(ADDITIONAL_MODE_DEFINITIONS, value);
}

export function createAdditionalModePromptReferenceToken(
  id: AdditionalModePromptReferenceId,
): string {
  return `[[openbitfun-additional-mode:${id}]]`;
}

export function parseAdditionalModePromptReferenceToken(
  token: string,
): AdditionalModePromptReferencePayload | null {
  const match = token.match(/^\[\[openbitfun-additional-mode:([a-z][a-z0-9-]*)\]\]$/);
  const id = match?.[1];
  if (!id || !isAdditionalModeId(id)) {
    return null;
  }

  return {
    id,
    ...ADDITIONAL_MODE_DEFINITIONS[id],
  };
}

export function getAdditionalModePromptReferenceMatches(text: string): Array<{
  token: string;
  start: number;
  end: number;
  payload: AdditionalModePromptReferencePayload;
}> {
  const matches: Array<{
    token: string;
    start: number;
    end: number;
    payload: AdditionalModePromptReferencePayload;
  }> = [];

  TOKEN_PATTERN.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = TOKEN_PATTERN.exec(text)) !== null) {
    const payload = parseAdditionalModePromptReferenceToken(match[0]);
    if (!payload) continue;
    matches.push({
      token: match[0],
      start: match.index,
      end: match.index + match[0].length,
      payload,
    });
  }

  return matches;
}

export function appendAdditionalModePromptReferenceToken(
  text: string,
  id: AdditionalModePromptReferenceId,
): string {
  const token = createAdditionalModePromptReferenceToken(id);
  const trimmed = text.trimEnd();
  return trimmed ? `${trimmed} ${token}` : token;
}

/**
 * Adapts a product-level mode capsule to the native command boundary.
 * The capsule may sit anywhere in the editor; the command must lead the
 * submitted text so the existing Review parser remains the runtime owner.
 */
export function expandAdditionalModePromptReferenceTokens(text: string): string {
  const selectedMode = getAdditionalModePromptReferenceMatches(text)[0]?.payload;
  if (!selectedMode) {
    return text;
  }

  TOKEN_PATTERN.lastIndex = 0;
  const remainingText = text.replace(TOKEN_PATTERN, (token) => {
    const payload = parseAdditionalModePromptReferenceToken(token);
    if (!payload) return token;
    return ' ';
  }).trim();

  return remainingText
    ? `${selectedMode.promptCommand} ${remainingText}`
    : selectedMode.promptCommand;
}
