import type { AppearanceValidationIssue } from '../types';

export interface AppearanceValidationIssueGroup {
  key: string;
  surfaceKind?: 'component' | 'scene';
  surfaceId?: string;
  section: string;
  issues: readonly AppearanceValidationIssue[];
  allowedParts: readonly string[];
}

function inferIssueLocation(issue: AppearanceValidationIssue): {
  surfaceKind?: 'component' | 'scene';
  surfaceId?: string;
  section: string;
} {
  if (issue.context?.surfaceKind && issue.context.surfaceId) {
    return {
      surfaceKind: issue.context.surfaceKind,
      surfaceId: issue.context.surfaceId,
      section: issue.context.surfaceKind === 'component' ? 'components' : 'scenes',
    };
  }
  const [section, surfaceId] = issue.path.split('.');
  if ((section === 'components' || section === 'scenes') && surfaceId) {
    return {
      surfaceKind: section === 'components' ? 'component' : 'scene',
      surfaceId,
      section,
    };
  }
  return { section: section && section !== '$' ? section : 'package' };
}

export function groupAppearanceValidationIssues(
  issues: readonly AppearanceValidationIssue[],
): AppearanceValidationIssueGroup[] {
  const groups = new Map<string, {
    surfaceKind?: 'component' | 'scene';
    surfaceId?: string;
    section: string;
    issues: AppearanceValidationIssue[];
    allowedParts: Set<string>;
  }>();

  issues.forEach(issue => {
    const location = inferIssueLocation(issue);
    const key = location.surfaceKind && location.surfaceId
      ? `${location.surfaceKind}:${location.surfaceId}`
      : `section:${location.section}`;
    const group = groups.get(key) ?? {
      ...location,
      issues: [],
      allowedParts: new Set<string>(),
    };
    group.issues.push(issue);
    issue.context?.allowedParts?.forEach(part => group.allowedParts.add(part));
    groups.set(key, group);
  });

  return [...groups.entries()].map(([key, group]) => Object.freeze({
    key,
    surfaceKind: group.surfaceKind,
    surfaceId: group.surfaceId,
    section: group.section,
    issues: Object.freeze([...group.issues]),
    allowedParts: Object.freeze([...group.allowedParts]),
  }));
}

function formatValidationMessage(issues: readonly AppearanceValidationIssue[]): string {
  const groups = groupAppearanceValidationIssues(issues);
  const details = groups.map(group => {
    const label = group.surfaceKind && group.surfaceId
      ? `${group.surfaceKind} ${group.surfaceId}`
      : group.section;
    return `${label}:\n${group.issues.map(issue => `  - [${issue.code}] ${issue.path}: ${issue.message}`).join('\n')}`;
  }).join('\n');
  return `Appearance package validation failed with ${issues.length} issue${issues.length === 1 ? '' : 's'}:\n${details}`;
}

export class AppearancePackageValidationError extends Error {
  readonly code = 'APPEARANCE_PACKAGE_VALIDATION_FAILED';
  readonly issues: readonly AppearanceValidationIssue[];
  readonly groups: readonly AppearanceValidationIssueGroup[];

  constructor(issues: readonly AppearanceValidationIssue[]) {
    super(formatValidationMessage(issues));
    this.name = 'AppearancePackageValidationError';
    this.issues = Object.freeze(issues.map(issue => {
      const context = issue.context ? Object.freeze({
        ...issue.context,
        ...(issue.context.allowedParts
          ? { allowedParts: Object.freeze([...issue.context.allowedParts]) }
          : {}),
      }) : undefined;
      return Object.freeze({
        ...issue,
        ...(context ? { context } : {}),
      });
    }));
    this.groups = Object.freeze(groupAppearanceValidationIssues(this.issues));
  }
}

export function getAppearancePackageValidationError(error: unknown): AppearancePackageValidationError | null {
  return error instanceof AppearancePackageValidationError ? error : null;
}
