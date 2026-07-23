import { describe, expect, it, vi } from 'vitest';
import {
  createStartupTrace,
  estimateJsonBytes,
  isRemoteTraceContext,
  isRemoteTraceRequest,
  isStartupRenderTraceEnabled,
  markPhaseAfterAnimationFrames,
  recordReactRenderProfile,
} from './startupTrace';
import type { LoggerLike } from './timing';

function createTestLogger(): LoggerLike & {
  debug: ReturnType<typeof vi.fn>;
  info: ReturnType<typeof vi.fn>;
  warn: ReturnType<typeof vi.fn>;
} {
  return {
    trace: vi.fn(),
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  };
}

describe('startupTrace', () => {
  it('records startup phases without exposing sensitive fields or writing event logs by default', () => {
    const logger = createTestLogger();
    const trace = createStartupTrace({
      logger,
      traceId: 'trace-test',
      now: () => 100,
    });

    trace.markPhase('before_render_start', {
      apiKey: 'secret',
      command: 'get_config',
      request: { nested: 'payload' },
      remoteConnectionId: 'ssh-user@example.com',
      sshHost: 'example.internal',
      remote: true,
    });

    expect(logger.debug).not.toHaveBeenCalled();
    const payload = trace.getSnapshot().phases.events[0];
    expect(payload).toMatchObject({
      traceId: 'trace-test',
      phase: 'before_render_start',
      command: 'get_config',
      remote: true,
    });
    expect(payload).not.toHaveProperty('apiKey');
    expect(payload).not.toHaveProperty('request');
    expect(payload).not.toHaveProperty('remoteConnectionId');
    expect(payload).not.toHaveProperty('sshHost');
  });

  it('records react render profiles only when perf trace is explicitly enabled', () => {
    const previousRenderProfileEnabled = globalThis.__BITFUN_RENDER_PROFILE_ENABLED__;
    const trace = createStartupTrace({
      traceId: 'trace-test',
      now: () => 100,
    });

    try {
      globalThis.__BITFUN_RENDER_PROFILE_ENABLED__ = false;
      expect(isStartupRenderTraceEnabled()).toBe(false);
      recordReactRenderProfile(trace, {
        component: 'MarkdownRenderer',
        phase: 'mount',
        actualDurationMs: 12.345,
        baseDurationMs: 20.5,
        startTimeMs: 10,
        commitTimeMs: 25,
        contentLength: 1024,
        itemCount: 12,
        groupCount: 7,
        renderedCount: 5,
        turnId: 'turn-1',
        roundId: 'round-1',
        itemId: 'item-1',
        visibleGroupStartIndex: 2,
        visibleGroupEndIndex: 7,
        textItemCount: 4,
        toolItemCount: 8,
        visibleTextItemCount: 2,
        visibleToolItemCount: 3,
        criticalGroupCount: 5,
        exploreGroupCount: 2,
        hasCodeBlock: true,
        request: { unsafe: 'payload' },
      });
      expect(trace.getSnapshot().phases.events).toHaveLength(0);

      globalThis.__BITFUN_RENDER_PROFILE_ENABLED__ = true;
      expect(isStartupRenderTraceEnabled()).toBe(true);
      recordReactRenderProfile(trace, {
        component: 'MarkdownRenderer',
        phase: 'mount',
        actualDurationMs: 12.345,
        baseDurationMs: 20.5,
        startTimeMs: 10,
        commitTimeMs: 25,
        contentLength: 1024,
        itemCount: 12,
        groupCount: 7,
        renderedCount: 5,
        turnId: 'turn-1',
        roundId: 'round-1',
        itemId: 'item-1',
        visibleGroupStartIndex: 2,
        visibleGroupEndIndex: 7,
        textItemCount: 4,
        toolItemCount: 8,
        visibleTextItemCount: 2,
        visibleToolItemCount: 3,
        criticalGroupCount: 5,
        exploreGroupCount: 2,
        hasCodeBlock: true,
        request: { unsafe: 'payload' },
      });

      expect(trace.getSnapshot().phases.events).toEqual([
        expect.objectContaining({
          phase: 'react_render_profile',
          component: 'MarkdownRenderer',
          renderPhase: 'mount',
          actualDurationMs: 12.3,
          baseDurationMs: 20.5,
          startTimeMs: 10,
          commitTimeMs: 25,
          contentLength: 1024,
          itemCount: 12,
          groupCount: 7,
          renderedCount: 5,
          turnId: 'turn-1',
          roundId: 'round-1',
          itemId: 'item-1',
          visibleGroupStartIndex: 2,
          visibleGroupEndIndex: 7,
          textItemCount: 4,
          toolItemCount: 8,
          visibleTextItemCount: 2,
          visibleToolItemCount: 3,
          criticalGroupCount: 5,
          exploreGroupCount: 2,
          hasCodeBlock: true,
        }),
      ]);
      expect(trace.getSnapshot().phases.events[0]).not.toHaveProperty('request');
    } finally {
      if (previousRenderProfileEnabled === undefined) {
        delete globalThis.__BITFUN_RENDER_PROFILE_ENABLED__;
      } else {
        globalThis.__BITFUN_RENDER_PROFILE_ENABLED__ = previousRenderProfileEnabled;
      }
    }
  });

  it('logs sanitized phase events only when explicitly enabled', () => {
    const logger = createTestLogger();
    const trace = createStartupTrace({
      logger,
      traceId: 'trace-test',
      now: () => 100,
      logEvents: true,
    });

    trace.markPhase('before_render_start', {
      command: 'get_config',
      request: { nested: 'payload' },
      remote: true,
    });

    expect(logger.debug).toHaveBeenCalledTimes(1);
    const [, payload] = logger.debug.mock.calls[0];
    expect(payload).toMatchObject({
      traceId: 'trace-test',
      phase: 'before_render_start',
      command: 'get_config',
      remote: true,
    });
    expect(payload).not.toHaveProperty('request');
  });

  it('aggregates API calls by command and remote status', () => {
    const logger = createTestLogger();
    const trace = createStartupTrace({
      logger,
      traceId: 'trace-test',
      now: () => 100,
    });

    trace.recordApiCall({
      type: 'tauri',
      command: 'list_persisted_sessions',
      durationMs: 12.4,
      startedAtMs: 100,
      endedAtMs: 112.4,
      requestBytes: 100,
      responseBytes: 500,
      remote: true,
      cacheOutcome: 'miss',
    });
    trace.recordApiCall({
      type: 'tauri',
      command: 'list_persisted_sessions',
      durationMs: 7.6,
      requestBytes: 80,
      responseBytes: 300,
      remote: true,
      cacheOutcome: 'hit',
    });
    trace.recordApiCall({
      type: 'tauri',
      command: 'get_config',
      target: 'font',
      durationMs: 5,
      requestBytes: 40,
      responseBytes: 60,
      remote: false,
    });
    trace.recordApiCall({
      type: 'tauri',
      command: 'git_get_status',
      durationMs: 8,
      requestBytes: 20,
      remote: false,
      outcome: 'failure',
    });

    trace.flushSummary('test');

    expect(logger.info).not.toHaveBeenCalled();
    const payload = trace.getSnapshot();
    expect(payload).toMatchObject({
      traceId: 'trace-test',
      phases: {
        events: [],
      },
      api: {
        totalCount: 4,
        successCount: 3,
        failureCount: 1,
        cacheHitCount: 1,
        cacheMissCount: 1,
        cacheUnknownCount: 2,
        remoteCount: 2,
        requestBytes: 240,
        responseBytes: 860,
      },
    });
    expect(payload.api.byCommand).toEqual([
      {
        command: 'list_persisted_sessions',
        count: 2,
        successCount: 2,
        failureCount: 0,
        cacheHitCount: 1,
        cacheMissCount: 1,
        cacheUnknownCount: 0,
        remoteCount: 2,
        totalDurationMs: 20,
        maxDurationMs: 12.4,
        requestBytes: 180,
        responseBytes: 800,
      },
      {
        command: 'git_get_status',
        count: 1,
        successCount: 0,
        failureCount: 1,
        cacheHitCount: 0,
        cacheMissCount: 0,
        cacheUnknownCount: 1,
        remoteCount: 0,
        totalDurationMs: 8,
        maxDurationMs: 8,
        requestBytes: 20,
        responseBytes: 0,
      },
      {
        command: 'get_config',
        count: 1,
        successCount: 1,
        failureCount: 0,
        cacheHitCount: 0,
        cacheMissCount: 0,
        cacheUnknownCount: 1,
        remoteCount: 0,
        totalDurationMs: 5,
        maxDurationMs: 5,
        requestBytes: 40,
        responseBytes: 60,
      },
    ]);
    expect(payload.api.calls[0]).toMatchObject({
      traceId: 'trace-test',
      type: 'tauri',
      command: 'list_persisted_sessions',
      startedAtMs: 100,
      endedAtMs: 112.4,
      durationMs: 12.4,
      outcome: 'success',
      cacheOutcome: 'miss',
      requestBytes: 100,
      responseBytes: 500,
      remote: true,
    });
    expect(payload.api.calls[2]).toMatchObject({
      command: 'get_config',
      target: 'font',
    });
  });

  it('records API boundary timing and concurrency fields for bottleneck attribution', () => {
    const trace = createStartupTrace({
      logger: createTestLogger(),
      traceId: 'trace-test',
      now: () => 100,
    });

    trace.recordApiCall({
      type: 'tauri',
      command: 'list_persisted_sessions_page',
      durationMs: 120.4,
      startedAtMs: 200,
      endedAtMs: 320.4,
      requestBytes: 50,
      responseBytes: 500,
      remote: false,
      requestPayloadEstimateDurationMs: 1.2,
      responsePayloadEstimateDurationMs: 2.3,
      adapterInitDurationMs: 4.5,
      transportDurationMs: 112.1,
      activeRequestsAtStart: 3,
      activeRequestsAtEnd: 2,
      maxConcurrentRequests: 5,
    });

    const [call] = trace.getSnapshot().api.calls;
    expect(call).toMatchObject({
      command: 'list_persisted_sessions_page',
      durationMs: 120.4,
      requestPayloadEstimateDurationMs: 1.2,
      responsePayloadEstimateDurationMs: 2.3,
      payloadEstimateDurationMs: 3.5,
      adapterInitDurationMs: 4.5,
      transportDurationMs: 112.1,
      activeRequestsAtStart: 3,
      activeRequestsAtEnd: 2,
      maxConcurrentRequests: 5,
    });
  });

  it('flushes bounded phase records so early events survive logger startup timing', () => {
    const logger = createTestLogger();
    let now = 10;
    const trace = createStartupTrace({
      logger,
      traceId: 'trace-test',
      now: () => now,
      maxPhaseEvents: 2,
    });

    trace.markPhase('first_script_eval', { remote: false });
    now = 20;
    trace.markPhase('before_render_start');
    now = 30;
    trace.markPhase('ignored_after_limit');
    trace.flushSummary('test');

    trace.flushSummary('test');

    expect(logger.info).not.toHaveBeenCalled();
    const payload = trace.getSnapshot();
    expect(payload.phases).toMatchObject({
      count: 2,
      events: [
        {
          traceId: 'trace-test',
          phase: 'first_script_eval',
          atMs: 10,
          remote: false,
        },
        {
          traceId: 'trace-test',
          phase: 'before_render_start',
          atMs: 20,
        },
      ],
    });
  });

  it('returns a sanitized immutable snapshot for performance E2E collection', () => {
    const logger = createTestLogger();
    let now = 100;
    const trace = createStartupTrace({
      logger,
      traceId: 'trace-test',
      now: () => now,
    });

    trace.markPhase('historical_session_hydrate_start', {
      sessionTraceId: 'session-1',
      workspacePath: '/workspace/BitFun',
      remote: false,
    });
    trace.recordApiCall({
      type: 'tauri',
      command: 'restore_session_view',
      durationMs: 42.4,
      responseBytes: 2048,
      remote: false,
      cacheOutcome: 'unknown',
    });

    const snapshot = trace.getSnapshot();
    expect(snapshot).toMatchObject({
      traceId: 'trace-test',
      phases: {
        count: 1,
        events: [
          {
            traceId: 'trace-test',
            phase: 'historical_session_hydrate_start',
            sessionTraceId: 'session-1',
            atMs: 100,
            remote: false,
          },
        ],
      },
      api: {
        totalCount: 1,
        successCount: 1,
        calls: [
          {
            command: 'restore_session_view',
            durationMs: 42.4,
            outcome: 'success',
          },
        ],
        byCommand: [
          {
            command: 'restore_session_view',
            count: 1,
            totalDurationMs: 42.4,
            responseBytes: 2048,
          },
        ],
      },
    });
    expect(snapshot.phases.events[0]).not.toHaveProperty('workspacePath');

    snapshot.phases.events.push({
      traceId: 'mutated',
      phase: 'mutated',
      atMs: 0,
    });
    expect(trace.getSnapshot().phases.events).toHaveLength(1);
  });

  it('does not log when disabled', () => {
    const logger = createTestLogger();
    const trace = createStartupTrace({
      enabled: false,
      logger,
      traceId: 'trace-test',
      now: () => 100,
    });

    trace.markPhase('first_script_eval');
    trace.recordApiCall({
      type: 'tauri',
      command: 'get_config',
      durationMs: 1,
      remote: false,
    });
    trace.flushSummary('disabled');

    expect(logger.debug).not.toHaveBeenCalled();
    expect(logger.info).not.toHaveBeenCalled();
  });

  it('marks deferred phases only after the requested animation frames', () => {
    const logger = createTestLogger();
    let now = 100;
    const callbacks: Array<(time: number) => void> = [];
    const trace = createStartupTrace({
      logger,
      traceId: 'trace-test',
      now: () => now,
    });

    markPhaseAfterAnimationFrames(trace, 'historical_session_first_paint', {
      sessionTraceId: 'session-trace',
      remote: false,
    }, {
      frameCount: 2,
      now: () => now,
      requestAnimationFrame: callback => {
        callbacks.push(callback);
        return callbacks.length;
      },
    });

    expect(logger.debug).not.toHaveBeenCalled();
    expect(callbacks).toHaveLength(1);

    now = 116;
    callbacks.shift()?.(now);
    expect(logger.debug).not.toHaveBeenCalled();
    expect(callbacks).toHaveLength(1);

    now = 132;
    callbacks.shift()?.(now);

    expect(logger.debug).not.toHaveBeenCalled();
    const payload = trace.getSnapshot().phases.events[0];
    expect(payload).toMatchObject({
      traceId: 'trace-test',
      phase: 'historical_session_first_paint',
      sessionTraceId: 'session-trace',
      remote: false,
      durationMs: 32,
    });
  });

  it('uses the desktop injected trace id when available', () => {
    const previousTraceId = (globalThis as { __BITFUN_STARTUP_TRACE_ID__?: string })
      .__BITFUN_STARTUP_TRACE_ID__;
    (globalThis as { __BITFUN_STARTUP_TRACE_ID__?: string }).__BITFUN_STARTUP_TRACE_ID__ =
      'desktop-123';

    try {
      const trace = createStartupTrace({
        logger: createTestLogger(),
        now: () => 100,
      });

      expect(trace.traceId).toBe('desktop-123');
    } finally {
      if (previousTraceId === undefined) {
        delete (globalThis as { __BITFUN_STARTUP_TRACE_ID__?: string })
          .__BITFUN_STARTUP_TRACE_ID__;
      } else {
        (globalThis as { __BITFUN_STARTUP_TRACE_ID__?: string }).__BITFUN_STARTUP_TRACE_ID__ =
          previousTraceId;
      }
    }
  });
});

