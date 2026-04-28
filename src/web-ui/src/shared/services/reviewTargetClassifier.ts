export type ReviewTargetSource =
  | 'session_files'
  | 'slash_command_explicit_files'
  | 'slash_command_git_ref'
  | 'workspace_diff'
  | 'manual_prompt'
  | 'unknown';

export type ReviewDomainTag =
  | 'frontend_ui'
  | 'frontend_style'
  | 'frontend_i18n'
  | 'frontend_contract'
  | 'desktop_contract'
  | 'web_server_contract'
  | 'backend_core'
  | 'transport'
  | 'api_layer'
  | 'ai_adapter'
  | 'installer_ui'
  | 'test'
  | 'docs'
  | 'config'
  | 'generated_or_lock'
  | 'unknown';

export interface ReviewTargetFile {
  path: string;
  normalizedPath: string;
  oldPath?: string;
  normalizedOldPath?: string;
  status: 'added' | 'modified' | 'deleted' | 'renamed' | 'copied' | 'unknown';
  source: ReviewTargetSource;
  tags: ReviewDomainTag[];
  excluded?: boolean;
  excludeReason?: 'lockfile' | 'generated' | 'binary' | 'too_large' | 'unsupported';
}

export interface ReviewTargetWarning {
  code:
    | 'target_unknown'
    | 'git_ref_unresolved'
    | 'file_list_empty'
    | 'remote_resolution_unavailable'
    | 'excluded_files_present'
    | 'contract_surface_detected'
    | 'classification_partial';
  message: string;
}

export interface ReviewTargetClassification {
  source: ReviewTargetSource;
  resolution: 'resolved' | 'partial' | 'unknown';
  files: ReviewTargetFile[];
  tags: ReviewDomainTag[];
  evidence: string[];
  warnings: ReviewTargetWarning[];
}

interface PathTagRule {
  id: string;
  tags: ReviewDomainTag[];
  match: {
    pathPrefixes?: string[];
    extensions?: string[];
    exactFiles?: string[];
  };
  evidence: string;
}

export const FRONTEND_REVIEW_DOMAIN_TAGS: ReviewDomainTag[] = [
  'frontend_ui',
  'frontend_style',
  'frontend_i18n',
  'frontend_contract',
  'desktop_contract',
  'web_server_contract',
];

const PATH_TAG_RULES: PathTagRule[] = [
  {
    id: 'web-ui-locales',
    tags: ['frontend_i18n'],
    match: { pathPrefixes: ['src/web-ui/src/locales/'] },
    evidence: 'Frontend locale file changed',
  },
  {
    id: 'web-ui-style',
    tags: ['frontend_style'],
    match: {
      pathPrefixes: ['src/web-ui/'],
      extensions: ['.scss', '.css', '.sass', '.less'],
    },
    evidence: 'Frontend stylesheet changed',
  },
  {
    id: 'web-ui-source',
    tags: ['frontend_ui'],
    match: {
      pathPrefixes: ['src/web-ui/src/'],
      extensions: ['.ts', '.tsx', '.js', '.jsx'],
    },
    evidence: 'File is under src/web-ui/src',
  },
  {
    id: 'desktop-api-contract',
    tags: ['desktop_contract', 'frontend_contract'],
    match: { pathPrefixes: ['src/apps/desktop/src/api/'] },
    evidence: 'Desktop API surface may affect frontend invoke contract',
  },
  {
    id: 'api-layer-contract',
    tags: ['api_layer', 'frontend_contract'],
    match: { pathPrefixes: ['src/crates/api-layer/'] },
    evidence: 'API layer may affect frontend/backend contract',
  },
  {
    id: 'server-contract',
    tags: ['web_server_contract', 'frontend_contract'],
    match: { pathPrefixes: ['src/apps/server/src/routes/'] },
    evidence: 'Server route surface may affect frontend communication contract',
  },
  {
    id: 'transport',
    tags: ['transport'],
    match: { pathPrefixes: ['src/crates/transport/'] },
    evidence: 'Transport layer changed',
  },
  {
    id: 'core',
    tags: ['backend_core'],
    match: { pathPrefixes: ['src/crates/core/'] },
    evidence: 'Core product logic changed',
  },
  {
    id: 'ai-adapter',
    tags: ['ai_adapter'],
    match: { pathPrefixes: ['src/crates/ai-adapters/'] },
    evidence: 'AI adapter changed',
  },
  {
    id: 'installer-ui',
    tags: ['installer_ui'],
    match: { pathPrefixes: ['BitFun-Installer/'] },
    evidence: 'Installer UI changed',
  },
  {
    id: 'docs',
    tags: ['docs'],
    match: {
      pathPrefixes: ['docs/'],
      extensions: ['.md'],
    },
    evidence: 'Documentation changed',
  },
  {
    id: 'lockfile',
    tags: ['generated_or_lock'],
    match: {
      exactFiles: ['pnpm-lock.yaml', 'package-lock.json', 'yarn.lock', 'Cargo.lock'],
    },
    evidence: 'Lockfile changed',
  },
];

