import { createReadStream } from 'node:fs';
import { createInterface } from 'node:readline';

function printUsage() {
  console.log(`Usage:
  node scripts/diagnostics/analyze-flowchat-log.mjs <flowchat.log> [options]

Options:
  --top <count>       Maximum rows per summary table (default: 20)
  --min-delta <px>    Minimum positive reservation jump to show (default: 100)
  --around <sequence> Show a compact event window around a sequence
  --radius <count>    Sequence radius for --around (default: 8)
  --help              Show this help`);
}

function parseNumberOption(args, index, optionName) {
  const rawValue = args[index + 1];
  const value = Number(rawValue);
  if (!rawValue || !Number.isFinite(value)) {
    throw new Error(`${optionName} requires a finite number`);
  }
  return value;
}

function parseArgs(argv) {
  if (argv.includes('--help')) {
    printUsage();
    process.exit(0);
  }

  const logPath = argv[0];
  if (!logPath || logPath.startsWith('--')) {
    printUsage();
    throw new Error('A FlowChat JSONL log path is required');
  }

  const options = {
    logPath,
    top: 20,
    minDelta: 100,
    around: null,
    radius: 8,
  };

  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--top') {
      options.top = Math.max(1, Math.floor(parseNumberOption(argv, index, arg)));
      index += 1;
    } else if (arg === '--min-delta') {
      options.minDelta = Math.max(0, parseNumberOption(argv, index, arg));
      index += 1;
    } else if (arg === '--around') {
      options.around = Math.floor(parseNumberOption(argv, index, arg));
      index += 1;
    } else if (arg === '--radius') {
      options.radius = Math.max(0, Math.floor(parseNumberOption(argv, index, arg)));
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
  return Math.round(finiteNumber(value) * 100) / 100;
}

function reservationTotal(reservation) {
  return finiteNumber(reservation?.collapse?.px) + finiteNumber(reservation?.pin?.px);
}

function compactData(data) {
  if (!data) return '';
  const serialized = JSON.stringify(data);
  return serialized.length <= 240 ? serialized : `${serialized.slice(0, 237)}...`;
}

async function analyze(options) {
  const eventCounts = new Map();
  const reservationJumps = [];
  const collapseIntents = [];
  const sequenceWindow = [];
  let lineCount = 0;
  let eventCount = 0;
  let parseErrorCount = 0;

  const input = createReadStream(options.logPath, { encoding: 'utf8' });
  const lines = createInterface({ input, crlfDelay: Infinity });

  for await (const line of lines) {
    lineCount += 1;
    if (!line.trim()) continue;

    let event;
    try {
      event = JSON.parse(line);
    } catch {
      parseErrorCount += 1;
      continue;
    }
    eventCount += 1;

    const countKey = `${event.location ?? ''}\u0000${event.message ?? ''}`;
    const existingCount = eventCounts.get(countKey);
    if (existingCount) {
      existingCount.count += 1;
    } else {
      eventCounts.set(countKey, {
        count: 1,
        location: event.location ?? '',
        message: event.message ?? '',
      });
    }

    if (
      event.location === 'VirtualMessageList.updateBottomReservationState' &&
      event.data?.before &&
      event.data?.after
    ) {
      const before = reservationTotal(event.data.before);
      const after = reservationTotal(event.data.after);
      const delta = after - before;
      if (delta >= options.minDelta) {
        reservationJumps.push({
          sequence: event.sequence,
          deltaPx: round(delta),
          beforePx: round(before),
          afterPx: round(after),
          collapsePx: round(event.data.after.collapse?.px),
          pinPx: round(event.data.after.pin?.px),
          coordinatorMode: event.data.coordinatorMode ?? '',
          following: event.data.isFollowingOutput === true,
          streaming: event.data.isStreamingOutput === true,
        });
      }
    }

    if (
      event.location === 'VirtualMessageList.handleToolCardCollapseIntent' &&
      event.message === 'Tool card collapse reservation calculated'
    ) {
      const current = finiteNumber(event.data?.currentTotalCompensationPx);
      const provisional = finiteNumber(event.data?.provisionalTotalCompensationPx);
      collapseIntents.push({
        sequence: event.sequence,
        tool: event.data?.nextIntent?.toolName ?? '',
        cardHeightPx: round(event.data?.estimatedShrink),
        distancePx: round(event.data?.effectiveDistanceFromBottom),
        addedPx: round(provisional - current),
        totalPx: round(provisional),
        coordinatorMode: event.data?.coordinatorMode ?? '',
      });
    }

    if (
      options.around !== null &&
      finiteNumber(event.sequence) >= options.around - options.radius &&
      finiteNumber(event.sequence) <= options.around + options.radius
    ) {
      sequenceWindow.push({
        sequence: event.sequence,
        location: event.location ?? '',
        message: event.message ?? '',
        data: compactData(event.data),
      });
    }
  }

  console.log(`FlowChat log: ${options.logPath}`);
  console.log(`Lines: ${lineCount}, events: ${eventCount}, parse errors: ${parseErrorCount}`);

  console.log('\nMost frequent events');
  console.table(
    [...eventCounts.values()]
      .sort((left, right) => right.count - left.count)
      .slice(0, options.top),
  );

  console.log(`\nLargest reservation increases (>= ${options.minDelta}px)`);
  console.table(
    reservationJumps
      .sort((left, right) => right.deltaPx - left.deltaPx)
      .slice(0, options.top),
  );

  console.log('\nLargest collapse-intent estimates');
  console.table(
    collapseIntents
      .sort((left, right) => right.addedPx - left.addedPx)
      .slice(0, options.top),
  );

  if (options.around !== null) {
    console.log(`\nEvents around sequence ${options.around} (+/- ${options.radius})`);
    console.table(sequenceWindow);
  }
}

try {
  const options = parseArgs(process.argv.slice(2));
  await analyze(options);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
