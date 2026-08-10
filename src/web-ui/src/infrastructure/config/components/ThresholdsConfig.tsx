import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RotateCcw } from 'lucide-react';
import {
  Button,
  ConfigPageLoading,
  NumberInput,
} from '@/component-library';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { configManager } from '../services/ConfigManager';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageRow,
  ConfigPageSection,
} from './common';

const log = createLogger('ThresholdsConfig');

/**
 * ai.thresholds.* — 阈值参数配置化统一入口。
 *
 * 每个子域对应一个分组；默认值 = 后端 legacy 硬编码（未配置时行为不变）。
 * 写入路径：configManager.setConfig('ai.thresholds.<domain>.<field>', value)。
 */

interface ThresholdsShape {
  subagent: { max_hard_cap: number; timeout_grace_secs: number; session_references_per_turn: number };
  compression: {
    safety_reserve_tokens: number;
    overflow_attempts: number;
    main_context_overflow_recoveries: number;
    consecutive_failures: number;
    failed_tool_recovery_attempts: number;
    stop_hook_continuations: number;
    same_round_passes: number;
    recent_context_tokens: number;
    retry_step_tokens: number;
    max_retained_user_tokens: number;
    image_bearing_messages: number;
  };
  model_retry: {
    max_attempts: number;
    base_delay_ms: number;
    rate_limit_base_delay_ms: number;
    max_exponential_delay_ms: number;
    max_rate_limit_delay_ms: number;
    max_exponent_shift: number;
  };
  tool_output_cap: {
    default_chars: number;
    per_round_chars: number;
    preview_chars: number;
    read_chars: number;
    shell_chars: number;
  };
  tool_timeout: {
    bash_default_ms: number;
    bash_max_ms: number;
    exec_command_yield_ms: number;
    remote_shell_probe_ms: number;
    document_conversion_secs: number;
    web_fetch_secs: number;
    exa_secs: number;
    agent_wait_default_ms: number;
    agent_wait_max_ms: number;
    mcp_render_chars: number;
    diff_page_chars: number;
    diff_total_chars: number;
    diff_new_file_bytes: number;
  };
  knowledge_search: {
    max_scan_file_bytes: number;
    max_scan_depth: number;
    default_max_results: number;
    max_results_cap: number;
  };
  acp_timeout: {
    client_startup_secs: number;
    permission_secs: number;
    session_close_secs: number;
    cli_detect_secs: number;
    handshake_secs: number;
    try_connect_total_secs: number;
    requirement_probe_secs: number;
    adapter_download_secs: number;
    cli_install_secs: number;
    direct_secs: number;
    task_secs: number;
  };
  warden: { max_defer_count: number; max_rate: number; judgement_timeout_secs: number };
  deep_review: {
    diff_max_chars_per_turn: number;
    diff_max_acquisitions_per_turn: number;
    max_parallel_instances: number;
    max_queue_wait_secs: number;
    auto_retry_elapsed_guard_secs: number;
  };
  memories: {
    summary_token_limit: number;
    message_content_token_limit: number;
    tool_input_token_limit: number;
    tool_result_token_limit: number;
    tool_error_token_limit: number;
    rollout_token_limit: number;
  };
  output_tokens: { automatic_tiers: number[]; ratio_percent: number };
  goal: { idle_wakeup_delay_ms: number; max_auto_continuations: number };
}

