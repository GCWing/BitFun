import React, { useState, useCallback, useEffect } from 'react';
import { Bot, Cpu, SlidersHorizontal } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Search, Switch, IconButton, Badge } from '@/component-library';
import {
  useTeamStore,
  type AgentWithCapabilities,
  type AgentKind,
} from '../teamStore';
import { CAPABILITY_ACCENT } from '../teamIcons';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { SubagentAPI } from '@/infrastructure/api/service-api/SubagentAPI';
import type { SubagentSource } from '@/infrastructure/api/service-api/SubagentAPI';
import './TeamHomePage.scss';

// ─── Agent badge ──────────────────────────────────────────────────────────────

interface AgentBadgeConfig {
  variant: 'accent' | 'info' | 'success' | 'purple' | 'neutral';
  label: string;
}

function getAgentBadge(agentKind?: AgentKind, source?: SubagentSource): AgentBadgeConfig {
  if (agentKind === 'mode') {
    return { variant: 'accent', label: 'Agent' };
  }
  switch (source) {
    case 'user':    return { variant: 'success', label: '用户 Sub-Agent' };
    case 'project': return { variant: 'purple',  label: '项目 Sub-Agent' };
    default:        return { variant: 'info',    label: 'Sub-Agent' };
  }
}

// ─── Enrich capabilities ──────────────────────────────────────────────────────

function enrichCapabilities(agent: AgentWithCapabilities): AgentWithCapabilities {
  if (agent.capabilities?.length) return agent;
  const id   = agent.id.toLowerCase();
  const name = agent.name.toLowerCase();

  if (agent.agentKind === 'mode') {
    if (id === 'agentic') return { ...agent, capabilities: [{ category: '编码', level: 5 }, { category: '分析', level: 4 }] };
    if (id === 'plan')    return { ...agent, capabilities: [{ category: '分析', level: 5 }, { category: '文档', level: 3 }] };
    if (id === 'debug')   return { ...agent, capabilities: [{ category: '编码', level: 5 }, { category: '分析', level: 3 }] };
    if (id === 'cowork')  return { ...agent, capabilities: [{ category: '分析', level: 4 }, { category: '创意', level: 3 }] };
  }

  if (id === 'explore')     return { ...agent, capabilities: [{ category: '分析', level: 4 }, { category: '编码', level: 3 }] };
  if (id === 'file_finder') return { ...agent, capabilities: [{ category: '分析', level: 3 }, { category: '编码', level: 2 }] };

  if (name.includes('code') || name.includes('debug') || name.includes('test')) {
    return { ...agent, capabilities: [{ category: '编码', level: 4 }] };
  }
  if (name.includes('doc') || name.includes('write')) {
    return { ...agent, capabilities: [{ category: '文档', level: 4 }] };
  }
  return { ...agent, capabilities: [{ category: '分析', level: 3 }] };
}

// ─── Agent list item ──────────────────────────────────────────────────────────

