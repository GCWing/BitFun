import { Button, Icon, IconButton, Input, Select, type SelectOption, Tooltip } from '@bitfun/ui';
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Save } from 'lucide-react';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { useI18n } from '@/infrastructure/i18n';
import { useNotification } from '@/shared/notification-system';
import { copyTextToClipboard } from '@/shared/utils/textSelection';
import { ConfigLoadingState, ConfigMessage } from './common';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageRow,
  ConfigPageSection,
} from './common';
import { createLogger } from '@/shared/utils/logger';
import './WebSearchSettingsPage.scss';

const log = createLogger('WebSearchSettings');

type ProviderId = 'exa_mcp_free' | 'exa_search_api' | 'tavily' | 'bitfun_search_http';
type HttpAuthMode = 'none' | 'bearer' | 'header';

interface CredentialProviderConfig extends Record<string, unknown> {
  credentialId: string;
}

interface BitFunHttpAuthConfig extends Record<string, unknown> {
  mode: string;
  credentialId: string;
  headerName: string;
}

interface BitFunHttpConfig extends Record<string, unknown> {
  endpoint: string;
  auth: BitFunHttpAuthConfig;
}

interface WebSearchConfig extends Record<string, unknown> {
  provider: string;
  providers: {
    exa_search_api: CredentialProviderConfig;
    tavily: CredentialProviderConfig;
    bitfun_search_http: BitFunHttpConfig;
    [key: string]: unknown;
  };
}

const DEFAULT_CONFIG: WebSearchConfig = {
  provider: 'exa_mcp_free',
  providers: {
    exa_search_api: { credentialId: 'exa-search-api' },
    tavily: { credentialId: 'tavily-search-api' },
    bitfun_search_http: {
      endpoint: '',
      auth: {
        mode: 'none',
        credentialId: 'bitfun-search-http',
        headerName: '',
      },
    },
  },
};

const BITFUN_PROTOCOL_REQUEST_EXAMPLE = `POST <configured endpoint>
Content-Type: application/json
Accept: application/vnd.bitfun.web-search.v1+json

{
  "query": "Rust async runtime",
  "maxResults": 10
}`;

const BITFUN_PROTOCOL_SUCCESS_EXAMPLE = `{
  "results": [
    {
      "title": "Rust Async Book",
      "url": "https://example.com/rust-async",
      "publishedAt": "2026-08-30T00:00:00Z",
      "author": "Example Author"
    }
  ]
}`;

const BITFUN_PROTOCOL_ERROR_EXAMPLE = `{
  "error": {
    "code": "rate_limited",
    "message": "try again later",
    "retryAfterSeconds": 30
  }
}`;

const BITFUN_PROTOCOL_ERROR_CODES = [
  'invalid_request',
  'authentication_failed',
  'permission_denied',
  'quota_exhausted',
  'rate_limited',
  'provider_unavailable',
  'invalid_response',
].join(', ');

function normalizeSelectValue(value: string | number | (string | number)[]): string {
  const selected = Array.isArray(value) ? value[0] : value;
  return selected == null ? '' : String(selected);
}

function normalizeConfig(value: unknown): WebSearchConfig {
  const raw = value && typeof value === 'object' ? value as Partial<WebSearchConfig> : {};
  const rawProviders: Partial<WebSearchConfig['providers']> & Record<string, unknown> =
    raw.providers && typeof raw.providers === 'object'
      ? raw.providers
      : {};
  const rawExa = rawProviders.exa_search_api && typeof rawProviders.exa_search_api === 'object'
    ? rawProviders.exa_search_api as Partial<CredentialProviderConfig>
    : {};
  const rawTavily = rawProviders.tavily && typeof rawProviders.tavily === 'object'
    ? rawProviders.tavily as Partial<CredentialProviderConfig>
    : {};
  const rawHttp = rawProviders.bitfun_search_http && typeof rawProviders.bitfun_search_http === 'object'
    ? rawProviders.bitfun_search_http as Partial<BitFunHttpConfig>
    : {};
  const rawAuth = rawHttp.auth && typeof rawHttp.auth === 'object'
    ? rawHttp.auth as Partial<BitFunHttpAuthConfig>
    : {};
  return {
    ...DEFAULT_CONFIG,
    ...raw,
    provider: typeof raw.provider === 'string' ? raw.provider : DEFAULT_CONFIG.provider,
    providers: {
      ...DEFAULT_CONFIG.providers,
      ...rawProviders,
      exa_search_api: {
        ...DEFAULT_CONFIG.providers.exa_search_api,
        ...rawExa,
      },
      tavily: {
        ...DEFAULT_CONFIG.providers.tavily,
        ...rawTavily,
      },
      bitfun_search_http: {
        ...DEFAULT_CONFIG.providers.bitfun_search_http,
        ...rawHttp,
        auth: {
          ...DEFAULT_CONFIG.providers.bitfun_search_http.auth,
          ...rawAuth,
        },
      },
    },
  };
}

