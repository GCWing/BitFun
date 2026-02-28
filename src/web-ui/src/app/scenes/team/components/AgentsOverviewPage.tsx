import React, { useState, useCallback } from 'react';
import { Bot, User, SlidersHorizontal } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Search, Switch, IconButton, Badge } from '@/component-library';
import {
  useTeamStore,
  MOCK_AGENTS,
  type AgentWithCapabilities,
} from '../teamStore';
import { CAPABILITY_ACCENT } from '../teamIcons';
import './TeamHomePage.scss';

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
            <Badge variant="neutral">
              <User size={9} />
              Agent
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

  const filteredAgents = MOCK_AGENTS.filter((a) => {
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
          {filteredAgents.length === 0 ? (
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
                  soloEnabled={agentSoloEnabled[a.id] ?? false}
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
