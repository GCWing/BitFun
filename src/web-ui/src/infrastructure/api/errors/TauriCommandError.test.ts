import { describe, expect, it } from 'vitest';
import { isSessionInUseError, TauriCommandError } from './TauriCommandError';

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