const WebSearchSettingsPage: React.FC = () => {
  const { t } = useI18n('settings/web-search');
  const { success: notifySuccess, error: notifyError } = useNotification();
  const protocolRef = useRef<HTMLDivElement>(null);
  const [config, setConfig] = useState<WebSearchConfig>(DEFAULT_CONFIG);
  const [savedConfig, setSavedConfig] = useState<WebSearchConfig>(DEFAULT_CONFIG);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [credentialBusy, setCredentialBusy] = useState(false);
  const [credential, setCredential] = useState('');
  const [credentialConfigured, setCredentialConfigured] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);

  const providerOptions = useMemo<SelectOption[]>(() => [
    { value: 'exa_mcp_free', label: t('providers.exaMcpFree') },
    { value: 'exa_search_api', label: t('providers.exaSearchApi') },
    { value: 'tavily', label: t('providers.tavily') },
    { value: 'bitfun_search_http', label: t('providers.bitfunSearchHttp') },
  ], [t]);
  const authOptions = useMemo<SelectOption[]>(() => [
    { value: 'none', label: t('auth.none') },
    { value: 'bearer', label: t('auth.bearer') },
    { value: 'header', label: t('auth.header') },
  ], [t]);

  const selectedProvider = config.provider as ProviderId;
  const httpConfig = config.providers.bitfun_search_http;
  const hasUnsavedChanges = useMemo(
    () => JSON.stringify(config) !== JSON.stringify(savedConfig),
    [config, savedConfig],
  );
  const credentialRequired = selectedProvider === 'exa_search_api'
    || selectedProvider === 'tavily'
    || (selectedProvider === 'bitfun_search_http' && !['', 'none'].includes(httpConfig.auth.mode));

  const refreshCredentialStatus = useCallback(async (provider: string, required: boolean) => {
    if (!required) {
      setCredentialConfigured(false);
      return;
    }
    try {
      const status = await configAPI.getWebSearchCredentialStatus(provider);
      setCredentialConfigured(status.configured);
    } catch (error) {
      log.error('Failed to load WebSearch credential status', { provider, error });
      setCredentialConfigured(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const loaded = normalizeConfig(await configAPI.getConfig('ai.web_search'));
        if (!cancelled) {
          setConfig(loaded);
          setSavedConfig(loaded);
          const provider = loaded.provider as ProviderId;
          const requiresCredential = provider === 'exa_search_api'
            || provider === 'tavily'
            || (provider === 'bitfun_search_http'
              && !['', 'none'].includes(loaded.providers.bitfun_search_http.auth.mode));
          await refreshCredentialStatus(provider, requiresCredential);
        }
      } catch (error) {
        log.error('Failed to load WebSearch settings', error);
        if (!cancelled) {
          setMessage({ type: 'error', text: t('messages.loadFailed') });
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [refreshCredentialStatus, t]);

  useEffect(() => {
    void refreshCredentialStatus(selectedProvider, credentialRequired);
    setCredential('');
  }, [credentialRequired, refreshCredentialStatus, selectedProvider]);

  const saveConfiguration = useCallback(async (showSuccess: boolean) => {
    await configAPI.setConfig('ai.web_search', config);
    setSavedConfig(config);
    if (showSuccess) {
      setMessage(null);
      notifySuccess(t('messages.saved'));
    }
  }, [config, notifySuccess, t]);

  const handleSaveConfiguration = useCallback(async () => {
    setSaving(true);
    try {
      await saveConfiguration(true);
      await refreshCredentialStatus(selectedProvider, credentialRequired);
    } catch (error) {
      log.error('Failed to save WebSearch settings', error);
      setMessage({ type: 'error', text: t('messages.saveFailed') });
    } finally {
      setSaving(false);
    }
  }, [credentialRequired, refreshCredentialStatus, saveConfiguration, selectedProvider, t]);

  const handleSaveCredential = useCallback(async () => {
    if (!credential.trim()) {
      setMessage({ type: 'error', text: t('messages.credentialRequired') });
      return;
    }
    setCredentialBusy(true);
    try {
      await saveConfiguration(false);
      const status = await configAPI.saveWebSearchCredential(selectedProvider, credential);
      setCredential('');
      setCredentialConfigured(status.configured);
      setMessage(null);
      notifySuccess(t('messages.credentialSaved'));
    } catch (error) {
      log.error('Failed to save WebSearch credential', { provider: selectedProvider, error });
      setMessage({ type: 'error', text: t('messages.credentialSaveFailed') });
    } finally {
      setCredentialBusy(false);
    }
  }, [credential, notifySuccess, saveConfiguration, selectedProvider, t]);

  const handleClearCredential = useCallback(async () => {
    setCredentialBusy(true);
    try {
      const status = await configAPI.clearWebSearchCredential(selectedProvider);
      setCredentialConfigured(status.configured);
      setCredential('');
      setMessage(null);
      notifySuccess(t('messages.credentialCleared'));
    } catch (error) {
      log.error('Failed to clear WebSearch credential', { provider: selectedProvider, error });
      setMessage({ type: 'error', text: t('messages.credentialClearFailed') });
    } finally {
      setCredentialBusy(false);
    }
  }, [notifySuccess, selectedProvider, t]);

  const handleCopyProtocol = useCallback(async () => {
    const protocolContent = protocolRef.current?.innerText.trim();
    if (!protocolContent) {
      notifyError(t('messages.protocolCopyFailed'));
      return;
    }
    const copied = await copyTextToClipboard([
      t('sections.protocol.title'),
      t('sections.protocol.description'),
      protocolContent,
    ].join('\n\n'));
    if (copied) {
      notifySuccess(t('messages.protocolCopied'));
    } else {
      notifyError(t('messages.protocolCopyFailed'));
    }
  }, [notifyError, notifySuccess, t]);

  if (loading) {
    return <ConfigLoadingState label={t('messages.loading')} />;
  }

  return (
    <ConfigPageLayout data-bf-component="config" data-bf-part="root">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
        extra={(
          <Tooltip content={t('actions.saveConfiguration')} placement="bottom">
            <IconButton
              type="button"
              size="sm"
              variant={hasUnsavedChanges ? 'primary' : 'quiet'}
              loading={saving}
              aria-label={t('actions.saveConfiguration')}
              icon={<Save />}
              onClick={() => void handleSaveConfiguration()}
            />
          </Tooltip>
        )}
      />
      <ConfigPageContent>
        <ConfigMessage message={message} />
        <ConfigPageSection
          title={t('sections.provider.title')}
          extra={(
            <Select
              className="web-search-settings__provider-select"
              value={config.provider}
              options={providerOptions}
              size="sm"
              onValueChange={(value) => setConfig(previous => ({
                ...previous,
                provider: normalizeSelectValue(value),
              }))}
            />
          )}
        >
          {null}
        </ConfigPageSection>

        {selectedProvider === 'bitfun_search_http' ? (
          <ConfigPageSection
            title={t('sections.http.title')}
            description={t('sections.http.description')}
          >
            <ConfigPageRow label={t('fields.endpoint.label')} description={t('fields.endpoint.description')} balanced>
              <Input
                size="sm"
                value={httpConfig.endpoint}
                placeholder={t('fields.endpoint.placeholder')}
                onChange={(event) => setConfig(previous => ({
                  ...previous,
                  providers: {
                    ...previous.providers,
                    bitfun_search_http: {
                      ...previous.providers.bitfun_search_http,
                      endpoint: event.target.value,
                    },
                  },
                }))}
              />
            </ConfigPageRow>
            <ConfigPageRow label={t('fields.authMode.label')} description={t('fields.authMode.description')} align="center">
              <Select
                value={httpConfig.auth.mode || 'none'}
                options={authOptions}
                size="sm"
                onValueChange={(value) => setConfig(previous => ({
                  ...previous,
                  providers: {
                    ...previous.providers,
                    bitfun_search_http: {
                      ...previous.providers.bitfun_search_http,
                      auth: {
                        ...previous.providers.bitfun_search_http.auth,
                        mode: normalizeSelectValue(value) as HttpAuthMode,
                      },
                    },
                  },
                }))}
              />
            </ConfigPageRow>
            {httpConfig.auth.mode === 'header' ? (
              <ConfigPageRow label={t('fields.headerName.label')} description={t('fields.headerName.description')} balanced>
                <Input
                  size="sm"
                  value={httpConfig.auth.headerName}
                  placeholder={t('fields.headerName.placeholder')}
                  onChange={(event) => setConfig(previous => ({
                    ...previous,
                    providers: {
                      ...previous.providers,
                      bitfun_search_http: {
                        ...previous.providers.bitfun_search_http,
                        auth: {
                          ...previous.providers.bitfun_search_http.auth,
                          headerName: event.target.value,
                        },
                      },
                    },
                  }))}
                />
              </ConfigPageRow>
            ) : null}
          </ConfigPageSection>
        ) : null}

        {credentialRequired ? (
          <ConfigPageSection title={t('sections.credential.title')} description={t('sections.credential.description')}>
            <ConfigPageRow label={t('fields.credentialStatus.label')} description={t('fields.credentialStatus.description')} balanced>
              <div className="web-search-settings__credential-status">
                <span>{credentialConfigured ? t('status.configured') : t('status.missing')}</span>
                <Button size="sm" variant="outline" disabled={!credentialConfigured || credentialBusy} onClick={() => void handleClearCredential()}>
                  {t('actions.clearCredential')}
                </Button>
              </div>
            </ConfigPageRow>
            <ConfigPageRow label={t('fields.credential.label')} description={t('fields.credential.description')} balanced>
              <div className="web-search-settings__credential-field">
                <Input
                  type="password"
                  size="sm"
                  autoComplete="off"
                  value={credential}
                  placeholder={credentialConfigured
                    ? t('fields.credential.replacePlaceholder')
                    : t('fields.credential.placeholder')}
                  onChange={(event) => setCredential(event.target.value)}
                />
                <Button size="sm" variant="fill" loading={credentialBusy} onClick={() => void handleSaveCredential()}>
                  {t('actions.saveCredential')}
                </Button>
              </div>
            </ConfigPageRow>
          </ConfigPageSection>
        ) : null}

        {selectedProvider === 'bitfun_search_http' ? (
          <ConfigPageSection
            title={t('sections.protocol.title')}
            description={t('sections.protocol.description')}
            extra={(
              <Tooltip content={t('actions.copyProtocol')} placement="bottom">
                <IconButton
                  type="button"
                  size="sm"
                  variant="quiet"
                  aria-label={t('actions.copyProtocol')}
                  icon={<Icon name="duplicate" size="sm" />}
                  onClick={() => void handleCopyProtocol()}
                />
              </Tooltip>
            )}
          >
            <div
              ref={protocolRef}
              id="web-search-bitfun-protocol"
              data-bf-component="config"
              data-bf-part="collectionDetails"
            >
              <section>
                <h4>{t('protocol.request.title')}</h4>
                <p>{t('protocol.request.description')}</p>
                <pre><code>{BITFUN_PROTOCOL_REQUEST_EXAMPLE}</code></pre>
              </section>
              <section>
                <h4>{t('protocol.success.title')}</h4>
                <p>{t('protocol.success.description')}</p>
                <pre><code>{BITFUN_PROTOCOL_SUCCESS_EXAMPLE}</code></pre>
              </section>
              <section>
                <h4>{t('protocol.error.title')}</h4>
                <p>{t('protocol.error.description')}</p>
                <pre><code>{BITFUN_PROTOCOL_ERROR_EXAMPLE}</code></pre>
                <p>
                  {t('protocol.error.codes')}{' '}
                  {BITFUN_PROTOCOL_ERROR_CODES}
                </p>
              </section>
              <ul>
                <li>{t('protocol.notes.resultHandling')}</li>
                <li>{t('protocol.notes.transportLimits')}</li>
              </ul>
            </div>
          </ConfigPageSection>
        ) : null}

      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default WebSearchSettingsPage;
