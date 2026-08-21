import { describe, expect, it } from 'vitest';
import {
  SUBSCRIPTION_AUTH_OPTION_VALUES,
  buildAuthSelectValue,
  parseAuthSelectValue,
  subscriptionStatusKind,
  buildSubscriptionAccountDescription,
} from './subscriptionAuthOptions';

describe('subscriptionAuthOptions', () => {
  it('lists seven auth options including CodeBuddy and Qoder', () => {
    expect(SUBSCRIPTION_AUTH_OPTION_VALUES).toEqual([
      'api_key',
      'subscription:codex',
      'subscription:antigravity',
      'subscription:opencode:zen',
      'subscription:opencode:go',
      'subscription:codebuddy',
      'subscription:qoder',
    ]);
    expect(SUBSCRIPTION_AUTH_OPTION_VALUES).toContain('subscription:codebuddy');
    expect(SUBSCRIPTION_AUTH_OPTION_VALUES).toContain('subscription:qoder');
  });

  it('builds auth select values for non-opencode providers', () => {
    expect(buildAuthSelectValue('codebuddy')).toBe('subscription:codebuddy');
    expect(buildAuthSelectValue('qoder')).toBe('subscription:qoder');
    expect(buildAuthSelectValue('opencode', 'zen')).toBe('subscription:opencode:zen');
  });

  it('round-trips auth select values', () => {
    expect(parseAuthSelectValue('subscription:codebuddy')).toEqual({
      kind: 'subscription',
      provider: 'codebuddy',
      plan: undefined,
    });
    expect(parseAuthSelectValue('subscription:qoder')).toEqual({
      kind: 'subscription',
      provider: 'qoder',
      plan: undefined,
    });
    expect(parseAuthSelectValue('subscription:opencode:go')).toEqual({
      kind: 'subscription',
      provider: 'opencode',
      plan: 'go',
    });
    expect(parseAuthSelectValue('api_key').kind).toBe('api_key');
  });

  it('classifies signed-in and signed-out panel states', () => {
    expect(subscriptionStatusKind({ connected: true })).toBe('connected');
    expect(subscriptionStatusKind({ connected: false, vault_unavailable: true })).toBe('vault_unavailable');
    expect(subscriptionStatusKind({ connected: false, reauthentication_required: true })).toBe('reauthentication_required');
    expect(subscriptionStatusKind({ connected: false })).toBe('not_signed_in');
  });

  it('describes a signed-in account with token validity', () => {
    const t = (key: string) => key;
    const parts = buildSubscriptionAccountDescription(
      { connected: true, account: 'user-1', expires_at: null },
      t,
    );
    expect(parts).toContain('user-1');
    expect(parts).toContain('subscriptionAuth.tokenValid');
  });

  it('describes an expired signed-in account with expiry time', () => {
    const t = (key: string) => key;
    const parts = buildSubscriptionAccountDescription(
      { connected: true, account: 'user-1', expires_at: 1700000000 },
      t,
      () => '2030-11-14',
    );
    expect(parts).toContain('user-1');
    expect(parts.some((part) => part.startsWith('subscriptionAuth.expiresAt')))
      .toBe(true);
  });

  it('describes signed-out / vault / reauth states', () => {
    const t = (key: string) => key;
    expect(buildSubscriptionAccountDescription({ connected: false }, t))
      .toContain('subscriptionAuth.notSignedIn');
    expect(buildSubscriptionAccountDescription({ connected: false, vault_unavailable: true }, t))
      .toContain('subscriptionAuth.vaultUnavailable');
    expect(buildSubscriptionAccountDescription({ connected: false, reauthentication_required: true }, t))
      .toContain('subscriptionAuth.reauthenticationRequired');
  });
});
