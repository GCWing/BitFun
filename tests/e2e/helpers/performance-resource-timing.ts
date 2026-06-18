import { browser } from '@wdio/globals';

export const MAX_RESOURCE_TIMING_ENTRIES = 40;

type BrowserResourceTimingEntry = {
  name: string;
  initiatorType: string;
  startTime: number;
  duration: number;
  transferSize?: number;
  encodedBodySize?: number;
  decodedBodySize?: number;
  renderBlockingStatus?: string;
};

export type StartupResourceTimingEntry = {
  name: string;
  initiatorType: string;
  startTimeMs: number;
  durationMs: number;
  transferSize?: number;
  encodedBodySize?: number;
  decodedBodySize?: number;
  renderBlockingStatus?: string;
};

export type StartupResourceTimingSummary = {
  totalCount: number;
  sampledCount: number;
  cutoffMs?: number;
  byInitiatorType: Array<{
    initiatorType: string;
    count: number;
    totalDurationMs: number;
    maxDurationMs: number;
    totalDecodedBodySize: number;
  }>;
  topDuration: StartupResourceTimingEntry[];
  topDecodedBodySize: StartupResourceTimingEntry[];
};

function round(value: number): number {
  return Math.round(value * 10) / 10;
}

export function sanitizeResourceTimingName(name: string): string {
  try {
    const url = new URL(name);
    const path = url.pathname || url.hostname || name;
    return path.slice(0, 180);
  } catch {
    return name.replace(/\\/g, '/').split('/').slice(-2).join('/').slice(0, 180);
  }
}

function normalizeEntry(entry: BrowserResourceTimingEntry): StartupResourceTimingEntry {
  return {
    name: sanitizeResourceTimingName(entry.name),
    initiatorType: entry.initiatorType || 'unknown',
    startTimeMs: round(entry.startTime),
    durationMs: round(entry.duration),
    transferSize: typeof entry.transferSize === 'number' ? entry.transferSize : undefined,
    encodedBodySize: typeof entry.encodedBodySize === 'number' ? entry.encodedBodySize : undefined,
    decodedBodySize: typeof entry.decodedBodySize === 'number' ? entry.decodedBodySize : undefined,
    renderBlockingStatus:
      typeof entry.renderBlockingStatus === 'string' ? entry.renderBlockingStatus : undefined,
  };
}

export function summarizeStartupResourceTiming(
  entries: BrowserResourceTimingEntry[],
  cutoffMs?: number,
): StartupResourceTimingSummary {
  const filtered = entries
    .filter(entry => Number.isFinite(entry.startTime) && Number.isFinite(entry.duration))
    .filter(entry => cutoffMs === undefined || entry.startTime <= cutoffMs);
  const normalized = filtered.map(normalizeEntry);
  const byInitiator = new Map<string, StartupResourceTimingSummary['byInitiatorType'][number]>();

  for (const entry of normalized) {
    const existing = byInitiator.get(entry.initiatorType) ?? {
      initiatorType: entry.initiatorType,
      count: 0,
      totalDurationMs: 0,
      maxDurationMs: 0,
      totalDecodedBodySize: 0,
    };
    existing.count += 1;
    existing.totalDurationMs = round(existing.totalDurationMs + entry.durationMs);
    existing.maxDurationMs = Math.max(existing.maxDurationMs, entry.durationMs);
    existing.totalDecodedBodySize += entry.decodedBodySize ?? 0;
    byInitiator.set(entry.initiatorType, existing);
  }

  return {
    totalCount: filtered.length,
    sampledCount: Math.min(normalized.length, MAX_RESOURCE_TIMING_ENTRIES),
    cutoffMs: cutoffMs === undefined ? undefined : round(cutoffMs),
    byInitiatorType: Array.from(byInitiator.values()).sort(
      (left, right) => right.totalDurationMs - left.totalDurationMs
    ),
    topDuration: [...normalized]
      .sort((left, right) => right.durationMs - left.durationMs)
      .slice(0, MAX_RESOURCE_TIMING_ENTRIES),
    topDecodedBodySize: [...normalized]
      .sort((left, right) => (right.decodedBodySize ?? 0) - (left.decodedBodySize ?? 0))
      .slice(0, MAX_RESOURCE_TIMING_ENTRIES),
  };
}

export async function readStartupResourceTimingSummary(
  cutoffMs?: number,
): Promise<StartupResourceTimingSummary> {
  const entries = await browser.execute(() =>
    performance.getEntriesByType('resource').map(entry => {
      const resource = entry as PerformanceResourceTiming;
      return {
        name: resource.name,
        initiatorType: resource.initiatorType,
        startTime: resource.startTime,
        duration: resource.duration,
        transferSize: resource.transferSize,
        encodedBodySize: resource.encodedBodySize,
        decodedBodySize: resource.decodedBodySize,
        renderBlockingStatus: (resource as PerformanceResourceTiming & {
          renderBlockingStatus?: string;
        }).renderBlockingStatus,
      };
    })
  );

  return summarizeStartupResourceTiming(entries, cutoffMs);
}
