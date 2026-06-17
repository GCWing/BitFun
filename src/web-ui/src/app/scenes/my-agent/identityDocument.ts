import yaml from 'yaml';

export interface IdentityDocument {
  name: string;
  creature: string;
  vibe: string;
  emoji: string;
  body: string;
  /** Override for primary model slot. Empty string = inherit from template. */
  modelPrimary?: string;
  /** Override for fast model slot. Empty string = inherit from template. */
  modelFast?: string;
}

export const EMPTY_IDENTITY_DOCUMENT: IdentityDocument = {
  name: '',
  creature: '',
  vibe: '',
  emoji: '',
  body: '',
  modelPrimary: '',
  modelFast: '',
};

const FRONTMATTER_FIELDS: Array<keyof Omit<IdentityDocument, 'body'>> = [
  'name',
  'creature',
  'vibe',
  'emoji',
  'modelPrimary',
  'modelFast',
];

export interface MarkdownFrontmatterSections {
  hasFrontmatter: boolean;
  frontmatter: string;
  body: string;
}

function normalizeLineEndings(content: string): string {
  return content.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
}

function normalizeShortField(value: unknown): string {
  if (typeof value !== 'string') {
    return '';
  }

  return value.replace(/\s+/g, ' ').trim();
}

function serializeScalar(value: string): string {
  return yaml.stringify(value).trimEnd();
}

export function splitMarkdownFrontmatter(content: string): MarkdownFrontmatterSections {
  const normalizedContent = normalizeLineEndings(content || '');
  const frontmatterMatch = normalizedContent.match(/^---\n([\s\S]*?)\n---\n?([\s\S]*)$/);

  if (!frontmatterMatch) {
    return {
      hasFrontmatter: false,
      frontmatter: '',
      body: normalizedContent.trimEnd(),
    };
  }

  return {
    hasFrontmatter: true,
    frontmatter: frontmatterMatch[1] ?? '',
    body: (frontmatterMatch[2] ?? '').replace(/^\n+/, '').trimEnd(),
  };
}

export function joinMarkdownFrontmatter(
  frontmatter: string,
  body: string,
  options?: { preserveFrontmatterBlock?: boolean }
): string {
  const normalizedFrontmatter = normalizeLineEndings(frontmatter || '')
    .replace(/^\n+/, '')
    .trimEnd();
  const normalizedBody = normalizeLineEndings(body || '')
    .replace(/^\n+/, '')
    .trimEnd();

  if (!normalizedFrontmatter && !options?.preserveFrontmatterBlock) {
    return normalizedBody ? `${normalizedBody}\n` : '';
  }

  return `---\n${normalizedFrontmatter}\n---\n\n${normalizedBody}`.trimEnd() + '\n';
}

export function parseIdentityDocument(content: string): IdentityDocument {
  const sections = splitMarkdownFrontmatter(content);
  if (!sections.hasFrontmatter) {
    return {
      ...EMPTY_IDENTITY_DOCUMENT,
      body: sections.body.trim(),
    };
  }

  const parsed = (yaml.parse(sections.frontmatter) || {}) as Record<string, unknown>;

  return {
    name: normalizeShortField(parsed.name),
    creature: normalizeShortField(parsed.creature),
    vibe: normalizeShortField(parsed.vibe),
    emoji: normalizeShortField(parsed.emoji),
    body: sections.body,
    modelPrimary: normalizeShortField(parsed.modelPrimary),
    modelFast: normalizeShortField(parsed.modelFast),
  };
}

export function serializeIdentityDocument(document: IdentityDocument): string {
  const normalized = {
    name: normalizeShortField(document.name),
    creature: normalizeShortField(document.creature),
    vibe: normalizeShortField(document.vibe),
    emoji: normalizeShortField(document.emoji),
    body: normalizeLineEndings(document.body || '').replace(/^\n+/, '').trimEnd(),
    modelPrimary: normalizeShortField(document.modelPrimary ?? ''),
    modelFast: normalizeShortField(document.modelFast ?? ''),
  };

  const optionalFields = new Set<keyof Omit<IdentityDocument, 'body'>>(['modelPrimary', 'modelFast']);
  const frontmatter = FRONTMATTER_FIELDS
    .filter((field) => {
      if (optionalFields.has(field)) return !!normalized[field];
      return true;
    })
    .map((field) => {
      const value = normalized[field];
      return value ? `${field}: ${serializeScalar(value)}` : `${field}:`;
    })
    .join('\n');

  return joinMarkdownFrontmatter(frontmatter, normalized.body);
}

export function getIdentityFilePath(workspaceRoot: string): string {
  const normalizedRoot = workspaceRoot.replace(/\\/g, '/').replace(/\/+$/, '');
  return `${normalizedRoot}/IDENTITY.md`;
}