export function normalizeReviewPath(path: string): string {
  return path.trim().replace(/\\/g, '/').replace(/^\.\/+/, '');
}

function dedupe<T>(values: T[]): T[] {
  return Array.from(new Set(values));
}

function getExtension(path: string): string {
  const lastSlash = path.lastIndexOf('/');
  const lastDot = path.lastIndexOf('.');
  if (lastDot <= lastSlash) {
    return '';
  }
  return path.slice(lastDot);
}

function matchesRule(path: string, rule: PathTagRule): boolean {
  const { pathPrefixes, extensions, exactFiles } = rule.match;
  const extension = getExtension(path);
  return Boolean(
    exactFiles?.includes(path) ||
      pathPrefixes?.some((prefix) => path.startsWith(prefix)) &&
        (!extensions || extensions.includes(extension)) ||
      !pathPrefixes &&
        extensions?.includes(extension),
  );
}

function inferSupplementalTags(path: string): ReviewDomainTag[] {
  const tags: ReviewDomainTag[] = [];
  if (
    path.includes('/tests/') ||
    path.endsWith('.test.ts') ||
    path.endsWith('.test.tsx') ||
    path.endsWith('.spec.ts') ||
    path.endsWith('.spec.tsx')
  ) {
    tags.push('test');
  }
  if (
    path === 'package.json' ||
    path.endsWith('/package.json') ||
    path.endsWith('.config.ts') ||
    path.endsWith('.config.js') ||
    path.startsWith('.github/workflows/')
  ) {
    tags.push('config');
  }
  return tags;
}

function classifyPath(
  originalPath: string,
  source: ReviewTargetSource,
): { file: ReviewTargetFile; evidence: string[] } {
  const normalizedPath = normalizeReviewPath(originalPath);
  const matchedRules = PATH_TAG_RULES.filter((rule) =>
    matchesRule(normalizedPath, rule),
  );
  const ruleTags = matchedRules.flatMap((rule) => rule.tags);
  const tags = dedupe([...ruleTags, ...inferSupplementalTags(normalizedPath)]);
  const finalTags = tags.length > 0 ? tags : ['unknown' as const];

  return {
    file: {
      path: originalPath,
      normalizedPath,
      status: 'unknown',
      source,
      tags: finalTags,
    },
    evidence: matchedRules.map((rule) => rule.evidence),
  };
}

export function createUnknownReviewTargetClassification(
  source: ReviewTargetSource,
): ReviewTargetClassification {
  return {
    source,
    resolution: 'unknown',
    files: [],
    tags: ['unknown'],
    evidence: ['Review target could not be resolved before launch.'],
    warnings: [
      {
        code: 'target_unknown',
        message: 'Review target could not be resolved before launch.',
      },
    ],
  };
}

export function classifyReviewTargetFromFiles(
  filePaths: string[],
  source: ReviewTargetSource,
): ReviewTargetClassification {
  const normalizedInputs = filePaths
    .map((path) => path.trim())
    .filter(Boolean);

  if (normalizedInputs.length === 0) {
    return {
      ...createUnknownReviewTargetClassification(source),
      warnings: [
        {
          code: 'file_list_empty',
          message: 'No reviewable files were provided for target classification.',
        },
      ],
    };
  }

  const classified = normalizedInputs.map((path) => classifyPath(path, source));
  const files = classified.map((item) => item.file);
  const tags = dedupe(files.flatMap((file) => file.tags));
  const hasUnknown = tags.includes('unknown');
  const hasKnown = tags.some((tag) => tag !== 'unknown');
  const resolution = hasUnknown ? (hasKnown ? 'partial' : 'unknown') : 'resolved';
  const warnings: ReviewTargetWarning[] = [];

  if (resolution === 'partial') {
    warnings.push({
      code: 'classification_partial',
      message: 'Some review target files could not be classified.',
    });
  }

  if (tags.includes('frontend_contract')) {
    warnings.push({
      code: 'contract_surface_detected',
      message: 'A frontend-facing contract surface changed.',
    });
  }

  return {
    source,
    resolution,
    files,
    tags,
    evidence: dedupe(classified.flatMap((item) => item.evidence)),
    warnings,
  };
}