const AgentListItem: React.FC<{
  agent: AgentWithCapabilities;
  soloEnabled: boolean;
  onToggleSolo: (agentId: string, enabled: boolean) => void;
  index: number;
}> = ({ agent, soloEnabled, onToggleSolo, index }) => {
  const { t } = useTranslation('scenes/team');
  const [expanded, setExpanded] = useState(false);

  const toggleExpand = useCallback(() => setExpanded((v) => !v), []);

  const badge = getAgentBadge(agent.agentKind, agent.subagentSource);

  return (
    <div
      className={['th-list__item', expanded && 'is-expanded'].filter(Boolean).join(' ')}
      style={{ '--item-index': index } as React.CSSProperties}
    >
      <div
        className="th-list__item-row"
        onClick={toggleExpand}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => e.key === 'Enter' && toggleExpand()}
      >
        <div className="th-list__item-info">
          <div className="th-list__item-name-row">
            <span className="th-list__item-name">{agent.name}</span>
            <Badge variant={badge.variant}>
              {agent.agentKind === 'mode' ? <Cpu size={9} /> : <Bot size={9} />}
              {badge.label}
            </Badge>
            {agent.model && (
              <Badge variant="neutral">{agent.model}</Badge>
            )}
          </div>
          <p className="th-list__item-desc">{agent.description}</p>
        </div>

        <div className="th-list__item-meta">
          {agent.capabilities.slice(0, 3).map((cap) => (
            <span key={cap.category} className="th-list__cap-chip">
              {cap.category}
            </span>
          ))}
        </div>

        <div className="th-list__item-action" onClick={(e) => e.stopPropagation()}>
          <Switch
            checked={soloEnabled}
            onChange={() => onToggleSolo(agent.id, !soloEnabled)}
            size="small"
          />
          <IconButton
            variant="ghost"
            size="small"
            tooltip={t('manage')}
          >
            <SlidersHorizontal size={14} />
          </IconButton>
        </div>
      </div>

      {expanded && (
        <div className="th-list__item-details">
          <p className="th-list__detail-desc">{agent.description}</p>
          <div className="th-list__cap-grid">
            {agent.capabilities.map((cap) => (
              <div key={cap.category} className="th-list__cap-row">
                <span
                  className="th-list__cap-label"
                  style={{ color: CAPABILITY_ACCENT[cap.category] }}
                >
                  {cap.category}
                </span>
                <div className="th-list__cap-bar">
                  {Array.from({ length: 5 }).map((_, i) => (
                    <span
                      key={i}
                      className={`th-list__cap-pip${i < cap.level ? ' is-filled' : ''}`}
                      style={
                        i < cap.level
                          ? ({ backgroundColor: CAPABILITY_ACCENT[cap.category] } as React.CSSProperties)
                          : undefined
                      }
                    />
                  ))}
                </div>
                <span className="th-list__cap-level">{cap.level}/5</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

// ─── Page ─────────────────────────────────────────────────────────────────────

const AgentsOverviewPage: React.FC = () => {
  const { t } = useTranslation('scenes/team');
  const { agentSoloEnabled, setAgentSoloEnabled } = useTeamStore();
  const [query, setQuery] = useState('');
  const [allAgents, setAllAgents] = useState<AgentWithCapabilities[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    Promise.all([
      agentAPI.getAvailableModes().catch(() => []),
      SubagentAPI.listSubagents().catch(() => []),
    ]).then(([modes, subagents]) => {
      if (cancelled) return;

      const modeAgents: AgentWithCapabilities[] = modes.map((m) =>
        enrichCapabilities({
          id: m.id,
          name: m.name,
          description: m.description,
          isReadonly: m.isReadonly,
          toolCount: m.toolCount,
          defaultTools: m.defaultTools ?? [],
          enabled: m.enabled,
          capabilities: [],
          agentKind: 'mode',
        })
      );

      const subAgents: AgentWithCapabilities[] = subagents.map((s) =>
        enrichCapabilities({
          ...s,
          capabilities: [],
          agentKind: 'subagent',
        })
      );

      setAllAgents([...modeAgents, ...subAgents]);
    }).finally(() => {
      if (!cancelled) setLoading(false);
    });

    return () => { cancelled = true; };
  }, []);

  const filteredAgents = allAgents.filter((a) => {
    if (!query) return true;
    const q = query.toLowerCase();
    return a.name.toLowerCase().includes(q) || a.description.toLowerCase().includes(q);
  });

  return (
    <div className="th">
      <div className="th__header">
        <div className="th__header-inner">
          <div className="th__title-row">
            <div>
              <h2 className="th__title">{t('agentsOverview.title')}</h2>
              <p className="th__title-sub">{t('agentsOverview.subtitle')}</p>
            </div>
          </div>
          <div className="th__toolbar">
            <Search
              placeholder={t('home.search')}
              value={query}
              onChange={setQuery}
              clearable
              size="small"
            />
          </div>
        </div>
      </div>

      <div className="th__list-body">
        <div className="th__list-inner">
          <div className="th-list__section-head">
            <span className="th-list__section-title">{t('agentsOverview.sectionTitle')}</span>
            <span className="th-list__section-count">{filteredAgents.length}</span>
          </div>
          {loading ? (
            <div className="th-list__empty">
              <Bot size={28} strokeWidth={1.5} />
              <span>{t('loading', '加载中…')}</span>
            </div>
          ) : filteredAgents.length === 0 ? (
            <div className="th-list__empty">
              <Bot size={28} strokeWidth={1.5} />
              <span>{t('empty')}</span>
            </div>
          ) : (
            <div className="th-list">
              {filteredAgents.map((a, i) => (
                <AgentListItem
                  key={a.id}
                  agent={a}
                  soloEnabled={agentSoloEnabled[a.id] ?? a.enabled}
                  onToggleSolo={setAgentSoloEnabled}
                  index={i}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default AgentsOverviewPage;
