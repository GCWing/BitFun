import { describe, expect, it } from 'vitest';
import { isExpectedTauriRequestError } from './tauri-adapter';

describe('Tauri adapter expected errors', () => {
  it('classifies optional get_config not found as expected', () => {
    expect(isExpectedTauriRequestError(
      'get_config',
      {
        request: {
          path: 'font',
          skipRetryOnNotFound: true,
        },
      },
      new Error("Config path not found: 'font'")
    )).toBe(true);
  });

  it('does not hide non-optional get_config failures', () => {
    expect(isExpectedTauriRequestError(
      'get_config',
      {
        request: {
          path: 'font',
        },
      },
      new Error("Config path not found: 'font'")
    )).toBe(false);
  });
});
