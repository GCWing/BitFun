import { i18nService } from '@/infrastructure/i18n/core/I18nService';
import type { TFunction } from 'i18next';
import type { ThreadGoalUiAction } from '../services/threadGoalActions';

const DEFAULT_AUTO_CONTINUATION_MAX = 100;

function coercePositiveInt(value: unknown): number | undefined {
  if (typeof value === 'number' && Number.isFinite(value) && value > 0) {
    return Math.trunc(value);
  }
  if (typeof value === 'string' && value.trim() !== '') {
    const parsed = Number.parseInt(value, 10);
    if (Number.isFinite(parsed) && parsed > 0) {
      return parsed;
    }
  }
  return undefined;
}

export function resolveThreadGoalUserMessageDisplay(
  content: string,
  metadata: Record<string, unknown> | undefined | null
): string {
  if (!metadata) {
    return content;
  }
  if (metadata.threadGoalObjectiveUpdated && typeof metadata.objective === 'string') {
    return i18nService.t('flow-chat:threadGoal.updatedUserMessage', {
      objective: metadata.objective,
    });
  }
  if (metadata.threadGoalKickoff && typeof metadata.threadGoalObjective === 'string') {
    return i18nService.t('flow-chat:threadGoal.kickoffUserMessage', {
      objective: metadata.threadGoalObjective,
    });
  }
  if (metadata.threadGoalContinuation && typeof metadata.objective === 'string') {
    return i18nService.t('flow-chat:threadGoal.continuationCheckUserMessage', {
      objective: metadata.objective,
    });
  }
  return content;
}

export function resolveThreadGoalHeaderTitle(
  metadata: Record<string, unknown> | undefined | null
): string | null {
  if (!metadata) {
    return null;
  }
  if (metadata.threadGoalContinuation) {
    const attempt = coercePositiveInt(metadata.autoContinuationAttempt);
    const max = coercePositiveInt(metadata.autoContinuationMax) ?? DEFAULT_AUTO_CONTINUATION_MAX;
    if (attempt != null) {
      return i18nService.t('flow-chat:threadGoal.continuationCheckHeaderWithAttempt', {
        attempt,
        max,
      });
    }
    return i18nService.t('flow-chat:threadGoal.continuationCheckHeader');
  }
  if (metadata.threadGoalKickoff || metadata.threadGoalObjectiveUpdated) {
    return i18nService.t('flow-chat:threadGoal.menuTitle');
  }
  return null;
}

export function resolveThreadGoalStatusLabel(t: TFunction, status: string): string {
  if (status === 'complete') {
    return t('shared:statuses.done');
  }
  return t(`threadGoal.status.${status}`, { defaultValue: status });
}

export function resolveThreadGoalActionLabel(
  t: TFunction,
  action: ThreadGoalUiAction
): string {
  if (action === 'edit') {
    return t('shared:tools.edit');
  }
  return t(`threadGoal.action.${action}`);
}
