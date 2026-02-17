import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { InstallOptions } from '../types/installer';

interface ModelSetupProps {
  options: InstallOptions;
  setOptions: React.Dispatch<React.SetStateAction<InstallOptions>>;
  onSkip: () => void;
  onNext: () => Promise<void>;
}

const PROVIDERS = [
  { id: 'deepseek', label: 'DeepSeek', baseUrl: 'https://api.deepseek.com/v1', format: 'openai' as const },
  { id: 'qwen', label: 'Qwen', baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1', format: 'openai' as const },
  { id: 'zhipu', label: 'Zhipu', baseUrl: 'https://open.bigmodel.cn/api/paas/v4', format: 'openai' as const },
  { id: 'anthropic', label: 'Anthropic', baseUrl: 'https://api.anthropic.com/v1', format: 'anthropic' as const },
];

export function ModelSetup({ options, setOptions, onSkip, onNext }: ModelSetupProps) {
  const { t } = useTranslation();

  const current = options.modelConfig;
  const selectedProvider = useMemo(
    () => PROVIDERS.find((p) => p.id === current?.provider) ?? null,
    [current?.provider],
  );

  const setProvider = (providerId: string) => {
    const provider = PROVIDERS.find((p) => p.id === providerId);
    if (!provider) {
      setOptions((prev) => ({ ...prev, modelConfig: null }));
      return;
    }
    setOptions((prev) => ({
      ...prev,
      modelConfig: {
        provider: provider.id,
        apiKey: prev.modelConfig?.apiKey ?? '',
        baseUrl: provider.baseUrl,
        modelName: prev.modelConfig?.modelName ?? '',
        format: provider.format,
      },
    }));
  };

  const updateField = (field: 'apiKey' | 'modelName', value: string) => {
    setOptions((prev) => {
      if (!prev.modelConfig) return prev;
      return { ...prev, modelConfig: { ...prev.modelConfig, [field]: value } };
    });
  };

  const canContinue = Boolean(
    current && current.provider && current.apiKey.trim() && current.modelName.trim() && current.baseUrl.trim(),
  );

  return (
    <div style={{
      flex: 1,
      display: 'flex',
      flexDirection: 'column',
      justifyContent: 'center',
      padding: '20px 42px 24px',
      maxWidth: 560,
      margin: '0 auto',
      width: '100%',
      animation: 'fadeIn 0.4s ease-out',
    }}>
      <div style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 6,
        padding: '4px 10px',
        borderRadius: 999,
        background: 'transparent',
        color: 'rgba(110, 231, 163, 0.95)',
        fontSize: 14,
        fontWeight: 600,
        marginBottom: 10,
        width: 'fit-content',
        marginInline: 'auto',
        justifyContent: 'center',
      }}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M20 6L9 17L4 12" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        {t('model.installDone')}
      </div>
      <div style={{ marginBottom: 8, fontSize: 12, color: 'var(--color-text-muted)' }}>
        {t('model.subtitle')}
      </div>

      <div className="section-label">{t('model.provider')}</div>
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginBottom: 16 }}>
        {PROVIDERS.map((provider) => {
          const active = current?.provider === provider.id;
          return (
            <button
              key={provider.id}
              type="button"
              onClick={() => setProvider(provider.id)}
              style={{
                border: 'none',
                borderRadius: 8,
                padding: '8px 10px',
                background: active ? 'rgba(96, 165, 250, 0.14)' : 'rgba(148, 163, 184, 0.08)',
                color: active ? 'var(--color-text-primary)' : 'var(--color-text-secondary)',
                fontSize: 12,
                cursor: 'pointer',
              }}
            >
              {provider.label}
            </button>
          );
        })}
      </div>

      <div className="section-label">{t('model.config')}</div>
      <div style={{ display: 'grid', gap: 10, marginBottom: 4 }}>
        <input
          className="input"
          placeholder={t('model.modelName')}
          value={current?.modelName ?? ''}
          disabled={!selectedProvider}
          onChange={(e) => updateField('modelName', e.target.value)}
        />
        <input
          className="input"
          placeholder={t('model.apiKey')}
          type="password"
          value={current?.apiKey ?? ''}
          disabled={!selectedProvider}
          onChange={(e) => updateField('apiKey', e.target.value)}
        />
        <input className="input" value={current?.baseUrl ?? ''} disabled />
      </div>

      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 10, alignItems: 'center', paddingTop: 20, marginTop: 'auto' }}>
        <button className="btn btn-ghost" onClick={onSkip}>
          {t('model.skip')}
        </button>
        <button className="btn btn-primary" onClick={onNext} disabled={!canContinue}>
          {t('model.nextTheme')}
        </button>
      </div>
    </div>
  );
}

