import { describe, expect, it } from 'vitest';
import { sanitizeErrorForLog, sanitizeLogValue } from './logSanitizer';

describe('API log sanitizer', () => {
  it('fully redacts MiniApp provider tokens from request logs', () => {
    const token = 'Bearer ghp_superSecretTokenValue1234567890';
    const sanitized = sanitizeLogValue({
      command: 'miniapp_host_call',
      args: {
        request: {
          method: 'net.fetch',
          params: {
            url: 'https://api.github.com/repos/GCWing/BitFun/pulls',
            headers: {
              Authorization: token,
              'X-GitHub-Token': 'github_pat_11SECRETSECRETSECRET',
            },
            body: JSON.stringify({ token: 'ghp_bodySecretValue1234567890' }),
          },
        },
      },
    });

    const text = JSON.stringify(sanitized);
    expect(text).not.toContain('superSecretTokenValue');
    expect(text).not.toContain('TokenValue1234567890');
    expect(text).not.toContain('github_pat_11SECRET');
    expect(text).not.toContain('ghp_bodySecretValue');
    expect(text).not.toContain('Bear');
    expect(text).toContain('[redacted]');
  });

  it('redacts token-bearing error messages before logging', () => {
    const error = new Error('Provider failed with Authorization: Bearer ghp_errorSecretValue123456');
    const sanitized = sanitizeErrorForLog(error);

    const text = JSON.stringify(sanitized);
    expect(text).not.toContain('ghp_errorSecretValue');
    expect(text).toContain('[redacted]');
  });
});
