import React from 'react';
import { AlertTriangle } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  useAgentsStore,
  CAPABILITY_CATEGORIES,
  CAPABILITY_COLORS,
  computeAgentTeamCapabilities,
  type CapabilityCategory,
} from '../agentsStore';
import './CapabilityBar.scss';

const CapabilityBar: React.FC = () => {
  const { t } = useTranslation('scenes/agents');
  const { agentTeams, activeAgentTeamId, teamComposerAgents } = useAgentsStore();
  const team = agentTeams.find((t) => t.id === activeAgentTeamId);
  if (!team) return null;

  const coverage = computeAgentTeamCapabilities(team, teamComposerAgents);
  const weak = CAPABILITY_CATEGORIES.filter((c) => coverage[c] === 0);

  return (
    <div className="cap-bar" data-bf-component="capability-bar" data-bf-part="root">
      <span className="cap-bar__label" data-bf-component="capability-bar" data-bf-part="label">{t('capability.coverage')}</span>

      <div className="cap-bar__items" data-bf-component="capability-bar" data-bf-part="items">
        {CAPABILITY_CATEGORIES.map((cat) => {
          const level = coverage[cat];
          const color = CAPABILITY_COLORS[cat as CapabilityCategory];
          const pct = Math.round((level / 5) * 100);
          return (
            <div
              key={cat}
              className="cap-bar__item"
              title={`${cat}: ${level > 0 ? `Lv${level}` : t('capability.none')}`}
            >
              <span className="cap-bar__cat">{cat}</span>
              <div className="cap-bar__track">
                <div
                  className="cap-bar__fill"
                  style={{ width: `${pct}%`, background: level > 0 ? color : undefined }}
                />
              </div>
              <span
                className="cap-bar__lv"
                style={level > 0 ? { color } : undefined}
              >
                {level > 0 ? level : '—'}
              </span>
            </div>
          );
        })}
      </div>

      {weak.length > 0 && (
        <div className="cap-bar__warn" data-bf-component="capability-bar" data-bf-part="warn">
          <AlertTriangle size={10} />
          {t('capability.warning', { cats: weak.join(', ') })}
        </div>
      )}
    </div>
  );
};

export default CapabilityBar;
