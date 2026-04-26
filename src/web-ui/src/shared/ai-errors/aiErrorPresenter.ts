export type AiErrorCategory =
  | 'network'
  | 'auth'
  | 'rate_limit'
  | 'context_overflow'
  | 'loop_detected'
  | 'timeout'
  | 'provider_quota'
  | 'provider_billing'
  | 'provider_unavailable'
  | 'permission'
  | 'invalid_request'
  | 'content_policy'
  | 'model_error'
  | 'unknown';

export type AiErrorActionCode =
  | 'retry'
  | 'continue'
  | 'open_model_settings'
  | 'switch_model'
  | 'wait_and_retry'
  | 'compress_context'
  | 'start_new_chat'
  | 'copy_diagnostics';

export interface AiErrorDetail {
  category?: AiErrorCategory;
  provider?: string;
  providerCode?: string;
  providerMessage?: string;
  requestId?: string;
  httpStatus?: number;
  retryable?: boolean;
  actionHints?: AiErrorActionCode[];
  rawMessage?: string;
}

export interface AiErrorAction {
  code: AiErrorActionCode;
  labelKey: string;
}

export interface AiErrorPresentation {
  category: AiErrorCategory;
  titleKey: string;
  messageKey: string;
  severity: 'warning' | 'error';
  retryable: boolean;
  actions: AiErrorAction[];
  diagnostics: string;
}

const ACTION_LABEL_KEYS: Record<AiErrorActionCode, string> = {
  retry: 'errors:ai.actions.retry',
  continue: 'errors:ai.actions.continue',
  open_model_settings: 'errors:ai.actions.openModelSettings',
  switch_model: 'errors:ai.actions.switchModel',
  wait_and_retry: 'errors:ai.actions.waitAndRetry',
  compress_context: 'errors:ai.actions.compressContext',
  start_new_chat: 'errors:ai.actions.startNewChat',
  copy_diagnostics: 'errors:ai.actions.copyDiagnostics',
};

const PRESENTATION_KEYS: Record<AiErrorCategory, { titleKey: string; messageKey: string; severity: 'warning' | 'error' }> = {
  network: {
    titleKey: 'errors:ai.networkError',
    messageKey: 'errors:ai.networkErrorSuggestion',
    severity: 'warning',
  },
  auth: {
    titleKey: 'errors:ai.authError',
    messageKey: 'errors:ai.authErrorSuggestion',
    severity: 'error',
  },
  rate_limit: {
    titleKey: 'errors:ai.rateLimit',
    messageKey: 'errors:ai.rateLimitSuggestion',
    severity: 'warning',
  },
  context_overflow: {
    titleKey: 'errors:ai.contextOverflow',
    messageKey: 'errors:ai.contextOverflowSuggestion',
    severity: 'warning',
  },
  loop_detected: {
    titleKey: 'errors:ai.loopDetected',
    messageKey: 'errors:ai.loopDetectedSuggestion',
    severity: 'warning',
  },
  timeout: {
    titleKey: 'errors:ai.timeoutError',
    messageKey: 'errors:ai.timeoutSuggestion',
    severity: 'warning',
  },
  provider_quota: {
    titleKey: 'errors:ai.providerQuota.title',
    messageKey: 'errors:ai.providerQuota.message',
    severity: 'error',
  },
  provider_billing: {
    titleKey: 'errors:ai.providerBilling.title',
    messageKey: 'errors:ai.providerBilling.message',
    severity: 'error',
  },
  provider_unavailable: {
    titleKey: 'errors:ai.providerUnavailable.title',
    messageKey: 'errors:ai.providerUnavailable.message',
    severity: 'warning',
  },
  permission: {
    titleKey: 'errors:ai.permission.title',
    messageKey: 'errors:ai.permission.message',
    severity: 'error',
  },
  invalid_request: {
    titleKey: 'errors:ai.invalidRequest.title',
    messageKey: 'errors:ai.invalidRequest.message',
    severity: 'error',
  },
  content_policy: {
    titleKey: 'errors:ai.contentPolicy.title',
    messageKey: 'errors:ai.contentPolicy.message',
    severity: 'warning',
  },
  model_error: {
    titleKey: 'errors:ai.executionFailed',
    messageKey: 'errors:ai.genericSuggestion',
    severity: 'error',
  },
  unknown: {
    titleKey: 'errors:ai.executionFailed',
    messageKey: 'errors:ai.genericSuggestion',
    severity: 'error',
  },
};

const DEFAULT_ACTIONS: Record<AiErrorCategory, AiErrorActionCode[]> = {
  network: ['retry', 'switch_model', 'copy_diagnostics'],
  auth: ['open_model_settings', 'copy_diagnostics'],
  rate_limit: ['wait_and_retry', 'switch_model', 'copy_diagnostics'],
  context_overflow: ['compress_context', 'start_new_chat'],
  loop_detected: ['copy_diagnostics'],
  timeout: ['retry', 'switch_model', 'copy_diagnostics'],
  provider_quota: ['open_model_settings', 'switch_model', 'copy_diagnostics'],
  provider_billing: ['open_model_settings', 'switch_model', 'copy_diagnostics'],
  provider_unavailable: ['wait_and_retry', 'switch_model', 'copy_diagnostics'],
  permission: ['open_model_settings', 'copy_diagnostics'],
  invalid_request: ['copy_diagnostics'],
  content_policy: ['copy_diagnostics'],
  model_error: ['retry', 'switch_model', 'copy_diagnostics'],
  unknown: ['retry', 'switch_model', 'copy_diagnostics'],
};

