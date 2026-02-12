import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Loader2, PlugZap, Unplug } from 'lucide-react';
import { Button, Card } from '@/component-library';
import { MCPAPI, MCPServerInfo } from '@/infrastructure/api/service-api/MCPAPI';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { ConfigPageContent, ConfigPageHeader, ConfigPageLayout } from './common';
import './IntegrationsConfig.scss';

const log = createLogger('IntegrationsConfig');

type IntegrationId = 'notion';

const INTEGRATIONS: Array<{
  id: IntegrationId;
  defaultConfig: Record<string, any>;
}> = [
  {
    id: 'notion',
    defaultConfig: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', 'mcp-remote', 'https://mcp.notion.com/mcp'],
      enabled: true,
      autoStart: false,
      name: 'Notion'
    }
  }
];

function getMcpStatusClass(status: string): 'ok' | 'pending' | 'error' | 'unknown' {
  const statusLower = status.toLowerCase();
  if (statusLower.includes('healthy') || statusLower.includes('connected')) return 'ok';
  if (statusLower.includes('starting') || statusLower.includes('reconnecting') || statusLower.includes('stopping')) {
    return 'pending';
  }
  if (statusLower.includes('failed')) return 'error';
  if (statusLower.includes('stopped') || statusLower.includes('uninitialized')) return 'unknown';
  return 'unknown';
}

function IntegrationLogo({ id }: { id: IntegrationId }) {
  if (id === 'notion') {
    return (
      <svg
        className="integration-logo integration-logo--notion"
        viewBox="0 0 24 24"
        fill="none"
        aria-hidden="true"
      >
        <rect x="4" y="4" width="16" height="16" rx="3" stroke="currentColor" strokeWidth="1.5" />
        <path
          d="M9 16V8L15 16V8"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    );
  }
  return null;
}

function getIntegrationIcon(integrationId: IntegrationId) {
  switch (integrationId) {
    case 'notion':
      return <IntegrationLogo id="notion" />;
    default:
      return null;
  }
}

function deriveStatusLabelKey(status: string): 'connected' | 'connecting' | 'reconnecting' | 'disconnecting' | 'failed' | 'notConnected' {
  const s = status.toLowerCase();
  if (s.includes('healthy') || s.includes('connected')) return 'connected';
  if (s.includes('starting')) return 'connecting';
  if (s.includes('reconnecting')) return 'reconnecting';
  if (s.includes('stopping')) return 'disconnecting';
  if (s.includes('failed')) return 'failed';
  return 'notConnected';
}

function deriveConnected(status: string): boolean {
  const s = status.toLowerCase();
  return (
    s.includes('healthy')
    || s.includes('connected')
    || s.includes('reconnecting')
    || s.includes('stopping')
  );
}

function deriveActionMode(status: string): 'connect' | 'disconnect' | 'working' {
  const s = status.toLowerCase();
  if (s.includes('starting') || s.includes('stopping')) return 'working';
  return deriveConnected(status) ? 'disconnect' : 'connect';
}

