import type { OpenCodePlan, SubscriptionProvider } from '../types';
import type { SubscriptionAccount } from '@/infrastructure/api/service-api/AIApi';

/**
 * Auth dropdown option values, in display order. The subscription surface is
 * driven by `SubscriptionProvider::ALL` on the backend, but the template
 * picker hard-codes its option list; keep this in sync with the backend list.
 */
export const SUBSCRIPTION_AUTH_OPTION_VALUES: ReadonlyArray<string> = [
  'api_key',
  'subscription:codex',
  'subscription:antigravity',
  'subscription:opencode:zen',
  'subscription:opencode:go',
  'subscription:codebuddy',
  'subscription:qoder',
];

export function isSubscriptionAuthOption(value: string): boolean {
  return SUBSCRIPTION_AUTH_OPTION_VALUES.includes(value);
}

/** Serializes the current auth selection into a Select option value. */
export function buildAuthSelectValue(
  provider: SubscriptionProvider,
  plan?: OpenCodePlan,
): string {
  return provider === 'opencode'
    ? `subscription:opencode:${plan || 'zen'}`
    : `subscription:${provider}`;
}

export type ParsedAuthSelectValue =
  | { kind: 'api_key' }
  | { kind: 'subscription'; provider: SubscriptionProvider; plan?: OpenCodePlan };

/** Parses an auth Select option value back into a provider + optional plan. */
export function parseAuthSelectValue(value: string): ParsedAuthSelectValue {
  if (value === 'api_key') return { kind: 'api_key' };
  const [, providerValue, planValue] = value.split(':');
  const provider = providerValue as SubscriptionProvider;
  const plan = provider === 'opencode'
    ? (planValue || 'zen') as OpenCodePlan
    : undefined;
  return { kind: 'subscription', provider, plan };
}

export type SubscriptionStatusKind =
  | 'connected'
  | 'vault_unavailable'
  | 'reauthentication_required'
  | 'not_signed_in';

/** Reduces account flags to the single status shown in the panel description. */
export function subscriptionStatusKind(account: SubscriptionAccount): SubscriptionStatusKind {
  if (account.connected) return 'connected';
  if (account.vault_unavailable) return 'vault_unavailable';
  if (account.reauthentication_required) return 'reauthentication_required';
  return 'not_signed_in';
}

/**
 * Builds the human-readable description lines shown next to each provider
 * in the subscription panel. `formatExpiry` is injectable for tests.
 */
export function buildSubscriptionAccountDescription(
  account: SubscriptionAccount,
  t: (key: string, params?: Record<string, unknown>) => string,
  formatExpiry?: (unixSeconds: number) => string,
): string[] {
  const parts: string[] = [];
  const kind = subscriptionStatusKind(account);
  if (kind === 'connected') {
    if (account.account) parts.push(account.account);
    if (account.expires_at) {
      const time = formatExpiry
        ? formatExpiry(account.expires_at)
        : String(account.expires_at);
      parts.push(t('subscriptionAuth.expiresAt', { time }));
    } else {
      parts.push(t('subscriptionAuth.tokenValid'));
    }
    return parts;
  }
  switch (kind) {
    case 'vault_unavailable':
      parts.push(t('subscriptionAuth.vaultUnavailable'));
      break;
    case 'reauthentication_required':
      parts.push(t('subscriptionAuth.reauthenticationRequired'));
      break;
    default:
      parts.push(t('subscriptionAuth.notSignedIn'));
  }
  return parts;
}
