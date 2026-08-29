export const DEEP_RESEARCH_PHASE_IDS = [
  'phase-0-orient',
  'phase-1-specialists',
  'phase-2-citations',
  'phase-3-debate-r1',
  'phase-3-debate-r2',
  'phase-4-factcheck',
  'phase-5-arbitration',
  'phase-6-report',
  'complete',
] as const;

export type DeepResearchPhaseId = typeof DEEP_RESEARCH_PHASE_IDS[number];
export type DeepResearchAuthority = 'high' | 'medium' | 'low';
export type DeepResearchVerdictStatus = 'DECIDED' | 'CONTESTED' | 'GAP' | 'TENTATIVE';

export type DeepResearchProtocolMarker =
  | {
      kind: 'phase';
      phaseId: DeepResearchPhaseId;
      raw: string;
    }
  | {
      kind: 'subquestion';
      id: string;
      title: string;
      parentId: string;
      raw: string;
    }
  | {
      kind: 'citation';
      id: string;
      authority: DeepResearchAuthority;
      corroborated: boolean;
      url: string;
      raw: string;
    }
  | {
      kind: 'verdict';
      subquestionId: string;
      status: DeepResearchVerdictStatus;
      confidence: number;
      raw: string;
    };

export type DeepResearchContentSegment =
  | { type: 'markdown'; content: string }
  | { type: 'protocol'; kind: DeepResearchProtocolMarker['kind']; markers: DeepResearchProtocolMarker[] };

export interface ParsedDeepResearchContent {
  hasProtocol: boolean;
  segments: DeepResearchContentSegment[];
}

const MARKER_NAMES = ['PHASE', 'SUBQ', 'CITATION', 'VERDICT'] as const;
const MARKER_PATTERN = /\[\[(PHASE|SUBQ|CITATION|VERDICT):([^\]\r\n]*)\]\]/g;
const PHASE_IDS = new Set<string>(DEEP_RESEARCH_PHASE_IDS);
const AUTHORITIES = new Set<DeepResearchAuthority>(['high', 'medium', 'low']);
const VERDICT_STATUSES = new Set<DeepResearchVerdictStatus>([
  'DECIDED',
  'CONTESTED',
  'GAP',
  'TENTATIVE',
]);

function nonEmpty(value: string | undefined): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

function parseMarker(kind: typeof MARKER_NAMES[number], payload: string, raw: string): DeepResearchProtocolMarker | null {
  if (kind === 'PHASE') {
    const phaseId = payload.trim();
    if (!PHASE_IDS.has(phaseId)) return null;
    return { kind: 'phase', phaseId: phaseId as DeepResearchPhaseId, raw };
  }

  const parts = payload.split('|').map(part => part.trim());

  if (kind === 'SUBQ') {
    if (parts.length < 3) return null;
    const id = parts.shift();
    const parentId = parts.pop();
    const title = parts.join('|').trim();
    if (!nonEmpty(id) || !nonEmpty(title) || !nonEmpty(parentId)) return null;
    return { kind: 'subquestion', id, title, parentId, raw };
  }

  if (kind === 'CITATION') {
    if (parts.length < 4) return null;
    const id = parts.shift();
    const authority = parts.shift();
    const corroborated = parts.shift();
    const url = parts.join('|').trim();
    if (
      !nonEmpty(id)
      || !AUTHORITIES.has(authority as DeepResearchAuthority)
      || (corroborated !== 'true' && corroborated !== 'false')
      || !nonEmpty(url)
    ) {
      return null;
    }
    return {
      kind: 'citation',
      id,
      authority: authority as DeepResearchAuthority,
      corroborated: corroborated === 'true',
      url,
      raw,
    };
  }

  if (parts.length !== 3) return null;
  const [subquestionId, status, confidenceText] = parts;
  const confidence = Number(confidenceText);
  if (
    !nonEmpty(subquestionId)
    || !VERDICT_STATUSES.has(status as DeepResearchVerdictStatus)
    || !nonEmpty(confidenceText)
    || !Number.isFinite(confidence)
    || confidence < 0
    || confidence > 1
  ) {
    return null;
  }
  return {
    kind: 'verdict',
    subquestionId,
    status: status as DeepResearchVerdictStatus,
    confidence,
    raw,
  };
}

function splitPendingMarkerSuffix(content: string): { content: string; hasPendingMarker: boolean } {
  const openIndex = content.lastIndexOf('[[');
  if (openIndex < 0 || content.indexOf(']]', openIndex) >= 0) {
    return { content, hasPendingMarker: false };
  }

  const suffix = content.slice(openIndex + 2);
  if (suffix.includes('\n') || suffix.includes('\r')) {
    return { content, hasPendingMarker: false };
  }

  const normalized = suffix.toUpperCase();
  const isProtocolPrefix = MARKER_NAMES.some(name => (
    name.startsWith(normalized) || normalized.startsWith(`${name}:`)
  ));
  if (!isProtocolPrefix) {
    return { content, hasPendingMarker: false };
  }

  return {
    content: content.slice(0, openIndex),
    hasPendingMarker: true,
  };
}

function trimProtocolBoundaryWhitespace(content: string): string {
  return content
    .replace(/^(?:[\t ]*\r?\n)+/, '')
    .replace(/(?:\r?\n[\t ]*)+$/, '');
}

export function parseDeepResearchContent(content: string): ParsedDeepResearchContent {
  const pending = splitPendingMarkerSuffix(content);
  const visibleContent = pending.content;
  const matches: Array<{ start: number; end: number; marker: DeepResearchProtocolMarker }> = [];

  MARKER_PATTERN.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = MARKER_PATTERN.exec(visibleContent)) !== null) {
    const marker = parseMarker(
      match[1] as typeof MARKER_NAMES[number],
      match[2],
      match[0],
    );
    if (marker) {
      matches.push({
        start: match.index,
        end: match.index + match[0].length,
        marker,
      });
    }
  }

  if (matches.length === 0) {
    return {
      hasProtocol: pending.hasPendingMarker,
      segments: visibleContent ? [{ type: 'markdown', content: visibleContent }] : [],
    };
  }

  const segments: DeepResearchContentSegment[] = [];
  let cursor = 0;
  let markerRun: DeepResearchProtocolMarker[] = [];

  const flushMarkerRun = () => {
    if (markerRun.length === 0) return;
    segments.push({
      type: 'protocol',
      kind: markerRun[0].kind,
      markers: markerRun,
    });
    markerRun = [];
  };

  const pushMarkdown = (value: string) => {
    const normalized = trimProtocolBoundaryWhitespace(value);
    if (normalized.length > 0) {
      segments.push({ type: 'markdown', content: normalized });
    }
  };

  for (const current of matches) {
    const between = visibleContent.slice(cursor, current.start);
    const canJoinRun = markerRun.length > 0
      && between.trim().length === 0
      && markerRun[0].kind === current.marker.kind;

    if (!canJoinRun) {
      flushMarkerRun();
      pushMarkdown(between);
    }

    markerRun.push(current.marker);
    cursor = current.end;
  }

  flushMarkerRun();
  pushMarkdown(visibleContent.slice(cursor));

  return {
    hasProtocol: true,
    segments,
  };
}
