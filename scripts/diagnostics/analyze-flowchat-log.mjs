/**
 * Read `flowchat.log` and answer the questions the viewport trail exists for.
 *
 * The log is JSONL, one object per entry, written by
 * `src/web-ui/src/infrastructure/diagnostics/flowChatDiagnostics.ts` behind
 * `app.logging.flow_chat_diagnostics`. Viewport entries carry the tag
 * `viewport`; history paging carries `history-paging`.
 *
 * The reports here are not a frequency count with extra steps. Each one
 * corresponds to a fault that has actually shipped:
 *
 * - **Placements that did not stick.** A Turn that lands and is dragged away
 *   and a Turn that never landed leave the same final position, so every
 *   placement is recorded with what became of it and the drift is what
 *   separates them.
 * - **Fights.** Travel far exceeding net displacement is two writers undoing
 *   each other — the shape of a snap back reissued 958 times without arriving.
 * - **Refusals.** Who was outranked, by whom. A write that never happened is
 *   invisible in the DOM and in every other log.
 * - **Silent declines.** Each writer's reason for not moving. "Nothing
 *   happened" has been the report more often than "it moved wrongly".
 *
 * Entries carry a `repeated` summary when the frontend coalesced a run of
 * identical events, so every count here weighs an entry by what it stands for
 * rather than by one. Ignoring that would under-report exactly the runaway
 * loops the log is for.
 */

import { createReadStream } from 'node:fs';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

/** Joins the parts of a composite map key; cannot occur inside one. */
const KEY_SEPARATOR = String.fromCharCode(31);

const VIEWPORT_TAG = 'viewport';
const WRITE_LOCATION = 'viewportOwner.write';
const OUTCOME_SUFFIX = '.outcome';
const DROPPED_ENTRY_LOCATION = 'FlowChatDiagnosticsRecorder.flush';

/** Locations whose whole purpose is to record a movement that did not happen. */
const DECLINE_LOCATIONS = new Set([
  'anchor.dropped',
  'anchor.stoodDown',
  'followOutput.deferNewTurn',
  'historyPaging.refused',
  'snapBack.declined',
  'snapBack.notNeeded',
  'turnNavigation.rejected',
]);

export const DEFAULT_OPTIONS = {
  top: 20,
  minDrift: 8,
  gapMs: 750,
  around: null,
  radius: 12,
  tags: [],
};

function printUsage() {
  console.log(`Usage:
  node scripts/diagnostics/analyze-flowchat-log.mjs <flowchat.log> [options]

Options:
  --top <count>        Maximum rows per table (default: ${DEFAULT_OPTIONS.top})
  --min-drift <px>     Report a placement as unstuck past this drift (default: ${DEFAULT_OPTIONS.minDrift})
  --gap <ms>           Quiet period that ends an episode (default: ${DEFAULT_OPTIONS.gapMs})
  --around <sequence>  Print the raw entries around a sequence number
  --radius <count>     Sequence radius for --around (default: ${DEFAULT_OPTIONS.radius})
  --tag <name>         Only entries with this hypothesis tag; repeatable
  --help               Show this help`);
}

function parseNumberOption(args, index, optionName) {
  const rawValue = args[index + 1];
  const value = Number(rawValue);
  if (!rawValue || !Number.isFinite(value)) {
    throw new Error(`${optionName} requires a finite number`);
  }
  return value;
}

export function parseArgs(rawArgv) {
  // `npm run … -- <path>` forwards the separator itself; pnpm forwards it too
  // when it is typed. Either way it is not an argument.
  const argv = rawArgv[0] === '--' ? rawArgv.slice(1) : rawArgv;

  if (argv.includes('--help')) {
    printUsage();
    process.exit(0);
  }

  const logPath = argv[0];
  if (!logPath || logPath.startsWith('--')) {
    printUsage();
    throw new Error('A FlowChat JSONL log path is required');
  }

  const options = { ...DEFAULT_OPTIONS, tags: [], logPath };

  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--top') {
      options.top = Math.max(1, Math.floor(parseNumberOption(argv, index, arg)));
      index += 1;
    } else if (arg === '--min-drift') {
      options.minDrift = Math.max(0, parseNumberOption(argv, index, arg));
      index += 1;
    } else if (arg === '--gap') {
      options.gapMs = Math.max(0, parseNumberOption(argv, index, arg));
      index += 1;
    } else if (arg === '--around') {
      options.around = Math.floor(parseNumberOption(argv, index, arg));
      index += 1;
    } else if (arg === '--radius') {
      options.radius = Math.max(0, Math.floor(parseNumberOption(argv, index, arg)));
      index += 1;
    } else if (arg === '--tag') {
      const tag = argv[index + 1];
      if (!tag || tag.startsWith('--')) throw new Error('--tag requires a name');
      options.tags.push(tag);
      index += 1;
    } else {
      throw new Error(`Unknown option: ${arg}`);
    }
  }

  return options;
}

