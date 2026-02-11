import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Card } from '@/component-library';
import { MCPAPI, MCPServerInfo } from '@/infrastructure/api/service-api/MCPAPI';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { ConfigPageContent, ConfigPageHeader, ConfigPageLayout } from './common';
import './IntegrationsConfig.scss';

const log = createLogger('IntegrationsConfig');

type IntegrationId = 'notion' | 'gmail';

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
  },
  {
    id: 'gmail',
    defaultConfig: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@gongrzhe/server-gmail-autoauth-mcp'],
      enabled: true,
      autoStart: false,
      name: 'Gmail'
    }
  }
];

function getMcpStatusClass(status: string): 'ok' | 'pending' | 'error' | 'unknown' {
  const statusLower = status.toLowerCase();
  if (statusLower.includes('healthy') || statusLower.includes('connected')) return 'ok';
  if (statusLower.includes('starting') || statusLower.includes('reconnecting')) return 'pending';
  if (statusLower.includes('failed')) return 'error';
  if (statusLower.includes('stopped') || statusLower.includes('uninitialized')) return 'unknown';
  return 'unknown';
}

const IntegrationsConfig: React.FC = () => {
  const { t } = useTranslation('settings/integrations');
  const notification = useNotification();

  const [servers, setServers] = useState<Record<string, MCPServerInfo | null>>({});
  const [busy, setBusy] = useState<Record<string, boolean>>({});

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
    }
  };

  const disconnect = async (serverId: IntegrationId) => {
    try {
      setBusy((prev) => ({ ...prev, [serverId]: true }));
      await MCPAPI.stopServer(serverId);
      notification.success(t('messages.disconnected', { name: t(`integrations.${serverId}`) }));
    } catch (error) {
      log.error('Failed to disconnect integration', { serverId, error });
      notification.error(t('errors.disconnectFailed'), { title: t(`integrations.${serverId}`) });
    } finally {
      await refreshServers();
      setBusy((prev) => ({ ...prev, [serverId]: false }));
    }
  };

  const items = useMemo(() => {
    return INTEGRATIONS.map((integration) => {
      const server = servers[integration.id] ?? null;
      const status = server?.status ?? 'Uninitialized';
      const statusClass = getMcpStatusClass(status);
      const connected = statusClass === 'ok';
      return {
        id: integration.id,
        label: t(`integrations.${integration.id}`),
        status,
        statusClass,
        connected,
        busy: !!busy[integration.id],
      };
    });
  }, [busy, servers, t]);

  return (
    <ConfigPageLayout className="integrations-config-panel">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent className="integrations-config-panel__content">
        <div className="integrations-list">
          {items.map((item) => (
            <Card key={item.id} variant="default" padding="none" className="integration-card">
              <div className="integration-card__left">
                <div className="integration-card__title">{item.label}</div>
                <div className={`integration-card__status integration-card__status--${item.statusClass}`}>
                  {item.statusClass === 'ok'
                    ? t('status.connected')
                    : item.statusClass === 'pending'
                      ? t('status.connecting')
                      : item.statusClass === 'error'
                        ? t('status.failed')
                        : t('status.notConnected')}
                </div>
              </div>
              <div className="integration-card__right">
                <Button
                  variant={item.connected ? 'secondary' : 'primary'}
                  size="small"
                  disabled={item.busy}
                  onClick={() => {
                    if (item.connected) {
                      void disconnect(item.id);
                    } else {
                      void connect(item.id);
                    }
                  }}
                >
                  {item.busy
                    ? t('actions.working')
                    : item.connected
                      ? t('actions.disconnect')
                      : t('actions.connect')}
                </Button>
              </div>
            </Card>
          ))}
        </div>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default IntegrationsConfig;

