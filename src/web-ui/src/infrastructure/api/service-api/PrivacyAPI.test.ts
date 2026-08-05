import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('./ApiClient', () => ({ api: { invoke } }));

import { bundledPrivacyPreviewStatus, PrivacyAPI } from './PrivacyAPI';

describe('PrivacyAPI', () => {
  beforeEach(() => invoke.mockReset());

  it('uses structured requests for every privacy transition', async () => {
    invoke.mockResolvedValue({
      enabled: true,
      lifecycleState: 'privacy_not_accepted',
      effectiveMode: 'privacy_not_accepted',
    });
    const client = new PrivacyAPI();
    await client.enterNotAccepted('zh-TW');
    await client.applyCollectionPolicy('privacy_not_accepted', 'zh-TW');
    expect(invoke).toHaveBeenNthCalledWith(
      1,
      'privacy_enter_not_accepted',
      { request: { locale: 'zh-TW' } },
      { retries: 0 },
    );
    expect(invoke).toHaveBeenNthCalledWith(
      2,
      'privacy_apply_collection_policy',
      { request: { mode: 'privacy_not_accepted', locale: 'zh-TW' } },
      { retries: 0 },
    );
  });

  it('identifies policy acceptance and viewing by update timestamp', async () => {
    invoke.mockResolvedValue({
      enabled: true,
      lifecycleState: 'full',
      effectiveMode: 'full',
    });
    const client = new PrivacyAPI();
    await client.accept({
      policyUpdatedAt: '2026-07-30T00:00:00Z',
      consentVersion: '4',
      documentSha256: 'a'.repeat(64),
      locale: 'zh-CN',
    });
    await client.markViewed('2026-07-30T00:00:00Z', 'zh-CN');

    expect(invoke).toHaveBeenNthCalledWith(
      1,
      'privacy_accept',
      {
        request: {
          policyUpdatedAt: '2026-07-30T00:00:00Z',
          consentVersion: '4',
          documentSha256: 'a'.repeat(64),
          locale: 'zh-CN',
        },
      },
      { retries: 0 },
    );
    expect(invoke).toHaveBeenNthCalledWith(
      2,
      'privacy_mark_viewed',
      { request: { policyUpdatedAt: '2026-07-30T00:00:00Z', locale: 'zh-CN' } },
      { retries: 0 },
    );
    expect(JSON.stringify(invoke.mock.calls)).not.toContain('policyVersion');
  });

  it('uses the editorial Chinese document timestamp for every preview locale', () => {
    const status = bundledPrivacyPreviewStatus('en-US');
    expect(status.enabled).toBe(false);
    expect(status.lifecycleState).toBe('full');
    expect(status.policy?.locale).toBe('zh-CN');
    expect(status.policy?.content).toContain('关于HUAWEI BitFun的隐私协议');
    expect(status.policy?.content).not.toContain('开发测试占位版');
    expect(status.policy).not.toHaveProperty('policyVersion');
    expect(status.policy?.updatedAt).toBe('2026-07-30T00:00:00Z');
    expect(status.policy?.consentVersion).toBe('4');
    expect(status.policy?.changeType).toBe('editorial');
  });
});