function finiteNumber(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : 0;
}

function round(value) {
  return Math.round(finiteNumber(value) * 10) / 10;
}

/**
 * What one entry stands for.
 *
 * A coalesced entry is one line describing a run, so counting it as one event
 * would report a 300-write fight as a handful of writes. Travel is the same
 * question for distance.
 */
export function entryWeight(entry) {
  return 1 + Math.max(0, Math.floor(finiteNumber(entry?.data?.repeated?.suppressedCount)));
}

export function entryTravelPx(entry) {
  const data = entry?.data ?? {};
  const own = data.toPx === undefined || data.fromPx === undefined
    ? 0
    : Math.abs(finiteNumber(data.toPx) - finiteNumber(data.fromPx));
  return own + Math.abs(finiteNumber(data.repeated?.suppressedTravelPx));
}

function compactData(data) {
  if (!data) return '';
  const serialized = JSON.stringify(data);
  return serialized.length <= 240 ? serialized : `${serialized.slice(0, 237)}...`;
}

function increment(map, key, by = 1) {
  map.set(key, (map.get(key) ?? 0) + by);
}

/**
 * Group viewport writes into stretches of activity.
 *
 * A fault is a period, not an entry: "it flickered for a second when I opened
 * the session" is one episode with several owners in it. The gap that ends one
 * is wall-clock, because the interesting silence is the reader looking at a
 * still transcript.
 */
export function collectEpisodes(writes, gapMs) {
  const episodes = [];
  let current = null;

  for (const write of writes) {
    const atMs = finiteNumber(write.performanceTimeMs);
    if (current === null || atMs - current.endMs > gapMs) {
      current = {
        firstSequence: write.sequence,
        lastSequence: write.sequence,
        startMs: atMs,
        endMs: atMs,
        writes: 0,
        refusals: 0,
        travelPx: 0,
        fromPx: null,
        toPx: null,
        owners: new Map(),
      };
      episodes.push(current);
    }

    const weight = entryWeight(write);
    const granted = write.data?.granted !== false;
    current.lastSequence = write.sequence;
    current.endMs = atMs;
    current.travelPx += entryTravelPx(write);
    if (granted) {
      current.writes += weight;
      if (current.fromPx === null) current.fromPx = finiteNumber(write.data?.fromPx);
      current.toPx = finiteNumber(write.data?.toPx);
    } else {
      current.refusals += weight;
    }
    increment(current.owners, String(write.data?.owner ?? 'unknown'), weight);
  }

  return episodes.map(episode => {
    const netPx = episode.fromPx === null ? 0 : episode.toPx - episode.fromPx;
    return {
      sequences: `${episode.firstSequence}-${episode.lastSequence}`,
      durationMs: Math.round(episode.endMs - episode.startMs),
      writes: episode.writes,
      refusals: episode.refusals,
      travelPx: round(episode.travelPx),
      netPx: round(netPx),
      /*
       * Distance travelled per pixel of progress. One means a clean move;
       * anything large is writers undoing each other, and it is the number to
       * sort by when the report is "it shook".
       */
      churn: Math.abs(netPx) < 1
        ? round(episode.travelPx)
        : round(episode.travelPx / Math.abs(netPx)),
      owners: [...episode.owners.entries()]
        .sort((left, right) => right[1] - left[1])
        .map(([owner, count]) => `${owner}x${count}`)
        .join(' '),
    };
  });
}

/**
 * Pair every placement with the sample taken after it settled.
 *
 * Matched first-in-first-out per location: an outcome lands hundreds of
 * milliseconds after its placement, so another placement of the same kind can
 * begin in between, and the queue keeps them in order. An outcome with nothing
 * pending is reported rather than dropped — it means the log starts mid-flight.
 */
