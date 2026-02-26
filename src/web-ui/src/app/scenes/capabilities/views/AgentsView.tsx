/**
 * AgentsView — agents card list with toggle and expand details.
 */

import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Bot, RefreshCw } from 'lucide-react';
import { Switch, Card, CardBody } from '@/component-library';
import { configAPI } from '@/infrastructure/api';
import { SubagentAPI, type SubagentInfo } from '@/infrastructure/api/service-api/SubagentAPI';
import { useNotification } from '@/shared/notification-system';
import { isBuiltinSubAgent } from '@/infrastructure/agents/constants';
import './capabilities-views.scss';

function getSourceBadge(agent: SubagentInfo): string {
  if (agent.subagentSource === 'builtin' && isBuiltinSubAgent(agent.id)) return 'Sub-Agent';
  return agent.subagentSource ?? '';
}

const AgentsView: React.FC = () => {
  const { t } = useTranslation('scenes/capabilities');
  const { error: notifyError } = useNotification();
  const [agents, setAgents] = useState<SubagentInfo[]>([]);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(() => new Set());

  const load = useCallback(
    async (silent = false) => {
      try {
        const list = await SubagentAPI.listSubagents();
        setAgents(list);
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

  const toggleExpanded = useCallback((id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const handleToggle = useCallback(
    async (agent: SubagentInfo) => {
      try {
        const isCustom = agent.subagentSource === 'user' || agent.subagentSource === 'project';
        if (isCustom) {
          await SubagentAPI.updateSubagentConfig({ subagentId: agent.id, enabled: !agent.enabled });
        } else {
          await configAPI.setSubagentConfig(agent.id, !agent.enabled);
        }
        await load(true);
      } catch {
        notifyError(t('toggleFailed'));
      }
    },
    [load, notifyError, t]
  );

  return (
    <div className="bitfun-cap__view">
      <div className="bitfun-cap__content">
        {agents.length === 0 ? (
          <div className="bitfun-cap__empty">
            <span>{t('emptyAgents')}</span>
          </div>
        ) : (
          <div className="bitfun-cap__cards-grid">
            {agents.map((agent) => {
              const isExpanded = expandedIds.has(`agent:${agent.id}`);
              return (
                <Card
                  key={agent.id}
                  variant="default"
                  padding="none"
                  className={`bitfun-cap__card ${!agent.enabled ? 'is-disabled' : ''} ${isExpanded ? 'is-expanded' : ''}`}
                >
                  <div
                    className="bitfun-cap__card-header"
                    onClick={() => toggleExpanded(`agent:${agent.id}`)}
                  >
                    <div className="bitfun-cap__card-icon bitfun-cap__card-icon--agent">
                      <Bot size={13} />
                    </div>
                    <div className="bitfun-cap__card-info">
                      <span className="bitfun-cap__card-name">{agent.name}</span>
                      {agent.model && (
                        <span className="bitfun-cap__badge bitfun-cap__badge--blue">{agent.model}</span>
                      )}
                      {getSourceBadge(agent) && (
                        <span className="bitfun-cap__badge bitfun-cap__badge--gray">
                          {getSourceBadge(agent)}
                        </span>
                      )}
                    </div>
                    <div className="bitfun-cap__card-actions" onClick={(e) => e.stopPropagation()}>
                      <Switch
                        checked={agent.enabled}
                        onChange={() => handleToggle(agent)}
                        size="small"
                      />
                    </div>
                  </div>
                  {isExpanded && (
                    <CardBody className="bitfun-cap__card-details">
                      {agent.description && (
                        <div className="bitfun-cap__card-desc">{agent.description}</div>
                      )}
                      <div className="bitfun-cap__meta-row">
                        <span className="bitfun-cap__meta-label">{t('toolCount')}</span>
                        <span className="bitfun-cap__meta-value">{agent.toolCount}</span>
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

export default AgentsView;
