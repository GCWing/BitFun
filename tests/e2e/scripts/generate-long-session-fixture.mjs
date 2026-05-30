import crypto from 'crypto';
import fs from 'fs/promises';
import fsSync from 'fs';
import os from 'os';
import path from 'path';

const SESSION_STORAGE_SCHEMA_VERSION = 2;
const MAX_PROJECT_SLUG_LEN = 120;

function parseArgs(argv) {
  const options = {
    workspace: undefined,
    bitfunHome: process.env.BITFUN_HOME || path.join(os.homedir(), '.bitfun'),
    sessionPrefix: 'perf-long-session',
    sessionCount: 80,
    longTurns: 80,
    shortTurns: 1,
    assistantChars: 2_000,
    toolResultChars: 12_000,
    toolItems: 2,
    cleanup: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = () => {
      index += 1;
      if (index >= argv.length) {
        throw new Error(`Missing value for ${arg}`);
      }
      return argv[index];
    };

    switch (arg) {
      case '--workspace':
        options.workspace = next();
        break;
      case '--bitfun-home':
        options.bitfunHome = next();
        break;
      case '--session-prefix':
        options.sessionPrefix = next();
        break;
      case '--session-count':
        options.sessionCount = Number(next());
        break;
      case '--long-turns':
        options.longTurns = Number(next());
        break;
      case '--short-turns':
        options.shortTurns = Number(next());
        break;
      case '--assistant-chars':
        options.assistantChars = Number(next());
        break;
      case '--tool-result-chars':
        options.toolResultChars = Number(next());
        break;
      case '--tool-items':
        options.toolItems = Number(next());
        break;
      case '--cleanup':
        options.cleanup = true;
        break;
      case '--help':
        printHelp();
        process.exit(0);
        break;
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }

  if (!options.workspace) {
    throw new Error('Missing --workspace');
  }
  for (const key of ['sessionCount', 'longTurns', 'shortTurns', 'assistantChars', 'toolResultChars', 'toolItems']) {
    if (!Number.isFinite(options[key]) || options[key] < 0) {
      throw new Error(`Invalid numeric value for ${key}`);
    }
  }
  if (options.sessionCount < 1) {
    throw new Error('--session-count must be at least 1');
  }
  if (!options.sessionPrefix.trim()) {
    throw new Error('--session-prefix cannot be empty');
  }
  return options;
}

function printHelp() {
  console.log(`Generate BitFun long-session performance fixtures.

Usage:
  node tests/e2e/scripts/generate-long-session-fixture.mjs --workspace <path> [options]

Options:
  --session-count <n>       Number of metadata rows to create. Default: 80
  --long-turns <n>          Turn count for the latest session. Default: 80
  --short-turns <n>         Turn count for other sessions. Default: 1
  --assistant-chars <n>     Assistant text chars per turn. Default: 2000
  --tool-result-chars <n>   Raw tool result chars per tool item. Default: 12000
  --tool-items <n>          Tool item count per turn. Default: 2
  --session-prefix <text>   Session id prefix. Default: perf-long-session
  --bitfun-home <path>      BitFun home root. Default: BITFUN_HOME or ~/.bitfun
  --cleanup                 Remove generated sessions for the prefix.
`);
}

function projectRuntimeSlug(workspacePath) {
  const canonical = fsSync.realpathSync(workspacePath);
  const slug = canonical
    .split('')
    .map(ch => /[a-zA-Z0-9]/.test(ch) ? ch.toLowerCase() : '-')
    .join('')
    .replace(/^-+|-+$/g, '') || 'workspace';

  if (slug.length <= MAX_PROJECT_SLUG_LEN) {
    return slug;
  }

  const suffix = crypto.createHash('sha256').update(canonical).digest('hex').slice(0, 12);
  const maxPrefixLen = MAX_PROJECT_SLUG_LEN - suffix.length - 1;
  return `${slug.slice(0, maxPrefixLen).replace(/-+$/g, '')}-${suffix}`;
}

function repeatedText(chars, label) {
  if (chars <= label.length + 1) {
    return label.slice(0, chars);
  }
  return `${label} ${'x'.repeat(chars - label.length - 1)}`;
}

function makeMetadata({ sessionId, sessionName, workspacePath, createdAt, lastActiveAt, turnCount, toolItems }) {
  return {
    sessionId,
    sessionName,
    agentType: 'agentic',
    lastUserDialogAgentType: 'agentic',
    lastSubmittedAgentType: 'agentic',
    createdBy: null,
    sessionKind: 'standard',
    modelName: 'perf-fixture-model',
    createdAt,
    lastActiveAt,
    turnCount,
    messageCount: turnCount * 2,
    toolCallCount: turnCount * toolItems,
    status: 'active',
    terminalSessionId: null,
    snapshotSessionId: null,
    tags: ['performance-fixture'],
    customMetadata: {
      generatedBy: 'tests/e2e/scripts/generate-long-session-fixture.mjs',
      fixtureVersion: 1,
    },
    relationship: null,
    todos: null,
    deepReviewRunManifest: null,
    deepReviewCache: null,
    workspacePath,
    workspaceHostname: 'localhost',
    unreadCompletion: null,
    needsUserAttention: null,
  };
}

function makeState(workspacePath) {
  return {
    schema_version: SESSION_STORAGE_SCHEMA_VERSION,
    config: {
      max_context_tokens: 128128,
      auto_compact: true,
      enable_tools: true,
      safe_mode: true,
      max_turns: 200,
      enable_context_compression: true,
      compression_threshold: 0.8,
      workspace_path: workspacePath,
      workspace_id: null,
      remote_connection_id: null,
      remote_ssh_host: null,
      model_id: 'perf-fixture-model',
    },
    snapshot_session_id: null,
    last_user_dialog_agent_type: 'agentic',
    last_submitted_agent_type: 'agentic',
    compression_state: {
      last_compression_at: null,
      compression_count: 0,
    },
    runtime_state: 'Idle',
  };
}

function makeTurn({ sessionId, turnIndex, timestamp, assistantChars, toolResultChars, toolItems }) {
  const turnId = `${sessionId}-turn-${String(turnIndex).padStart(4, '0')}`;
  const textItemId = `${turnId}-text-0`;
  const toolItemsData = Array.from({ length: toolItems }, (_, toolIndex) => {
    const toolId = `${turnId}-tool-${toolIndex}`;
    return {
      id: toolId,
      toolName: 'Read',
      toolCall: {
        id: toolId,
        input: {
          filePath: `/workspace/perf-fixture-${turnIndex}-${toolIndex}.txt`,
        },
      },
      toolResult: {
        result: {
          output: repeatedText(toolResultChars, `raw result ${turnIndex}.${toolIndex}`),
          fixture: true,
        },
        success: true,
        resultForAssistant: repeatedText(
          Math.min(512, toolResultChars),
          `assistant result ${turnIndex}.${toolIndex}`,
        ),
        durationMs: 5,
      },
      aiIntent: 'Synthetic performance fixture tool result',
      startTime: timestamp + 10,
      endTime: timestamp + 15,
      durationMs: 5,
      queueWaitMs: 0,
      preflightMs: 0,
      confirmationWaitMs: 0,
      executionMs: 5,
      orderIndex: toolIndex,
      status: 'completed',
    };
  });

  return {
    schema_version: SESSION_STORAGE_SCHEMA_VERSION,
    turnId,
    turnIndex,
    sessionId,
    timestamp,
    kind: 'user_dialog',
    agentType: 'agentic',
    userMessage: {
      id: `${turnId}-user`,
      content: `Synthetic user turn ${turnIndex}`,
      timestamp,
      metadata: {
        generatedBy: 'performance-fixture',
      },
    },
    modelRounds: [
      {
        id: `${turnId}-round-0`,
        turnId,
        roundIndex: 0,
        timestamp: timestamp + 1,
        textItems: [
          {
            id: textItemId,
            content: repeatedText(assistantChars, `Synthetic assistant response ${turnIndex}`),
            isStreaming: false,
            timestamp: timestamp + 20,
            isMarkdown: true,
            orderIndex: toolItems,
            status: 'completed',
          },
        ],
        toolItems: toolItemsData,
        thinkingItems: [],
        startTime: timestamp + 1,
        endTime: timestamp + 30,
        durationMs: 29,
        providerId: 'perf-fixture',
        modelId: 'perf-fixture-model',
        modelAlias: 'Perf Fixture',
        status: 'completed',
      },
    ],
    startTime: timestamp,
    endTime: timestamp + 30,
    durationMs: 30,
    status: 'completed',
  };
}

async function readIndex(indexPath) {
  try {
    return JSON.parse(await fs.readFile(indexPath, 'utf8'));
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return null;
    }
    throw error;
  }
}

