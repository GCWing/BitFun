import React from 'react';
import { AlertTriangle } from 'lucide-react';
import {
  useTeamStore,
  MOCK_AGENTS,
  CAPABILITY_CATEGORIES,
  CAPABILITY_COLORS,
  computeTeamCapabilities,
  type AgentWithCapabilities,
  type CapabilityCategory,
} from '../teamStore';
import './CapabilityBar.scss';

const CapabilityBar: React.FC = () => {
  const { teams, activeTeamId } = useTeamStore();
  const team = teams.find((t) => t.id === activeTeamId);
  if (!team) return null;

  const coverage = computeTeamCapabilities(team, MOCK_AGENTS as AgentWithCapabilities[]);
  const weak     = CAPABILITY_CATEGORIES.filter((c) => coverage[c] === 0);

  return (
    <div className="cap-bar">
      <span className="cap-bar__label">能力覆盖</span>

      <div className="cap-bar__items">
        {CAPABILITY_CATEGORIES.map((cat) => {
          const level = coverage[cat];
          const color = CAPABILITY_COLORS[cat as CapabilityCategory];
          const pct   = Math.round((level / 5) * 100);
          return (
            <div key={cat} className="cap-bar__item" title={`${cat}：${level > 0 ? `Lv${level}` : '无覆盖'}`}>
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
        <div className="cap-bar__warn">
          <AlertTriangle size={10} />
          {weak.join('、')} 缺失
        </div>
      )}
    </div>
  );
};

export default CapabilityBar;
