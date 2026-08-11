import assert from 'node:assert/strict';
import test from 'node:test';

import {
  analyzeEntries,
  collectEpisodes,
  entryTravelPx,
  entryWeight,
  joinPlacements,
  parseArgs,
} from './analyze-flowchat-log.mjs';

let nextSequence = 0;

function entry(location, data, overrides = {}) {
  nextSequence += 1;
  return {
    sequence: nextSequence,
    timestamp: '2026-08-11T00:00:00.000Z',
    performanceTimeMs: nextSequence * 16,
    hypothesis: 'viewport',
    location,
    message: location,
    data,
    ...overrides,
  };
}

function write(owner, fromPx, toPx, overrides = {}) {
  return entry('viewportOwner.write', {
    owner,
    granted: true,
    heldBy: null,
    fromPx,
    toPx,
    ...overrides.data,
  }, overrides.entry);
}

test('parseArgs requires a log path and rejects unknown options', () => {
  assert.throws(() => parseArgs([]), /log path is required/);
  assert.throws(() => parseArgs(['flowchat.log', '--nope']), /Unknown option/);

  const options = parseArgs(['flowchat.log', '--min-drift', '20', '--tag', 'viewport']);
  assert.equal(options.logPath, 'flowchat.log');
  assert.equal(options.minDrift, 20);
  assert.deepEqual(options.tags, ['viewport']);

  // `npm run … -- <path>` hands the separator through as an argument.
  assert.equal(parseArgs(['--', 'flowchat.log']).logPath, 'flowchat.log');
});

test('a coalesced entry counts as the run it stands for', () => {
  const coalesced = write('follow-output', 100, 102, {
    data: { repeated: { suppressedCount: 59, suppressedTravelPx: 118, suppressedForMs: 480 } },
  });

  assert.equal(entryWeight(coalesced), 60);
  assert.equal(entryTravelPx(coalesced), 120);
  // An entry with no run behind it still stands for itself.
  assert.equal(entryWeight(write('follow-output', 0, 10)), 1);
});

test('episodes split on a quiet gap and score a fight by its churn', () => {
  nextSequence = 0;
  const fight = [
    write('snap-back', 1000, 1000.7),
    write('anchor-correction', 1000.7, 1000),
    write('snap-back', 1000, 1000.7),
    write('anchor-correction', 1000.7, 1000),
  ];
  const later = write('follow-output', 1000, 2000, {
    entry: { performanceTimeMs: 20_000 },
  });

  const episodes = collectEpisodes([...fight, later], 750);

  assert.equal(episodes.length, 2);
  // Four writes that went nowhere: travel without progress is the whole signal.
  assert.equal(episodes[0].writes, 4);
  assert.equal(episodes[0].netPx, 0);
  assert.ok(episodes[0].churn >= 2, `expected churn, got ${episodes[0].churn}`);
  assert.equal(episodes[1].netPx, 1000);
  assert.equal(episodes[1].churn, 1);
});

test('refused writes are counted apart from the ones that moved', () => {
  nextSequence = 0;
  const episodes = collectEpisodes([
    write('follow-output', 500, 600),
    write('anchor-correction', 600, 500, { data: { granted: false, heldBy: 'user-gesture' } }),
  ], 750);

  assert.equal(episodes[0].writes, 1);
  assert.equal(episodes[0].refusals, 1);
  // A refusal moved nothing, so it cannot contribute to net displacement.
  assert.equal(episodes[0].netPx, 100);
});

test('placements pair with their outcome in order, and a stray outcome is reported', () => {
  nextSequence = 0;
  const first = entry('turnNavigation.placed', { beforePx: 0, placedPx: 800, targetPx: 800 });
  const second = entry('turnNavigation.placed', { beforePx: 800, placedPx: 1600, targetPx: 1600 });
  const firstOutcome = entry('turnNavigation.placed.outcome', { settledPx: 40, driftPx: -760 });
  const secondOutcome = entry('turnNavigation.placed.outcome', { settledPx: 1600, driftPx: 0 });
  const stray = entry('visibleTask.scrollToTask.outcome', { settledPx: 10, driftPx: -5 });

  const { placements, orphanedOutcomes } = joinPlacements([
    first,
    second,
    firstOutcome,
    secondOutcome,
    stray,
  ]);

  assert.equal(placements.length, 2);
  assert.equal(placements[0].outcome, firstOutcome);
  assert.equal(placements[1].outcome, secondOutcome);
  assert.deepEqual(orphanedOutcomes, [
    { sequence: stray.sequence, location: 'visibleTask.scrollToTask' },
  ]);
});

test('analyzeEntries reports what did not stick, who was refused, and who declined', () => {
  nextSequence = 0;
  const report = analyzeEntries([
    entry('visibleTask.scrollToTask', { beforePx: 0, placedPx: 900, targetPx: 900 }),
    write('anchor-correction', 900, 120),
    entry('visibleTask.scrollToTask.outcome', { settledPx: 120, driftPx: -780 }),
    write('follow-output', 120, 200, { data: { granted: false, heldBy: 'user-gesture' } }),
    entry('snapBack.declined', { reason: 'follow-correcting' }),
    entry('snapBack.declined', { reason: 'follow-correcting' }),
    entry('followOutput.deferNewTurn', { turnId: 't-42' }),
    { ...entry('history_paging_requested', { direction: 'before' }), hypothesis: 'history-paging' },
  ], { minDrift: 8 });

  assert.equal(report.unstuckPlacements.length, 1);
  assert.equal(report.unstuckPlacements[0].driftPx, -780);
  assert.equal(report.unstuckPlacements[0].location, 'visibleTask.scrollToTask');

  assert.deepEqual(report.refusals, [
    { owner: 'follow-output', refusedBy: 'user-gesture', count: 1 },
  ]);

  const declined = report.declines.find(row => row.location === 'snapBack.declined');
  assert.deepEqual(declined, {
    location: 'snapBack.declined',
    reason: 'follow-correcting',
    count: 2,
  });
  assert.ok(report.declines.some(row => row.location === 'followOutput.deferNewTurn'));

  // The paging tag is counted but never mistaken for viewport activity.
  assert.equal(report.viewportEntryCount, 7);
  assert.ok(report.frequency.some(row => row.tag === 'history-paging'));
});

test('a placement whose outcome never arrived is not reported as having stuck', () => {
  nextSequence = 0;
  const report = analyzeEntries([
    entry('navigation.scrollIntoView', { beforePx: 0, placedPx: 300 }),
  ], {});

  assert.equal(report.placementCount, 1);
  assert.equal(report.unstuckPlacements.length, 0);
  assert.equal(report.unsampledPlacements.length, 1);
  assert.equal(report.unsampledPlacements[0].settled, 'unknown');
});

test('dropped entries are surfaced, because every count becomes a lower bound', () => {
  nextSequence = 0;
  const report = analyzeEntries([
    { ...entry('FlowChatDiagnosticsRecorder.flush', { droppedEntries: 128 }), hypothesis: 'I' },
    write('follow-output', 0, 100),
  ], {});

  assert.equal(report.droppedEntries, 128);
});

test('--tag keeps only the requested stream', () => {
  nextSequence = 0;
  const report = analyzeEntries([
    write('follow-output', 0, 100),
    { ...entry('history_paging_requested', { direction: 'before' }), hypothesis: 'history-paging' },
  ], { tags: ['history-paging'] });

  assert.equal(report.entryCount, 1);
  assert.equal(report.viewportEntryCount, 0);
  assert.equal(report.episodes.length, 0);
});