describe('startupTrace payload helpers', () => {
  it('estimates JSON payload size with a hard cap', () => {
    const value = {
      small: 'ok',
      large: 'x'.repeat(10_000),
    };

    expect(estimateJsonBytes(value, 128)).toBe(128);
  });

  it('detects remote requests without needing full payload serialization', () => {
    expect(isRemoteTraceRequest({
      request: {
        remoteConnectionId: 'ssh-user@example.com',
      },
    })).toBe(true);
    expect(isRemoteTraceRequest({
      request: {
        workspacePath: 'D:/workspace/bitfun',
      },
    })).toBe(false);
    expect(isRemoteTraceRequest({
      request: {
        sshHost: 'localhost',
      },
    })).toBe(false);
    expect(isRemoteTraceRequest({
      request: {
        sshHost: 'example.internal',
      },
    })).toBe(true);
    expect(isRemoteTraceRequest({
      request: {
        remoteSshHost: 'localhost',
      },
    })).toBe(false);
    expect(isRemoteTraceRequest({
      request: {
        remoteSshHost: 'example.internal',
      },
    })).toBe(true);
  });

  it('keeps local ssh hosts out of remote session counters', () => {
    expect(isRemoteTraceContext(undefined, 'localhost')).toBe(false);
    expect(isRemoteTraceContext(undefined, '127.0.0.1')).toBe(false);
    expect(isRemoteTraceContext('connection-1', 'localhost')).toBe(true);
    expect(isRemoteTraceContext(undefined, 'example.internal')).toBe(true);
  });
});
