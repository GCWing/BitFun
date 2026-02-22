/**
 * MCPView — MCP servers card list with status and reconnect.
 */

import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Plug, RefreshCw, AlertTriangle } from 'lucide-react';
import { Card, CardBody } from '@/component-library';
import { MCPAPI, type MCPServerInfo } from '@/infrastructure/api/service-api/MCPAPI';
import { useNotification } from '@/shared/notification-system';
import './capabilities-views.scss';

const MCP_HEALTHY_STATUSES = new Set(['connected', 'healthy']);

const MCPView: React.FC = () => {
  const { t } = useTranslation('scenes/capabilities');
  const { error: notifyError, success: notifySuccess } = useNotification();
  const [mcpServers, setMcpServers] = useState<MCPServerInfo[]>([]);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(() => new Set());

  const load = useCallback(
    async (silent = false) => {
      try {
        const list = await MCPAPI.getServers();
        setMcpServers(list);
      } catch (err) {
        if (!silent) notifyError(t('loadFailed'));
      }
    },
    [notifyError, t]
  );

  useEffect(() => {
    load();
  }, [load]);

  const handleRefresh = useCallback(async () => {
    setIsRefreshing(true);
    try {
      await load(true);
    } finally {
      setIsRefreshing(false);
    }
  }, [load]);

  const handleReconnect = useCallback(
    async (server: MCPServerInfo) => {
      try {
        if ((server.status || '').toLowerCase() === 'stopped') {
          await MCPAPI.startServer(server.id);
        } else {
          await MCPAPI.restartServer(server.id);
        }
        await load(true);
        notifySuccess(t('mcpReconnectSuccess', { name: server.name }));
      } catch {
        notifyError(t('mcpReconnectFailed', { name: server.name }));
      }
    },
    [load, notifyError, notifySuccess, t]
  );

  const toggleExpanded = useCallback((id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const enabledMcp = useMemo(() => mcpServers.filter((s) => s.enabled), [mcpServers]);
  const unhealthyMcp = useMemo(
    () =>
      enabledMcp.filter((s) => !MCP_HEALTHY_STATUSES.has((s.status || '').toLowerCase())),
    [enabledMcp]
  );
  const hasMcpIssue = unhealthyMcp.length > 0;

  return (
    <div className="bitfun-cap__view">
      {hasMcpIssue && (
        <div className="bitfun-cap__alert">
          <AlertTriangle size={12} />
          <span>{t('mcpWarning', { count: unhealthyMcp.length })}</span>
        </div>
      )}
      <div className="bitfun-cap__content">
        {mcpServers.length === 0 ? (
          <div className="bitfun-cap__empty" />
        ) : (
          <div className="bitfun-cap__cards-grid">
            {mcpServers.map((server) => {
              const healthy = MCP_HEALTHY_STATUSES.has((server.status || '').toLowerCase());
              const isExpanded = expandedIds.has(`mcp:${server.id}`);
              return (
                <Card
                  key={server.id}
                  variant="default"
                  padding="none"
                  className={`bitfun-cap__card ${isExpanded ? 'is-expanded' : ''} ${!healthy ? 'is-unhealthy' : ''}`}
                >
                  <div
                    className="bitfun-cap__card-header"
                    onClick={() => toggleExpanded(`mcp:${server.id}`)}
                  >
                    <div
                      className={`bitfun-cap__card-icon bitfun-cap__card-icon--mcp ${!healthy ? 'is-error' : ''}`}
                    >
                      <Plug size={13} />
                    </div>
                    <div className="bitfun-cap__card-info">
                      <span className="bitfun-cap__card-name">{server.name}</span>
                      <span
                        className={`bitfun-cap__badge bitfun-cap__badge--${healthy ? 'green' : 'yellow'}`}
                      >
                        {server.status}
                      </span>
                    </div>
                    {!healthy && (
                      <div className="bitfun-cap__card-actions" onClick={(e) => e.stopPropagation()}>
                        <button
                          type="button"
                          className="bitfun-cap__row-reconnect"
                          onClick={() => handleReconnect(server)}
                          title={t('reconnect')}
                        >
                          <RefreshCw size={11} />
                        </button>
                      </div>
                    )}
                  </div>
                  {isExpanded && (
                    <CardBody className="bitfun-cap__card-details">
                      <div className="bitfun-cap__meta-row">
                        <span className="bitfun-cap__meta-label">{t('serverType')}</span>
                        <span className="bitfun-cap__meta-value">{server.serverType}</span>
                      </div>
                    </CardBody>
                  )}
                </Card>
              );
            })}
          </div>
        )}
      </div>
      <div className="bitfun-cap__footer">
        <button
          type="button"
          className={`bitfun-cap__refresh ${isRefreshing ? 'is-spinning' : ''}`}
          onClick={handleRefresh}
          title={t('refresh')}
        >
          <RefreshCw size={11} />
          <span>{t('refresh')}</span>
        </button>
      </div>
    </div>
  );
};

export default MCPView;
