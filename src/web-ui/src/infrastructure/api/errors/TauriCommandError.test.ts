import { describe, expect, it } from 'vitest';
import {
  isHarnessProfileLockedError,
  isNotAvailableError,
  isOutcomeUnknownError,
  isSessionInUseError,
  TauriCommandError,
} from './TauriCommandError';

describe('isHarnessProfileLockedError', () => {
  it('recognizes the stable code from local Desktop and server validation errors', () => {
    expect(
      isHarnessProfileLockedError(
        new TauriCommandError('Command failed', {
          command: 'update_session_harness_profile',
          originalError: 'harness_profile_locked: Session already started',
        }),
      ),
    ).toBe(true);
    expect(
      isHarnessProfileLockedError({
        message:
          'Host command failed: Validation error: harness_profile_locked: Session already started',
      }),
    ).toBe(true);
  });

  it('does not infer the lock from similar human prose', () => {
    expect(
      isHarnessProfileLockedError(
        new Error('The Harness profile is locked because this session already started'),
      ),
    ).toBe(false);
  });
});

describe('isSessionInUseError', () => {
  it('recognizes local Tauri command errors without parsing human prose', () => {
    const error = new TauriCommandError('Command failed', {
      command: 'ensure_coordinator_session',
      originalError: new Error(
        'session_in_use: Session is already open for writing: session-1',
      ),
    });

    expect(isSessionInUseError(error)).toBe(true);
  });

  it('recognizes the same stable prefix through Peer error wrapping', () => {
    const error = {
      message: 'Host command failed',
      details: {
        originalError:
          'session_in_use: Session is already open for writing: session-1',
      },
    };

    expect(isSessionInUseError(error)).toBe(true);
  });

  it('does not classify similar human prose as the stable error', () => {
    expect(
      isSessionInUseError(
        new Error('This session seems to be in use by another process'),
      ),
    ).toBe(false);
  });
});

describe('isOutcomeUnknownError', () => {
  it('recognizes the stable rename error through Tauri and Peer wrappers', () => {
    expect(
      isOutcomeUnknownError(
        new TauriCommandError('Command failed', {
          command: 'update_session_title',
          originalError: 'outcome_unknown: inspect authoritative state',
        }),
      ),
    ).toBe(true);
    expect(
      isOutcomeUnknownError({
        message: 'Host command failed',
        details: { originalError: 'outcome_unknown: inspect authoritative state' },
      }),
    ).toBe(true);
  });

  it('does not infer unknown outcomes from human prose', () => {
    expect(isOutcomeUnknownError(new Error('The rename might have worked'))).toBe(false);
  });
});

describe('isNotAvailableError', () => {
  it('recognizes a stable unsupported capability prefix through wrappers', () => {
    expect(
      isNotAvailableError({
        context: { originalError: 'not_available: future profile is unsupported' },
      }),
    ).toBe(true);
  });

  it('does not infer unsupported state from prose', () => {
    expect(isNotAvailableError(new Error('This feature is unavailable'))).toBe(false);
  });
});
