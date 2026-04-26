import { describe, expect, it } from 'vitest';

import {
  buildSessionModelMigrationNotice,
  normalizeSessionModelMigrationReason,
  shouldSuppressSessionModelMigrationNotice,
} from './sessionModelMigrationNotice';

const translations: Record<string, string> = {
  'flow-chat:model.autoMigrated.title': '模型已自动切换',
  'flow-chat:model.autoMigrated.description':
    '当前会话之前使用的模型配置已不可用，系统已切换为自动选择模型。',
  'flow-chat:model.autoMigrated.reasons.modelUnavailableOnRestore':
    '这是恢复历史会话时的自动处理，你可以继续使用，或稍后重新指定模型。',
  'flow-chat:model.autoMigrated.reasons.modelReconciled':
    '模型配置刚发生过调整，系统已为当前会话自动切换到可用模型。',
  'flow-chat:model.autoMigrated.reasons.fallback': '系统已为当前会话自动选择可用模型。',
};

function t(key: string): string {
  return translations[key] ?? key;
}

describe('sessionModelMigrationNotice', () => {
  it('maps restore reasons to friendly copy and a stable dedupe key', () => {
    const notice = buildSessionModelMigrationNotice(
      {
        sessionId: 'session-1',
        newModelId: 'auto',
        reason: 'model_unavailable_on_restore',
      },
      t
    );

    expect(notice.title).toBe('模型已自动切换');
    expect(notice.message).toBe(
      '当前会话之前使用的模型配置已不可用，系统已切换为自动选择模型。 这是恢复历史会话时的自动处理，你可以继续使用，或稍后重新指定模型。'
    );
    expect(notice.dedupeKey).toBe('session-1:auto:modelUnavailableOnRestore');
  });

  it('falls back to generic copy for unknown reasons', () => {
    expect(normalizeSessionModelMigrationReason('unexpected_reason')).toBe('fallback');
  });

  it('suppresses duplicate notices within the quiet window', () => {
    const recentNoticeTimestamps = new Map<string, number>();
    const dedupeKey = 'session-1:auto:modelUnavailableOnRestore';

    expect(
      shouldSuppressSessionModelMigrationNotice(recentNoticeTimestamps, dedupeKey, 1_000, 2_000)
    ).toBe(false);
    expect(
      shouldSuppressSessionModelMigrationNotice(recentNoticeTimestamps, dedupeKey, 1_500, 2_000)
    ).toBe(true);
    expect(
      shouldSuppressSessionModelMigrationNotice(recentNoticeTimestamps, dedupeKey, 3_500, 2_000)
    ).toBe(false);
  });
});