async function writeJson(filePath, value) {
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

async function removeGeneratedSessions(sessionsRoot, sessionPrefix) {
  try {
    const entries = await fs.readdir(sessionsRoot, { withFileTypes: true });
    await Promise.all(entries
      .filter(entry => entry.isDirectory() && entry.name.startsWith(sessionPrefix))
      .map(entry => fs.rm(path.join(sessionsRoot, entry.name), { recursive: true, force: true })));
  } catch (error) {
    if (error && error.code !== 'ENOENT') {
      throw error;
    }
  }
}

async function writeIndex(sessionsRoot, generatedMetadata, sessionPrefix) {
  const indexPath = path.join(sessionsRoot, 'index.json');
  const existing = await readIndex(indexPath);
  const existingSessions = Array.isArray(existing?.sessions) ? existing.sessions : [];
  const retained = existingSessions.filter(session =>
    typeof session?.sessionId === 'string' && !session.sessionId.startsWith(sessionPrefix)
  );
  const sessions = [...generatedMetadata, ...retained]
    .sort((left, right) => (right.lastActiveAt ?? 0) - (left.lastActiveAt ?? 0));

  await writeJson(indexPath, {
    schema_version: SESSION_STORAGE_SCHEMA_VERSION,
    updated_at: Date.now(),
    metadata_file_count: sessions.length,
    sessions,
  });
}

async function generate(options) {
  const workspacePath = fsSync.realpathSync(options.workspace);
  const slug = projectRuntimeSlug(workspacePath);
  const sessionsRoot = path.join(options.bitfunHome, 'projects', slug, 'sessions');

  await fs.mkdir(sessionsRoot, { recursive: true });
  await removeGeneratedSessions(sessionsRoot, options.sessionPrefix);

  if (options.cleanup) {
    await writeIndex(sessionsRoot, [], options.sessionPrefix);
    return {
      action: 'cleanup',
      workspacePath,
      sessionsRoot,
      sessionPrefix: options.sessionPrefix,
    };
  }

  const now = Date.now();
  const generatedMetadata = [];
  for (let sessionIndex = 0; sessionIndex < options.sessionCount; sessionIndex += 1) {
    const sessionId = `${options.sessionPrefix}-${String(sessionIndex).padStart(3, '0')}`;
    const turnCount = sessionIndex === 0 ? options.longTurns : options.shortTurns;
    const createdAt = now - (options.sessionCount - sessionIndex) * 60_000;
    const lastActiveAt = now - sessionIndex * 1_000;
    const sessionDir = path.join(sessionsRoot, sessionId);
    const turnsDir = path.join(sessionDir, 'turns');
    const metadata = makeMetadata({
      sessionId,
      sessionName: sessionIndex === 0
        ? `Perf Fixture Long Session (${turnCount} turns)`
        : `Perf Fixture Session ${sessionIndex}`,
      workspacePath,
      createdAt,
      lastActiveAt,
      turnCount,
      toolItems: options.toolItems,
    });

    await fs.mkdir(turnsDir, { recursive: true });
    await writeJson(path.join(sessionDir, 'metadata.json'), {
      schema_version: SESSION_STORAGE_SCHEMA_VERSION,
      ...metadata,
    });
    await writeJson(path.join(sessionDir, 'state.json'), makeState(workspacePath));

    for (let turnIndex = 0; turnIndex < turnCount; turnIndex += 1) {
      await writeJson(
        path.join(turnsDir, `turn-${String(turnIndex).padStart(4, '0')}.json`),
        makeTurn({
          sessionId,
          turnIndex,
          timestamp: createdAt + turnIndex * 1_000,
          assistantChars: options.assistantChars,
          toolResultChars: options.toolResultChars,
          toolItems: options.toolItems,
        }),
      );
    }

    generatedMetadata.push(metadata);
  }

  await writeIndex(sessionsRoot, generatedMetadata, options.sessionPrefix);
  return {
    action: 'generate',
    workspacePath,
    bitfunHome: options.bitfunHome,
    sessionsRoot,
    sessionPrefix: options.sessionPrefix,
    sessionCount: options.sessionCount,
    longSessionId: `${options.sessionPrefix}-000`,
    longTurns: options.longTurns,
    assistantChars: options.assistantChars,
    toolResultChars: options.toolResultChars,
    toolItems: options.toolItems,
  };
}

try {
  const options = parseArgs(process.argv.slice(2));
  const result = await generate(options);
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  console.error('Run with --help for usage.');
  process.exit(1);
}
