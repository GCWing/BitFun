import { $, browser, expect } from '@wdio/globals';
import * as fs from 'fs/promises';
import * as path from 'path';
import {
  readPerformanceNow,
  readStartupTraceSnapshot,
  waitForTracePhaseCount,
} from '../../helpers/performance-trace';

type EditorScenario = 'code-editor' | 'git-diff';

interface EditorOpenReport {
  appMode: string;
  scenario: EditorScenario;
  traceId: string;
  filePath: string;
  waitedForEditorWarmup: boolean;
  editorWarmupStartAtMs?: number;
  editorWarmupEndAtMs?: number;
  editorWarmupDurationMs?: number;
  editorWarmupCompletedBeforeTrigger: boolean;
  triggerAtMs: number;
  editorReadyAtMs: number;
  triggerToEditorReadyMs: number;
  matchingEditorCount: number;
}

function reportDir(): string {
  return path.resolve(process.cwd(), 'reports', 'performance');
}

async function writeReport(name: string, data: unknown): Promise<void> {
  await fs.mkdir(reportDir(), { recursive: true });
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  await fs.writeFile(
    path.join(reportDir(), `${name}-${timestamp}.json`),
    `${JSON.stringify(data, null, 2)}\n`,
    'utf8',
  );
}

function getScenario(): EditorScenario {
  const raw = process.env.BITFUN_E2E_EDITOR_SCENARIO;
  return raw === 'git-diff' ? 'git-diff' : 'code-editor';
}

function getWorkspacePath(): string {
  return process.env.E2E_TEST_WORKSPACE || path.resolve(process.cwd(), '..', '..');
}

function getEditorFilePath(workspacePath: string): string {
  return process.env.BITFUN_E2E_EDITOR_FILE_PATH
    || path.join(workspacePath, 'src', 'web-ui', 'src', 'main.tsx');
}

function shouldWaitForEditorWarmup(): boolean {
  return process.env.BITFUN_E2E_WAIT_FOR_EDITOR_WARMUP === '1';
}

function fileNameOf(filePath: string): string {
  return filePath.replace(/\\/g, '/').split('/').pop() || 'main.tsx';
}

function lastPhaseAt(snapshot: Awaited<ReturnType<typeof readStartupTraceSnapshot>>, phase: string): number | undefined {
  return snapshot.phases.events
    .filter(event => event.phase === phase)
    .at(-1)?.atMs;
}

async function openSessionScene(): Promise<void> {
  await browser.execute(() => {
    window.dispatchEvent(new CustomEvent('scene:open', { detail: { sceneId: 'session' } }));
    window.dispatchEvent(new CustomEvent('expand-right-panel'));
  });
  await $('[data-testid="app-main-content"]').waitForExist({ timeout: 10000 });
}

async function openGitScene(): Promise<void> {
  await browser.execute(() => {
    window.dispatchEvent(new CustomEvent('scene:open', { detail: { sceneId: 'git' } }));
  });
  await $('.bitfun-git-scene-working-copy__diff-area').waitForExist({ timeout: 15000 });
}

async function triggerCodeEditor(filePath: string, fileName: string): Promise<number> {
  return browser.execute((args) => {
    const triggerAt = performance.now();
    const duplicateKey = `perf-code-editor:${args.filePath}:${triggerAt}`;
    window.dispatchEvent(new CustomEvent('scene:open', { detail: { sceneId: 'session' } }));
    window.dispatchEvent(new CustomEvent('expand-right-panel'));
    window.dispatchEvent(new CustomEvent('agent-create-tab', {
      detail: {
        type: 'code-editor',
        title: args.fileName,
        data: {
          filePath: args.filePath,
          fileName: args.fileName,
          language: 'typescript',
          readOnly: true,
          showLineNumbers: true,
          showMinimap: false,
          theme: 'vs-dark',
        },
        metadata: {
          filePath: args.filePath,
          fileName: args.fileName,
          duplicateCheckKey: duplicateKey,
        },
        checkDuplicate: false,
        duplicateCheckKey: duplicateKey,
        replaceExisting: false,
      },
    }));
    return triggerAt;
  }, { filePath, fileName });
}