const DEFAULT_THRESHOLDS: ThresholdsShape = {
  subagent: { max_hard_cap: 64, timeout_grace_secs: 10, session_references_per_turn: 5 },
  compression: {
    safety_reserve_tokens: 10_000,
    overflow_attempts: 4,
    main_context_overflow_recoveries: 2,
    consecutive_failures: 3,
    failed_tool_recovery_attempts: 3,
    stop_hook_continuations: 3,
    same_round_passes: 2,
    recent_context_tokens: 10_000,
    retry_step_tokens: 10_000,
    max_retained_user_tokens: 20_000,
    image_bearing_messages: 2,
  },
  model_retry: {
    max_attempts: 10,
    base_delay_ms: 500,
    rate_limit_base_delay_ms: 2_000,
    max_exponential_delay_ms: 30_000,
    max_rate_limit_delay_ms: 60_000,
    max_exponent_shift: 6,
  },
  tool_output_cap: {
    default_chars: 50_000,
    per_round_chars: 200_000,
    preview_chars: 2_000,
    read_chars: 72_000,
    shell_chars: 30_000,
  },
  tool_timeout: {
    bash_default_ms: 120_000,
    bash_max_ms: 600_000,
    exec_command_yield_ms: 30_000,
    remote_shell_probe_ms: 3_000,
    document_conversion_secs: 30,
    web_fetch_secs: 30,
    exa_secs: 25,
    agent_wait_default_ms: 600_000,
    agent_wait_max_ms: 3_600_000,
    mcp_render_chars: 32_000,
    diff_page_chars: 40_000,
    diff_total_chars: 80_000,
    diff_new_file_bytes: 16_384,
  },
  knowledge_search: {
    max_scan_file_bytes: 2_097_152,
    max_scan_depth: 16,
    default_max_results: 50,
    max_results_cap: 200,
  },
  acp_timeout: {
    client_startup_secs: 60,
    permission_secs: 600,
    session_close_secs: 5,
    cli_detect_secs: 5,
    handshake_secs: 30,
    try_connect_total_secs: 35,
    requirement_probe_secs: 3,
    adapter_download_secs: 120,
    cli_install_secs: 600,
    direct_secs: 1800,
    task_secs: 600,
  },
  warden: { max_defer_count: 3, max_rate: 1000, judgement_timeout_secs: 8 },
  deep_review: {
    diff_max_chars_per_turn: 240_000,
    diff_max_acquisitions_per_turn: 128,
    max_parallel_instances: 4,
    max_queue_wait_secs: 1200,
    auto_retry_elapsed_guard_secs: 180,
  },
  memories: {
    summary_token_limit: 2_500,
    message_content_token_limit: 8_000,
    tool_input_token_limit: 6_000,
    tool_result_token_limit: 12_000,
    tool_error_token_limit: 1_000,
    rollout_token_limit: 120_000,
  },
  output_tokens: { automatic_tiers: [8_000, 16_000, 24_000, 32_000, 64_000], ratio_percent: 40 },
  goal: { idle_wakeup_delay_ms: 600_000, max_auto_continuations: 10 },
};

function deepMerge(base: ThresholdsShape, patch: Partial<ThresholdsShape> | null | undefined): ThresholdsShape {
  if (!patch) return base;
  const merged: ThresholdsShape = { ...base };
  (Object.keys(base) as (keyof ThresholdsShape)[]).forEach((domain) => {
    const patchDomain = patch[domain];
    if (patchDomain && typeof patchDomain === 'object') {
      merged[domain] = { ...(base[domain] as object), ...(patchDomain as object) } as never;
    }
  });
  return merged;
}

function normalizeThresholds(raw: Partial<ThresholdsShape> | null | undefined): ThresholdsShape {
  return deepMerge(DEFAULT_THRESHOLDS, raw);
}

type DomainKey = keyof ThresholdsShape;
type DomainField<D extends DomainKey> = keyof ThresholdsShape[D];