export function joinPlacements(entries) {
  const pending = new Map();
  const placements = [];
  const orphanedOutcomes = [];

  for (const entry of entries) {
    const location = String(entry.location ?? '');
    if (location.endsWith(OUTCOME_SUFFIX)) {
      const placedLocation = location.slice(0, -OUTCOME_SUFFIX.length);
      const queue = pending.get(placedLocation);
      const placement = queue?.shift();
      if (!placement) {
        orphanedOutcomes.push({ sequence: entry.sequence, location: placedLocation });
        continue;
      }
      placement.outcome = entry;
      continue;
    }
    if (entry.data?.placedPx === undefined) continue;

    const placement = { placed: entry, outcome: null };
    placements.push(placement);
    const queue = pending.get(location);
    if (queue) queue.push(placement);
    else pending.set(location, [placement]);
  }

  return { placements, orphanedOutcomes };
}

export function summarizePlacements(placements) {
  return placements.map(({ placed, outcome }) => {
    const data = placed.data ?? {};
    const outcomeData = outcome?.data ?? {};
    return {
      sequence: placed.sequence,
      location: String(placed.location ?? ''),
      branch: String(data.branch ?? data.reason ?? ''),
      beforePx: round(data.beforePx),
      placedPx: round(data.placedPx),
      targetPx: data.targetPx === undefined ? round(data.placedPx) : round(data.targetPx),
      settledPx: outcome ? round(outcomeData.settledPx) : null,
      driftPx: outcome ? round(outcomeData.driftPx) : null,
      /*
       * No outcome is not "no drift". The sample is scheduled on a timer, so a
       * placement at the end of the log, or one whose view unmounted first,
       * never reports — and reading that as a clean landing is how a report
       * turns into a false negative.
       */
      settled: outcome ? 'yes' : 'unknown',
    };
  });
}

export function analyzeEntries(entries, options) {
  const settings = { ...DEFAULT_OPTIONS, ...options };
  const tags = new Set(settings.tags ?? []);
  const kept = tags.size === 0
    ? entries
    : entries.filter(entry => tags.has(String(entry.hypothesis ?? '')));

  const viewportEntries = kept.filter(entry => entry.hypothesis === VIEWPORT_TAG);
  const writes = viewportEntries.filter(entry => entry.location === WRITE_LOCATION);

  const refusals = new Map();
  const ownerActivity = new Map();
  for (const write of writes) {
    const weight = entryWeight(write);
    const owner = String(write.data?.owner ?? 'unknown');
    const activity = ownerActivity.get(owner) ?? { owner, writes: 0, refusals: 0, travelPx: 0 };
    activity.travelPx += entryTravelPx(write);
    if (write.data?.granted === false) {
      activity.refusals += weight;
      increment(refusals, `${owner}${KEY_SEPARATOR}${String(write.data?.heldBy ?? 'nobody')}`, weight);
    } else {
      activity.writes += weight;
    }
    ownerActivity.set(owner, activity);
  }

  const declines = new Map();
  for (const entry of viewportEntries) {
    const location = String(entry.location ?? '');
    if (!DECLINE_LOCATIONS.has(location)) continue;
    const reason = String(entry.data?.reason ?? entry.data?.branch ?? '');
    increment(declines, `${location}${KEY_SEPARATOR}${reason}`, entryWeight(entry));
  }

  const frequency = new Map();
  for (const entry of kept) {
    const key = `${String(entry.hypothesis ?? '')}${KEY_SEPARATOR}${String(entry.location ?? '')}`;
    increment(frequency, key, entryWeight(entry));
  }

  const { placements, orphanedOutcomes } = joinPlacements(viewportEntries);
  const summarized = summarizePlacements(placements);

  const droppedEntries = kept
    .filter(entry => entry.location === DROPPED_ENTRY_LOCATION)
    .reduce((total, entry) => total + finiteNumber(entry.data?.droppedEntries), 0);

  return {
    entryCount: kept.length,
    viewportEntryCount: viewportEntries.length,
    droppedEntries,
    sequenceRange: kept.length === 0
      ? null
      : { first: kept[0].sequence, last: kept[kept.length - 1].sequence },
    episodes: collectEpisodes(writes, settings.gapMs)
      .sort((left, right) => right.churn - left.churn),
    unstuckPlacements: summarized
      .filter(placement => placement.driftPx !== null
        && Math.abs(placement.driftPx) >= settings.minDrift)
      .sort((left, right) => Math.abs(right.driftPx) - Math.abs(left.driftPx)),
    unsampledPlacements: summarized.filter(placement => placement.settled === 'unknown'),
    placementCount: summarized.length,
    orphanedOutcomes,
    refusals: [...refusals.entries()]
      .map(([key, count]) => {
        const [owner, heldBy] = key.split(KEY_SEPARATOR);
        return { owner, refusedBy: heldBy, count };
      })
      .sort((left, right) => right.count - left.count),
    ownerActivity: [...ownerActivity.values()]
      .map(activity => ({ ...activity, travelPx: round(activity.travelPx) }))
      .sort((left, right) => right.writes + right.refusals - (left.writes + left.refusals)),
    declines: [...declines.entries()]
      .map(([key, count]) => {
        const [location, reason] = key.split(KEY_SEPARATOR);
        return { location, reason, count };
      })
      .sort((left, right) => right.count - left.count),
    frequency: [...frequency.entries()]
      .map(([key, count]) => {
        const [tag, location] = key.split(KEY_SEPARATOR);
        return { tag, location, count };
      })
      .sort((left, right) => right.count - left.count),
    window: settings.around === null ? [] : kept
      .filter(entry => finiteNumber(entry.sequence) >= settings.around - settings.radius
        && finiteNumber(entry.sequence) <= settings.around + settings.radius)
      .map(entry => ({
        sequence: entry.sequence,
        tag: String(entry.hypothesis ?? ''),
        location: String(entry.location ?? ''),
        message: String(entry.message ?? ''),
        data: compactData(entry.data),
      })),
  };
}

