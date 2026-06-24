const SKILL_TOKEN_PATTERN = /\[\$([^\]\r\n]+)\]/g;
const LEADING_SLASH_COMMAND_PATTERN = /^(\s*)\/[A-Za-z][\w:-]*/;
const SLASH_ADDRESSABLE_SKILL_NAME_PATTERN = /^[A-Za-z][\w:-]*$/;

export interface SkillPromptReferenceTokenPayload {
  skillName: string;
}

export function createSkillPromptReferenceToken(skillName: string): string {
  return `[$${skillName.trim()}]`;
}

export function parseSkillPromptReferenceToken(
  token: string,
): SkillPromptReferenceTokenPayload | null {
  const match = token.match(/^\[\$([^\]\r\n]+)\]$/);
  const skillName = match?.[1]?.trim();
  if (!skillName) {
    return null;
  }
  return { skillName };
}

export function getSkillPromptReferenceMatches(text: string): Array<{
  token: string;
  start: number;
  end: number;
  payload: SkillPromptReferenceTokenPayload;
}> {
  const matches: Array<{
    token: string;
    start: number;
    end: number;
    payload: SkillPromptReferenceTokenPayload;
  }> = [];

  SKILL_TOKEN_PATTERN.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = SKILL_TOKEN_PATTERN.exec(text)) !== null) {
    const payload = parseSkillPromptReferenceToken(match[0]);
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

export function appendSkillPromptReferenceToken(
  text: string,
  skillName: string,
): string {
  const token = createSkillPromptReferenceToken(skillName);
  const trimmed = text.trimEnd();
  return trimmed ? `${trimmed} ${token}` : token;
}

export function replaceLeadingSlashCommandWithSkillToken(
  text: string,
  skillName: string,
): string {
  const token = createSkillPromptReferenceToken(skillName);
  if (!text.trimStart().startsWith('/')) {
    return appendSkillPromptReferenceToken(text, skillName);
  }

  return text.replace(LEADING_SLASH_COMMAND_PATTERN, `${'$1'}${token}`);
}

export function isSlashAddressableSkillName(skillName: string): boolean {
  return SLASH_ADDRESSABLE_SKILL_NAME_PATTERN.test(skillName.trim());
}
