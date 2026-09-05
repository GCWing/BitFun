import type { ContextItem, SessionReferenceContext } from '@/shared/types/context';
import type { TurnRailCapsulePreview } from '@/shared/types/session-history';

export const COMPOSER_PRESENTATION_VERSION = 1;

export type ComposerPresentationSegment =
  | {
      kind: 'text';
      text: string;
    }
  | {
      kind: 'context';
      context: ContextItem;
      tag: string;
      label: string;
      title: string;
    }
  | {
      kind: 'inline-token';
      token: string;
      tokenType: 'skill' | 'widget';
      label: string;
    };

export interface ComposerPresentation {
  version: typeof COMPOSER_PRESENTATION_VERSION;
  segments: ComposerPresentationSegment[];
}

const CONTEXT_TYPES = new Set<ContextItem['type']>([
  'file',
  'directory',
  'session-reference',
  'code-snippet',
  'pull-request',
  'mermaid-node',
  'mermaid-diagram',
  'image',
  'terminal-command',
  'git-ref',
  'url',
  'web-element',
]);

function trimComposerText(text: string): string {
  return text.startsWith('/')
    ? text.replace(/^[\r\n]+/, '').replace(/[\r\n]+$/, '')
    : text.trim();
}

function normalizePromptText(text: string): string {
  return trimComposerText(
    text
      .replace(/[ \t]+\n/g, '\n')
      .replace(/\n{3,}/g, '\n\n')
      .replace(/[ \t]{2,}/g, ' '),
  );
}

export function appendComposerTextSegment(
  segments: ComposerPresentationSegment[],
  text: string,
): void {
  if (!text) {
    return;
  }

  const previous = segments[segments.length - 1];
  if (previous?.kind === 'text') {
    previous.text += text;
    return;
  }

  segments.push({ kind: 'text', text });
}

export function hasComposerPresentationReferences(
  presentation: ComposerPresentation | null | undefined,
): presentation is ComposerPresentation {
  return Boolean(
    presentation?.segments.some(segment => segment.kind !== 'text'),
  );
}

/** Reduce a full presentation to the read-only facts needed by Turn Rail. */
export function composerPresentationToTurnRailPreview(
  presentation: ComposerPresentation | null | undefined,
): TurnRailCapsulePreview | undefined {
  if (!presentation || !hasComposerPresentationReferences(presentation)) return undefined;
  const segments = presentation.segments.map(segment => {
    if (segment.kind === 'text') return { kind: 'text' as const, text: segment.text };
    if (segment.kind === 'inline-token') {
      return { kind: 'inlineToken' as const, tokenType: segment.tokenType, label: segment.label };
    }
    return {
      kind: 'context' as const,
      contextType: segment.context.type,
      label: segment.label,
      ...(segment.title ? { title: segment.title } : {}),
    };
  });
  return segments.length > 0 ? { segments } : undefined;
}

export function composerPresentationToEditorText(
  presentation: ComposerPresentation,
): string {
  return trimComposerText(
    presentation.segments.map(segment => {
      if (segment.kind === 'text') {
        return segment.text;
      }
      return segment.kind === 'context' ? segment.tag : segment.token;
    }).join(''),
  );
}

/**
 * Builds the text visible to the model. Session references are delivered through
 * their dedicated metadata contract, so their presentation capsules never become
 * ambiguous user instructions in the model prompt.
 */
export function composerPresentationToModelText(
  presentation: ComposerPresentation,
): string {
  let sessionReferenceIndex = 0;
  return normalizePromptText(
    presentation.segments.map(segment => {
      if (segment.kind === 'text') {
        return segment.text;
      }
      if (segment.kind === 'context' && segment.context.type === 'session-reference') {
        sessionReferenceIndex += 1;
        return `[session-ref:${sessionReferenceIndex}]`;
      }
      return segment.kind === 'context' ? segment.tag : segment.token;
    }).join(''),
  );
}

export function composerPresentationToAccessibleText(
  presentation: ComposerPresentation,
): string {
  return trimComposerText(
    presentation.segments.map(segment => {
      if (segment.kind === 'text') {
        return segment.text;
      }
      if (segment.kind === 'inline-token') {
        return `[${segment.tokenType === 'skill' ? 'Skill' : 'Widget'}: ${segment.label}]`;
      }
      const type = segment.context.type === 'session-reference'
        ? 'Session reference'
        : 'Context';
      return `[${type}: ${segment.label}]`;
    }).join(''),
  );
}

export function composerPresentationContexts(
  presentation: ComposerPresentation,
): ContextItem[] {
  const contexts = new Map<string, ContextItem>();
  for (const segment of presentation.segments) {
    if (segment.kind === 'context' && segment.context.type !== 'image') {
      contexts.set(segment.context.id, segment.context);
    }
  }
  return [...contexts.values()];
}

/** Returns session locators in the same order as their prompt markers. */
export function composerPresentationSessionReferences(
  presentation: ComposerPresentation,
): SessionReferenceContext[] {
  return presentation.segments.flatMap(segment => (
    segment.kind === 'context' && segment.context.type === 'session-reference'
      ? [segment.context]
      : []
  ));
}

function isContextLike(value: unknown): value is ContextItem {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const context = value as Record<string, unknown>;
  return (
    typeof context.id === 'string' &&
    typeof context.type === 'string' &&
    CONTEXT_TYPES.has(context.type as ContextItem['type'])
  );
}

/**
 * Metadata can arrive from persisted or remote history. Only accept the small,
 * versioned shape needed to render a non-interactive message capsule.
 */
export function parseComposerPresentation(value: unknown): ComposerPresentation | null {
  if (!value || typeof value !== 'object') {
    return null;
  }

  const candidate = value as Record<string, unknown>;
  if (candidate.version !== COMPOSER_PRESENTATION_VERSION || !Array.isArray(candidate.segments)) {
    return null;
  }

  const segments: ComposerPresentationSegment[] = [];
  for (const rawSegment of candidate.segments) {
    if (!rawSegment || typeof rawSegment !== 'object') {
      return null;
    }
    const segment = rawSegment as Record<string, unknown>;
    if (segment.kind === 'text' && typeof segment.text === 'string') {
      segments.push({ kind: 'text', text: segment.text });
      continue;
    }
    if (
      segment.kind === 'context' &&
      isContextLike(segment.context) &&
      typeof segment.tag === 'string' &&
      typeof segment.label === 'string' &&
      typeof segment.title === 'string'
    ) {
      segments.push({
        kind: 'context',
        context: segment.context,
        tag: segment.tag,
        label: segment.label,
        title: segment.title,
      });
      continue;
    }
    if (
      segment.kind === 'inline-token' &&
      typeof segment.token === 'string' &&
      (segment.tokenType === 'skill' || segment.tokenType === 'widget') &&
      typeof segment.label === 'string'
    ) {
      segments.push({
        kind: 'inline-token',
        token: segment.token,
        tokenType: segment.tokenType,
        label: segment.label,
      });
      continue;
    }
    return null;
  }

  return { version: COMPOSER_PRESENTATION_VERSION, segments };
}
