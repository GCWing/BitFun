import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import zhCnPolicy from '../../../../../crates/services/services-integrations/src/privacy/assets/zh-CN.md?raw';

export type PrivacyLifecycleState =
  | 'choice_required'
  | 'full'
  | 'privacy_not_accepted'
  | 'resource_error';
export type PrivacyEffectiveMode = 'full' | 'privacy_not_accepted';
export type PrivacyChangeType = 'material' | 'editorial';

export interface PrivacyConsentRecord {
  consentVersion: string;
  acceptedPolicyUpdatedAt: string;
  acceptedDocumentSha256: string;
  acceptedAt: string;
  locale: string;
  appVersion: string;
}

export interface PrivacyPolicyView {
  consentVersion: string;
  changeType: PrivacyChangeType;
  effectiveAt: string;
  updatedAt: string;
  locale: string;
  documentSha256: string;
  content: string;
}

export interface PrivacyStatus {
  enabled: boolean;
  lifecycleState: PrivacyLifecycleState;
  effectiveMode: PrivacyEffectiveMode;
  releaseReady: boolean;
  hasUnreadUpdate: boolean;
  policy?: PrivacyPolicyView;
  consent?: PrivacyConsentRecord;
  configurationError?: string;
}

export interface AcceptPrivacyRequest {
  policyUpdatedAt: string;
  consentVersion: string;
  documentSha256: string;
  locale: string;
}

export const disabledPrivacyStatus: PrivacyStatus = {
  enabled: false,
  lifecycleState: 'full',
  effectiveMode: 'full',
  releaseReady: true,
  hasUnreadUpdate: false,
};

const bundledPreviewPolicies = {
  'zh-CN': {
    content: zhCnPolicy,
    documentSha256: 'af8e3bff76330d3910ab2343a083922f0895d04f3e497c9cd02472cc2a5d9987',
  },
} as const;

export function bundledPrivacyPreviewStatus(_locale: string): PrivacyStatus {
  const policyLocale = 'zh-CN';
  const policy = bundledPreviewPolicies[policyLocale];
  return {
    ...disabledPrivacyStatus,
    policy: {
      consentVersion: '4',
      changeType: 'editorial',
      effectiveAt: '2026-07-30T00:00:00Z',
      updatedAt: '2026-07-30T00:00:00Z',
      locale: policyLocale,
      documentSha256: policy.documentSha256,
      content: policy.content,
    },
  };
}

export class PrivacyAPI {
  async initialize(): Promise<PrivacyStatus> {
    return this.invoke('privacy_initialize', {});
  }

  async getStatus(locale: string): Promise<PrivacyStatus> {
    return this.invoke('privacy_get_status', { locale });
  }

  async accept(request: AcceptPrivacyRequest): Promise<PrivacyStatus> {
    return this.invoke('privacy_accept', request);
  }

  async enterNotAccepted(locale: string): Promise<PrivacyStatus> {
    return this.invoke('privacy_enter_not_accepted', { locale });
  }

  async markViewed(policyUpdatedAt: string, locale: string): Promise<PrivacyStatus> {
    return this.invoke('privacy_mark_viewed', { policyUpdatedAt, locale });
  }

  async applyCollectionPolicy(
    mode: PrivacyEffectiveMode,
    locale: string,
  ): Promise<PrivacyStatus> {
    return this.invoke('privacy_apply_collection_policy', { mode, locale });
  }

  async showGateWindow(): Promise<void> {
    await api.invoke('show_main_window', {});
  }

  private async invoke<T>(command: string, request: object): Promise<T> {
    try {
      return await api.invoke<T>(command, { request }, { retries: 0 });
    } catch (error) {
      throw createTauriCommandError(command, error);
    }
  }
}

export const privacyAPI = new PrivacyAPI();