const IntegrationsConfig: React.FC = () => {
  const { t } = useTranslation('settings/integrations');
  const notification = useNotification();

  const [servers, setServers] = useState<Record<string, MCPServerInfo | null>>({});
  const [busy, setBusy] = useState<Record<string, boolean>>({});
  const [busyAction, setBusyAction] = useState<Partial<Record<IntegrationId, 'connect' | 'disconnect'>>>({});

  const refreshServers = useCallback(async () => {
    try {
      const list = await MCPAPI.getServers();
      const map: Record<string, MCPServerInfo | null> = {};
      for (const integration of INTEGRATIONS) {
        map[integration.id] = list.find((s) => s.id === integration.id) ?? null;
      }
      setServers(map);
    } catch (error) {
      log.warn('Failed to load MCP servers for integrations', error);
      const map: Record<string, MCPServerInfo | null> = {};
      for (const integration of INTEGRATIONS) {
        map[integration.id] = null;
      }
      setServers(map);
    }
  }, []);

  useEffect(() => {
    void refreshServers();
  }, [refreshServers]);

  useEffect(() => {
    const handle = window.setInterval(() => {
      void refreshServers();
    }, 5000);
    return () => window.clearInterval(handle);
  }, [refreshServers]);

  const ensureIntegrationConfigured = async (serverId: IntegrationId) => {
    const integration = INTEGRATIONS.find((i) => i.id === serverId);
    if (!integration) {
      throw new Error(`Unknown integration: ${serverId}`);
    }

    const jsonConfig = await MCPAPI.loadMCPJsonConfig();
    let configObj: any;
    try {
      configObj = JSON.parse(jsonConfig);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      throw new Error(t('errors.invalidMcpConfig', { message }));
    }

    if (!configObj || typeof configObj !== 'object') {
      configObj = {};
    }
    if (!configObj.mcpServers || typeof configObj.mcpServers !== 'object' || Array.isArray(configObj.mcpServers)) {
      configObj.mcpServers = {};
    }

    const existing = configObj.mcpServers[serverId];
    const safeExisting = existing && typeof existing === 'object' && !Array.isArray(existing) ? existing : {};

    const merged: any = {
      ...safeExisting,
      ...integration.defaultConfig,
      url: null,
      headers: safeExisting?.headers ?? {}
    };
    if (!merged.env || typeof merged.env !== 'object' || Array.isArray(merged.env)) {
      merged.env = {};
    }

    configObj.mcpServers[serverId] = merged;
    await MCPAPI.saveMCPJsonConfig(JSON.stringify(configObj, null, 2));
  };

  const connect = async (serverId: IntegrationId) => {
    try {
      setBusyAction((prev) => ({ ...prev, [serverId]: 'connect' }));
      setBusy((prev) => ({ ...prev, [serverId]: true }));
      await ensureIntegrationConfigured(serverId);
      await MCPAPI.startServer(serverId);
      notification.success(t('messages.connected', { name: t(`integrations.${serverId}`) }));
    } catch (error) {
      log.error('Failed to connect integration', { serverId, error });
      notification.error(
        error instanceof Error ? error.message : t('errors.connectFailed'),
        { title: t(`integrations.${serverId}`) }
      );
    } finally {
      await refreshServers();
      setBusy((prev) => ({ ...prev, [serverId]: false }));
      setBusyAction((prev) => {
        const next = { ...prev };
        delete next[serverId];
        return next;
      });
    }
  };

  const disconnect = async (serverId: IntegrationId) => {
    try {
      setBusyAction((prev) => ({ ...prev, [serverId]: 'disconnect' }));
      setBusy((prev) => ({ ...prev, [serverId]: true }));
      await MCPAPI.stopServer(serverId);
      notification.success(t('messages.disconnected', { name: t(`integrations.${serverId}`) }));
    } catch (error) {
      log.error('Failed to disconnect integration', { serverId, error });
      notification.error(t('errors.disconnectFailed'), { title: t(`integrations.${serverId}`) });
    } finally {
      await refreshServers();
      setBusy((prev) => ({ ...prev, [serverId]: false }));
      setBusyAction((prev) => {
        const next = { ...prev };
        delete next[serverId];
        return next;
      });
    }
  };

  const items = useMemo(() => {
    return INTEGRATIONS.map((integration) => {
      const server = servers[integration.id] ?? null;
      const status = server?.status ?? 'Uninitialized';
      const rawStatusClass = getMcpStatusClass(status);
      const rawConnected = deriveConnected(status);
      const rawActionMode = deriveActionMode(status);

      const action = busyAction[integration.id];
      const busyNow = !!busy[integration.id];

      const statusClass = action ? 'pending' : rawStatusClass;
      const connected =
        action === 'disconnect' ? true : action === 'connect' ? false : rawConnected;
      const statusLabelKey =
        action === 'connect'
          ? 'connecting'
          : action === 'disconnect'
            ? 'disconnecting'
            : deriveStatusLabelKey(status);

      const actionMode = action ? 'working' : rawActionMode;
      const actionDisabledFromStatus = actionMode === 'working';
      return {
        id: integration.id,
        label: t(`integrations.${integration.id}`),
        status,
        statusClass,
        connected,
        statusLabelKey,
        busy: busyNow,
        actionMode,
        actionDisabledFromStatus,
      };
    });
  }, [busy, busyAction, servers, t]);

  return (
    <ConfigPageLayout className="integrations-config-panel">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent className="integrations-config-panel__content">
        <div className="integrations-list">
          {items.map((item) => (
            <Card
              key={item.id}
              variant="elevated"
              padding="none"
              fullWidth
              className={`integration-card integration-card--${item.id}`}
            >
              <div className="integration-card__content">
                <div className="integration-card__icon" aria-hidden="true">
                  {getIntegrationIcon(item.id)}
                </div>

                <div className="integration-card__main">
                  <div className="integration-card__top">
                    <div className="integration-card__title">{item.label}</div>
                    <div
                      className={`integration-card__status integration-card__status--${item.statusClass}`}
                      title={item.status}
                    >
                      <span className="integration-card__status-dot" aria-hidden="true" />
                      {t(`status.${item.statusLabelKey}`)}
                    </div>
                  </div>
                </div>

                <div className="integration-card__actions">
                  <Button
                    variant={item.connected ? 'secondary' : 'primary'}
                    size="small"
                    disabled={item.busy || item.actionDisabledFromStatus}
                    onClick={() => {
                      if (item.actionMode === 'disconnect') {
                        void disconnect(item.id);
                      } else if (item.actionMode === 'connect') {
                        void connect(item.id);
                      }
                    }}
                  >
                    <span className="integration-card__button-inner">
                      {item.busy || item.actionMode === 'working' ? (
                        <Loader2 size={14} className="integration-card__spinner" />
                      ) : item.actionMode === 'disconnect' ? (
                        <Unplug size={14} />
                      ) : (
                        <PlugZap size={14} />
                      )}
                      <span>
                        {item.busy || item.actionMode === 'working'
                          ? t('actions.working')
                          : item.actionMode === 'disconnect'
                            ? t('actions.disconnect')
                            : t('actions.connect')}
                      </span>
                    </span>
                  </Button>
                </div>
              </div>
            </Card>
          ))}
        </div>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default IntegrationsConfig;
