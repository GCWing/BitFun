import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export interface MarketUserSummary {
  githubId: number;
  login: string;
  avatarUrl: string;
}

export interface MarketMe {
  user: MarketUserSummary;
  isAdmin: boolean;
}

export interface DesktopAuthStart {
  transactionId: string;
  authorizationUrl: string;
  expiresAt: number;
  pollIntervalSeconds: number;
}

class AccountIdentityAPI {
  async authStart(): Promise<DesktopAuthStart> {
    try {
      return await api.invoke('account_github_start', {});
    } catch (error) {
      throw createTauriCommandError('account_github_start', error);
    }
  }

  async authPoll(transaction: DesktopAuthStart): Promise<'pending' | 'authorized' | 'expired'> {
    try {
      const response = await api.invoke<{ status: 'pending' | 'authorized' | 'expired' }>(
        'account_github_poll',
        {
          request: {
            transactionId: transaction.transactionId,
          },
        },
      );
      return response.status;
    } catch (error) {
      throw createTauriCommandError('account_github_poll', error);
    }
  }

  async me(): Promise<MarketMe | null> {
    try {
      return await api.invoke('account_github_info', {});
    } catch (error) {
      throw createTauriCommandError('account_github_info', error);
    }
  }

  async logout(): Promise<void> {
    try {
      await api.invoke('account_logout', {});
    } catch (error) {
      throw createTauriCommandError('account_logout', error);
    }
  }

  onAccountChanged(handler: () => void): () => void {
    return api.listen('account-identity-changed', handler);
  }
}

export const accountIdentityAPI = new AccountIdentityAPI();