const RETRYABLE_CATEGORIES = new Set<AiErrorCategory>([
  'network',
  'rate_limit',
  'timeout',
  'provider_unavailable',
]);

export function normalizeAiErrorDetail(input: AiErrorDetail | null | undefined, fallbackMessage = ''): AiErrorDetail {
  const raw = input?.rawMessage ?? fallbackMessage;
  const category = normalizeCategory(input?.category, input, raw);

  return {
    ...input,
    category,
    providerCode: normalizeString(input?.providerCode),
    providerMessage: normalizeString(input?.providerMessage),
    requestId: normalizeString(input?.requestId),
    rawMessage: raw,
    retryable: input?.retryable ?? RETRYABLE_CATEGORIES.has(category),
    actionHints: normalizeActionHints(input?.actionHints),
  };
}

export function getAiErrorPresentation(detail: AiErrorDetail): AiErrorPresentation {
  const normalized = normalizeAiErrorDetail(detail);
  const category = normalized.category ?? 'unknown';
  const config = PRESENTATION_KEYS[category] ?? PRESENTATION_KEYS.unknown;
  const actionCodes = normalized.actionHints?.length ? normalized.actionHints : DEFAULT_ACTIONS[category];

  return {
    category,
    titleKey: config.titleKey,
    messageKey: config.messageKey,
    severity: config.severity,
    retryable: normalized.retryable ?? RETRYABLE_CATEGORIES.has(category),
    actions: actionCodes.map((code) => ({ code, labelKey: ACTION_LABEL_KEYS[code] })),
    diagnostics: buildDiagnostics(normalized),
  };
}

function normalizeCategory(
  category: AiErrorCategory | undefined,
  detail: AiErrorDetail | null | undefined,
  rawMessage: string,
): AiErrorCategory {
  if (category && category !== 'unknown') {
    return category;
  }

  const status = detail?.httpStatus;
  const code = `${detail?.providerCode ?? ''}`.toLowerCase();
  const message = `${detail?.providerMessage ?? ''} ${rawMessage}`.toLowerCase();

  if (
    status === 402 ||
    includesAny(`${code} ${message}`, [
      '1113',
      'insufficient_quota',
      'insufficient quota',
      'insufficient balance',
      'not_enough_balance',
      'not enough balance',
      'exceeded_current_quota',
      'exceeded current quota',
      '余额不足',
      '无可用资源包',
      '账户已欠费',
    ])
  ) {
    return 'provider_quota';
  }

  if (includesAny(`${code} ${message}`, ['billing', 'membership expired', 'subscription expired', '套餐已到期', '1309'])) {
    return 'provider_billing';
  }

  if (status === 529 || status === 503 || includesAny(`${code} ${message}`, ['overloaded_error', 'overloaded', 'service unavailable', 'temporarily unavailable', '1305'])) {
    return 'provider_unavailable';
  }

  if (status === 401 || includesAny(`${code} ${message}`, ['authentication', 'invalid api key', 'incorrect api key', 'unauthorized'])) {
    return 'auth';
  }

  if (status === 403 || includesAny(`${code} ${message}`, ['permission_error', 'permission denied', 'forbidden', 'not authorized'])) {
    return 'permission';
  }

  if (status === 429 || includesAny(`${code} ${message}`, ['rate limit', 'too many requests', '1302'])) {
    return 'rate_limit';
  }

  if (includesAny(message, ['context window', 'context length', 'token limit', 'max_tokens'])) {
    return 'context_overflow';
  }

  if (includesAny(`${code} ${message}`, ['content policy', 'content_filter', 'safety', 'sensitive', '1301'])) {
    return 'content_policy';
  }

  if (includesAny(message, ['stream closed', 'sse error', 'connection reset', 'broken pipe'])) {
    return 'network';
  }

  if (includesAny(message, ['timeout', 'timed out'])) {
    return 'timeout';
  }

  if (status === 400 || status === 413 || status === 422 || includesAny(`${code} ${message}`, ['invalid_request_error', 'invalid request', 'bad request', 'model_not_found', 'model not found'])) {
    return 'invalid_request';
  }

  return 'unknown';
}

function normalizeActionHints(actions: AiErrorActionCode[] | undefined): AiErrorActionCode[] | undefined {
  const valid = new Set(Object.keys(ACTION_LABEL_KEYS));
  const normalized = (actions ?? []).filter((action): action is AiErrorActionCode => valid.has(action));
  return normalized.length ? normalized : undefined;
}

function normalizeString(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed || undefined;
}

function includesAny(value: string, needles: string[]): boolean {
  return needles.some((needle) => value.includes(needle));
}

function buildDiagnostics(detail: AiErrorDetail): string {
  const parts = [
    `category=${detail.category ?? 'unknown'}`,
    detail.provider ? `provider=${detail.provider}` : null,
    detail.providerCode ? `code=${detail.providerCode}` : null,
    detail.httpStatus ? `http_status=${detail.httpStatus}` : null,
    detail.requestId ? `request_id=${detail.requestId}` : null,
  ].filter(Boolean);

  return parts.join(', ');
}
