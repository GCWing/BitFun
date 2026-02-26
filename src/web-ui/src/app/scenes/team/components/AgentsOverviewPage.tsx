import React, { useState } from 'react';
import { Search, Bot, User } from 'lucide-react';
import {
  useTeamStore,
  MOCK_AGENTS,
  type AgentWithCapabilities,
} from '../teamStore';
import { AGENT_ICON_MAP } from '../teamIcons';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import './TeamHomePage.scss';

// ─── Agent card ───────────────────────────────────────────────────────────────

const AgentCard: React.FC<{
  agent: AgentWithCapabilities;
  soloEnabled: boolean;
  onToggleSolo: (agentId: string, enabled: boolean) => void;
}> = ({ agent, soloEnabled, onToggleSolo }) => {
  const iconKey = (agent.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP;
  const IconComp = AGENT_ICON_MAP[iconKey] ?? Bot;

  return (
    <div className="th-card">
      <div className="th-card__head">
        <div className="th-card__icon">
          <IconComp size={16} />
        </div>
        <span className="th-card__type">
          <User size={9} />
          Agent
        </span>
      </div>
      <div className="th-card__name">{agent.name}</div>
      <div className="th-card__desc">{agent.description}</div>
      <div className="th-card__tags">
        {agent.capabilities.slice(0, 3).map((c) => (
          <span key={c.category} className="th-card__tag">
            {c.category}
          </span>
        ))}
      </div>
      <div className="th-card__foot">
        <span className="th-card__model">{agent.model ?? 'primary'}</span>
        <div className="th-card__foot-right">
          <button
            className={`th-card__solo-toggle ${soloEnabled ? 'is-on' : ''}`}
            type="button"
            onClick={() => onToggleSolo(agent.id, !soloEnabled)}
          >
            {soloEnabled ? '可独立使用' : '仅团队协作'}
          </button>
        </div>
      </div>
    </div>
  );
};

// ─── Page ─────────────────────────────────────────────────────────────────────

const AgentsOverviewPage: React.FC = () => {
  const { t } = useI18n('scenes/team');
  const { agentSoloEnabled, setAgentSoloEnabled } = useTeamStore();
  const [query, setQuery] = useState('');

  const agents = MOCK_AGENTS;
  const filteredAgents = agents.filter((a) => {
    if (!query) return true;
    const q = query.toLowerCase();
    return a.name.toLowerCase().includes(q) || a.description.toLowerCase().includes(q);
  });

  return (
    <div className="th">
      <div className="th__bar">
        <div className="th__title-wrap">
          <h3 className="th__title">{t('agentsOverview.title')}</h3>
          <span className="th__title-sub">{t('agentsOverview.subtitle')}</span>
        </div>
        <div className="th__search-wrap">
          <Search size={12} className="th__search-ico" />
          <input
            className="th__search"
            placeholder={t('home.search')}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
      </div>
      <div className="th__body">
        <section className="th__section th__section--agents">
          <div className="th__section-head">
            <span className="th__section-title">{t('agentsOverview.sectionTitle')}</span>
            <span className="th__section-count">{filteredAgents.length}</span>
          </div>
          <div className="th__section-grid">
            {filteredAgents.map((a) => (
              <AgentCard
                key={a.id}
                agent={a}
                soloEnabled={agentSoloEnabled[a.id] ?? false}
                onToggleSolo={setAgentSoloEnabled}
              />
            ))}
          </div>
        </section>
      </div>

    </div>
  );
};

export default AgentsOverviewPage;