export async function readEntries(logPath) {
  const entries = [];
  let lineCount = 0;
  let parseErrorCount = 0;

  const input = createReadStream(logPath, { encoding: 'utf8' });
  const lines = createInterface({ input, crlfDelay: Infinity });
  for await (const line of lines) {
    lineCount += 1;
    if (!line.trim()) continue;
    try {
      entries.push(JSON.parse(line));
    } catch {
      parseErrorCount += 1;
    }
  }

  return { entries, lineCount, parseErrorCount };
}

function reportTable(title, rows, top) {
  console.log(`\n${title}`);
  if (rows.length === 0) {
    console.log('  (none)');
    return;
  }
  console.table(rows.slice(0, top));
  if (rows.length > top) {
    console.log(`  ... ${rows.length - top} more`);
  }
}

export function printReport(report, options, source) {
  console.log(`FlowChat log: ${source.logPath}`);
  console.log(
    `Lines: ${source.lineCount}, entries: ${report.entryCount}`
    + ` (viewport: ${report.viewportEntryCount}), parse errors: ${source.parseErrorCount}`,
  );
  if (report.sequenceRange) {
    console.log(`Sequences: ${report.sequenceRange.first}-${report.sequenceRange.last}`);
  }
  if (report.droppedEntries > 0) {
    // Said loudly: every count below is a lower bound once entries were lost.
    console.log(
      `WARNING: ${report.droppedEntries} entries were dropped before reaching the log.`
      + ' Counts below are lower bounds.',
    );
  }

  reportTable(
    'Episodes of viewport activity, worst churn first (travel per pixel of progress)',
    report.episodes,
    options.top,
  );
  reportTable(
    `Placements that did not stick (drift >= ${options.minDrift}px)`,
    report.unstuckPlacements,
    options.top,
  );
  reportTable('Refusals: who was outranked, by whom', report.refusals, options.top);
  reportTable('Declines: a writer choosing not to move, and why', report.declines, options.top);
  reportTable('Per owner', report.ownerActivity, options.top);
  reportTable('Most frequent locations', report.frequency, options.top);

  console.log(
    `\nPlacements: ${report.placementCount},`
    + ` never sampled: ${report.unsampledPlacements.length},`
    + ` outcomes with no placement in this log: ${report.orphanedOutcomes.length}`,
  );

  if (options.around !== null) {
    reportTable(
      `Entries around sequence ${options.around} (+/- ${options.radius})`,
      report.window,
      report.window.length,
    );
  }
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const source = await readEntries(options.logPath);
  const report = analyzeEntries(source.entries, options);
  printReport(report, options, { ...source, logPath: options.logPath });
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
