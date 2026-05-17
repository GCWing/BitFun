import { describe, expect, it, vi } from 'vitest';
import {
  createStartupTrace,
  estimateJsonBytes,
  isRemoteTraceContext,
  isRemoteTraceRequest,
  markPhaseAfterAnimationFrames,
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
  it('records startup phases without exposing sensitive fields', () => {
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

    expect(logger.debug).toHaveBeenCalledTimes(1);
    const [, payload] = logger.debug.mock.calls[0];
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

    expect(logger.info).toHaveBeenCalledTimes(1);
    const [, payload] = logger.info.mock.calls[0];
    expect(payload).toMatchObject({
      traceId: 'trace-test',
      reason: 'test',
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

    const [, payload] = logger.info.mock.calls[0];
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

    expect(logger.debug).toHaveBeenCalledTimes(1);
    const [, payload] = logger.debug.mock.calls[0];
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