export default function ThresholdsConfig() {
  const { t } = useTranslation('settings/thresholds');
  const { success: notifySuccess, error: notifyError } = useNotification();
  const [config, setConfig] = useState<ThresholdsShape>(DEFAULT_THRESHOLDS);
  const [loading, setLoading] = useState(true);
  const [savingKey, setSavingKey] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const raw = await configManager.getConfig<Partial<ThresholdsShape>>('ai.thresholds');
        if (!cancelled) setConfig(normalizeThresholds(raw));
      } catch (error) {
        log.warn('Failed to load thresholds config, using defaults', error);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, []);

  const updateField = useCallback(async <D extends DomainKey>(
    domain: D,
    field: DomainField<D>,
    value: number,
  ) => {
    if (Number.isNaN(value) || value < 0) return;
    const key = `ai.thresholds.${domain}.${String(field)}`;
    const previous = config;
    setConfig((prev) => ({
      ...prev,
      [domain]: { ...(prev[domain] as object), [field]: value } as never,
    }));
    setSavingKey(key);
    try {
      await configManager.setConfig(key, value);
      notifySuccess(t('messages.saved'));
    } catch (error) {
      log.error('Failed to save thresholds config', { key, error });
      setConfig(previous);
      notifyError(error instanceof Error ? error.message : t('messages.saveFailed'));
    } finally {
      setSavingKey(null);
    }
  }, [config, notifySuccess, notifyError, t]);

  const handleReset = useCallback(async () => {
    setSavingKey('reset');
    try {
      await configManager.resetConfig('ai.thresholds');
      setConfig(DEFAULT_THRESHOLDS);
      notifySuccess(t('messages.settingsReset'));
    } catch (error) {
      log.error('Failed to reset thresholds config', error);
      notifyError(error instanceof Error ? error.message : t('messages.settingsResetFailed'));
    } finally {
      setSavingKey(null);
    }
  }, [notifySuccess, notifyError, t]);

  const renderField = useCallback(<D extends DomainKey>(
    domain: D,
    field: DomainField<D>,
    min = 1,
    step = 1,
  ) => {
    const value = (config[domain] as Record<string, unknown>)[field as string] as number;
    const labelKey = `fields.${domain}.${String(field)}`;
    return (
      <ConfigPageRow key={`${domain}.${String(field)}`} label={t(labelKey)}>
        <NumberInput
          value={value}
          min={min}
          step={step}
          disabled={savingKey === `ai.thresholds.${domain}.${String(field)}`}
          onChange={(next) => void updateField(domain, field, Number(next))}
        />
      </ConfigPageRow>
    );
  }, [config, savingKey, updateField, t]);

  const domainSections = useMemo(() => {
    const sections: { title: string; rows: React.ReactNode[] }[] = [];
    const add = (domain: DomainKey, rows: React.ReactNode[]) => {
      sections.push({ title: t(`fields.${domain}.__title`), rows });
    };

    add('subagent', [
      renderField('subagent', 'max_hard_cap', 1),
      renderField('subagent', 'timeout_grace_secs', 1),
      renderField('subagent', 'session_references_per_turn', 1),
    ]);
    add('compression', [
      renderField('compression', 'safety_reserve_tokens', 1, 100),
      renderField('compression', 'overflow_attempts', 1),
      renderField('compression', 'main_context_overflow_recoveries', 0),
      renderField('compression', 'consecutive_failures', 1),
      renderField('compression', 'failed_tool_recovery_attempts', 0),
      renderField('compression', 'stop_hook_continuations', 0),
      renderField('compression', 'same_round_passes', 1),
      renderField('compression', 'recent_context_tokens', 1, 100),
      renderField('compression', 'retry_step_tokens', 1, 100),
      renderField('compression', 'max_retained_user_tokens', 1, 100),
      renderField('compression', 'image_bearing_messages', 1),
    ]);
    add('model_retry', [
      renderField('model_retry', 'max_attempts', 1),
      renderField('model_retry', 'base_delay_ms', 1, 10),
      renderField('model_retry', 'rate_limit_base_delay_ms', 1, 10),
      renderField('model_retry', 'max_exponential_delay_ms', 1, 100),
      renderField('model_retry', 'max_rate_limit_delay_ms', 1, 100),
      renderField('model_retry', 'max_exponent_shift', 0),
    ]);
    add('tool_output_cap', [
      renderField('tool_output_cap', 'default_chars', 1, 100),
      renderField('tool_output_cap', 'per_round_chars', 1, 100),
      renderField('tool_output_cap', 'preview_chars', 1, 10),
      renderField('tool_output_cap', 'read_chars', 1, 100),
      renderField('tool_output_cap', 'shell_chars', 1, 100),
    ]);
    add('tool_timeout', [
      renderField('tool_timeout', 'bash_default_ms', 1, 1000),
      renderField('tool_timeout', 'bash_max_ms', 1, 1000),
      renderField('tool_timeout', 'exec_command_yield_ms', 1, 100),
      renderField('tool_timeout', 'remote_shell_probe_ms', 1, 10),
      renderField('tool_timeout', 'document_conversion_secs', 1),
      renderField('tool_timeout', 'web_fetch_secs', 1),
      renderField('tool_timeout', 'exa_secs', 1),
      renderField('tool_timeout', 'agent_wait_default_ms', 1, 1000),
      renderField('tool_timeout', 'agent_wait_max_ms', 1, 1000),
      renderField('tool_timeout', 'mcp_render_chars', 1, 100),
      renderField('tool_timeout', 'diff_page_chars', 1, 100),
      renderField('tool_timeout', 'diff_total_chars', 1, 100),
      renderField('tool_timeout', 'diff_new_file_bytes', 1, 100),
    ]);
    add('knowledge_search', [
      renderField('knowledge_search', 'max_scan_file_bytes', 1, 1024),
      renderField('knowledge_search', 'max_scan_depth', 1),
      renderField('knowledge_search', 'default_max_results', 1),
      renderField('knowledge_search', 'max_results_cap', 1),
    ]);
    add('acp_timeout', [
      renderField('acp_timeout', 'client_startup_secs', 1),
      renderField('acp_timeout', 'permission_secs', 1),
      renderField('acp_timeout', 'session_close_secs', 1),
      renderField('acp_timeout', 'cli_detect_secs', 1),
      renderField('acp_timeout', 'handshake_secs', 1),
      renderField('acp_timeout', 'try_connect_total_secs', 1),
      renderField('acp_timeout', 'requirement_probe_secs', 1),
      renderField('acp_timeout', 'adapter_download_secs', 1),
      renderField('acp_timeout', 'cli_install_secs', 1),
      renderField('acp_timeout', 'direct_secs', 1),
      renderField('acp_timeout', 'task_secs', 1),
    ]);
    add('deep_review', [
      renderField('deep_review', 'diff_max_chars_per_turn', 1, 100),
      renderField('deep_review', 'diff_max_acquisitions_per_turn', 1),
      renderField('deep_review', 'max_parallel_instances', 1),
      renderField('deep_review', 'max_queue_wait_secs', 1),
      renderField('deep_review', 'auto_retry_elapsed_guard_secs', 1),
    ]);
    add('memories', [
      renderField('memories', 'summary_token_limit', 1, 10),
      renderField('memories', 'message_content_token_limit', 1, 10),
      renderField('memories', 'tool_input_token_limit', 1, 10),
      renderField('memories', 'tool_result_token_limit', 1, 10),
      renderField('memories', 'tool_error_token_limit', 1, 10),
      renderField('memories', 'rollout_token_limit', 1, 100),
    ]);
    add('output_tokens', [
      renderField('output_tokens', 'ratio_percent', 1),
    ]);
    add('goal', [
      renderField('goal', 'idle_wakeup_delay_ms', 1, 1000),
      renderField('goal', 'max_auto_continuations', 1),
    ]);

    return sections;
  }, [renderField, t]);

  if (loading) {
    return <ConfigPageLoading text={t('messages.loading')} />;
  }

  return (
    <ConfigPageLayout>
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
        extra={
          <Button
            type="button"
            variant="secondary"
            size="small"
            disabled={savingKey === 'reset'}
            onClick={() => void handleReset()}
          >
            <RotateCcw size={14} />
            {t('actions.resetToDefaults')}
          </Button>
        }
      />
      <ConfigPageContent>
        {domainSections.map((section) => (
          <ConfigPageSection key={section.title} title={section.title}>
            {section.rows}
          </ConfigPageSection>
        ))}
      </ConfigPageContent>
    </ConfigPageLayout>
  );
}