async function triggerGitDiff(
  workspacePath: string,
  filePath: string,
  fileName: string,
): Promise<number> {
  const content = await fs.readFile(filePath, 'utf8');
  const relativePath = path.relative(workspacePath, filePath).replace(/\\/g, '/');
  const modifiedContent = `${content}\n// perf measurement synthetic diff\n`;

  return browser.execute((args) => {
    const triggerAt = performance.now();
    const duplicateKey = `perf-git-diff:${args.relativePath}:${triggerAt}`;
    window.dispatchEvent(new CustomEvent('git-create-tab', {
      detail: {
        type: 'diff-code-editor',
        title: `${args.fileName} - Git Diff`,
        data: {
          fileName: args.fileName,
          filePath: args.relativePath,
          language: 'typescript',
          originalCode: args.originalContent,
          modifiedCode: args.modifiedContent,
          readOnly: true,
          repositoryPath: args.workspacePath,
        },
        metadata: {
          filePath: args.relativePath,
          repositoryPath: args.workspacePath,
          duplicateCheckKey: duplicateKey,
        },
        checkDuplicate: false,
        duplicateCheckKey: duplicateKey,
        replaceExisting: false,
      },
    }));
    return triggerAt;
  }, {
    workspacePath,
    relativePath,
    fileName,
    originalContent: content,
    modifiedContent,
  });
}

async function waitForEditorReady(scenario: EditorScenario): Promise<{ atMs: number; count: number }> {
  const selector = editorSelector(scenario);

  await browser.waitUntil(async () => {
    const count = await countMatchingEditors(scenario);
    return count > 0;
  }, {
    timeout: 30000,
    interval: 50,
    timeoutMsg: `Timed out waiting for ${scenario} Monaco editor`,
  });

  const result = await browser.execute((query) => ({
    atMs: performance.now(),
    count: document.querySelectorAll(query).length,
  }), selector);
  return result as { atMs: number; count: number };
}

function editorSelector(scenario: EditorScenario): string {
  return scenario === 'git-diff'
    ? '.monaco-diff-editor, .monaco-editor'
    : '.monaco-editor';
}

async function countMatchingEditors(scenario: EditorScenario): Promise<number> {
  const selector = editorSelector(scenario);
  return browser.execute((query) => document.querySelectorAll(query).length, selector);
}

describe('Editor first-open performance telemetry', () => {
  before(async () => {
    await waitForTracePhaseCount('interactive_shell_ready', 1, 30000);
    if (shouldWaitForEditorWarmup()) {
      await waitForTracePhaseCount('editor_startup_warmup_end', 1, 30000);
    }
  });

  it('collects first-open timing for editor-heavy surfaces', async () => {
    const scenario = getScenario();
    const workspacePath = getWorkspacePath();
    const filePath = getEditorFilePath(workspacePath);
    const fileName = fileNameOf(filePath);

    if (scenario === 'git-diff') {
      await openGitScene();
    } else {
      await openSessionScene();
    }
    const existingEditorCount = await countMatchingEditors(scenario);
    if (existingEditorCount > 0) {
      throw new Error(`Expected no existing ${scenario} editor before first-open measurement; found ${existingEditorCount}`);
    }

    const snapshotBefore = await readStartupTraceSnapshot();
    const editorWarmupStartAtMs = lastPhaseAt(snapshotBefore, 'editor_startup_warmup_start');
    const editorWarmupEndAtMs = lastPhaseAt(snapshotBefore, 'editor_startup_warmup_end');
    const triggerAtMs = scenario === 'git-diff'
      ? await triggerGitDiff(workspacePath, filePath, fileName)
      : await triggerCodeEditor(filePath, fileName);
    const ready = await waitForEditorReady(scenario);
    const trace = await readStartupTraceSnapshot();

    const report: EditorOpenReport = {
      appMode: process.env.BITFUN_E2E_APP_MODE ?? 'auto',
      scenario,
      traceId: trace.traceId || snapshotBefore.traceId,
      filePath,
      waitedForEditorWarmup: shouldWaitForEditorWarmup(),
      editorWarmupStartAtMs,
      editorWarmupEndAtMs,
      editorWarmupDurationMs:
        typeof editorWarmupStartAtMs === 'number' && typeof editorWarmupEndAtMs === 'number'
          ? editorWarmupEndAtMs - editorWarmupStartAtMs
          : undefined,
      editorWarmupCompletedBeforeTrigger:
        typeof editorWarmupEndAtMs === 'number' && editorWarmupEndAtMs <= triggerAtMs,
      triggerAtMs,
      editorReadyAtMs: ready.atMs,
      triggerToEditorReadyMs: ready.atMs - triggerAtMs,
      matchingEditorCount: ready.count,
    };

    console.log('[Perf] editor-first-open', JSON.stringify(report));
    await writeReport(`editor-first-open-${scenario}`, report);

    expect(report.triggerToEditorReadyMs).toBeGreaterThan(0);
    expect(report.matchingEditorCount).toBeGreaterThan(0);
    expect(await readPerformanceNow()).toBeGreaterThan(report.triggerAtMs);
  });
});
