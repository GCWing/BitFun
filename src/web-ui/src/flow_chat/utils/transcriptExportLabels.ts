/**
 * Localized label bundles for transcript copy / Markdown export.
 *
 * Keys are resolved inside the `flow-chat` namespace so both `useTranslation`
 * consumers and plain `i18nService` callers share one label source.
 */

import { i18nService } from '@/infrastructure/i18n';
import type {
  SessionTranscriptMarkdownLabels,
  TranscriptExportLabels,
} from './dialogTranscriptExport';

export type TranscriptTranslate = (key: string, options?: Record<string, unknown>) => string;

/** Bare keys resolve inside `flow-chat`; namespaced keys pass through. */
const defaultTranslate: TranscriptTranslate = (key, options) =>
  i18nService.t(key.includes(':') ? key : `flow-chat:${key}`, options);

export function buildTranscriptExportLabels(
  t: TranscriptTranslate = defaultTranslate
): TranscriptExportLabels {
  return {
    userLabel: t('modelRound.userLabel'),
    toolCallLabel: (toolName: string) => t('modelRound.toolCallLabel', { name: toolName }),
    unknownTool: t('copyOutput.unknownTool'),
    thinking: t('transcriptExport.thinking'),
    toolInput: t('transcriptExport.toolInput'),
    toolResult: t('transcriptExport.toolResult'),
    toolError: t('transcriptExport.toolError'),
    empty: t('transcriptExport.empty'),
  };
}

export function buildSessionTranscriptMarkdownLabels(
  t: TranscriptTranslate = defaultTranslate
): SessionTranscriptMarkdownLabels {
  return {
    ...buildTranscriptExportLabels(t),
    scopeFull: t('transcriptExport.scopeFull'),
    scopeResult: t('transcriptExport.scopeResult'),
    metaSessionId: t('transcriptExport.metaSessionId'),
    metaExportedAt: t('transcriptExport.metaExportedAt'),
    metaTurnCount: t('transcriptExport.metaTurnCount'),
    metaScope: t('transcriptExport.metaScope'),
    metaWorkspace: t('shared:features.workspace'),
    turnHeading: (index: number) => t('transcriptExport.turnHeading', { index }),
    userHeading: t('transcriptExport.userHeading'),
    assistantHeading: t('transcriptExport.assistantHeading'),
    toolHeading: (toolName: string) => t('transcriptExport.toolHeading', { name: toolName }),
  };
}
