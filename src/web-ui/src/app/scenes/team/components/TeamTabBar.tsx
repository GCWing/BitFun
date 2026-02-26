import React, { useState } from 'react';
import { Plus, X, Code2, BarChart2, LayoutTemplate, Rocket, Users, Briefcase, Layers, type LucideIcon } from 'lucide-react';
import { useTeamStore, TEAM_TEMPLATES } from '../teamStore';
import { TEAM_ICON_MAP, getTeamAccent } from '../teamIcons';
import './TeamTabBar.scss';

// ─── Team icon renderer ───────────────────────────────────────────────────────

const TeamIconBadge: React.FC<{ iconKey: string; teamId: string; size?: number }> = ({
  iconKey,
  teamId,
  size = 12,
}) => {
  const accent = getTeamAccent(teamId);
  const key = iconKey as keyof typeof TEAM_ICON_MAP;
  const IconComp = TEAM_ICON_MAP[key] ?? Users;
  return (
    <span
      className="bt-tabbar__team-icon"
      style={{ color: accent }}
    >
      <IconComp size={size} />
    </span>
  );
};

// ─── New team form ────────────────────────────────────────────────────────────

const ICON_OPTIONS: Array<{ key: string; Icon: LucideIcon }> = [
  { key: 'code',      Icon: Code2 },
  { key: 'chart',     Icon: BarChart2 },
  { key: 'layout',    Icon: LayoutTemplate },
  { key: 'rocket',    Icon: Rocket },
  { key: 'users',     Icon: Users },
  { key: 'briefcase', Icon: Briefcase },
  { key: 'layers',    Icon: Layers },
];

interface NewTeamForm { name: string; icon: string; description: string }

const TeamTabBar: React.FC = () => {
  const { teams, activeTeamId, setActiveTeam, addTeam, deleteTeam } = useTeamStore();
  const [panel, setPanel] = useState<'none' | 'create' | 'templates'>('none');
  const [form, setForm] = useState<NewTeamForm>({ name: '', icon: 'rocket', description: '' });

  const closePanel = () => setPanel('none');

  const handleCreate = () => {
    if (!form.name.trim()) return;
    addTeam({ id: `team-${Date.now()}`, ...form, strategy: 'collaborative', shareContext: true });
    setForm({ name: '', icon: 'rocket', description: '' });
    closePanel();
  };

  const handleUseTemplate = (tpl: typeof TEAM_TEMPLATES[number]) => {
    addTeam({
      id: `team-${Date.now()}`,
      name: tpl.name,
      icon: tpl.icon,
      description: tpl.description,
      strategy: 'collaborative',
      shareContext: true,
    });
    closePanel();
  };

  const handleDelete = (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    if (teams.length <= 1) return;
    deleteTeam(id);
  };

  return (
    <div className="bt-tabbar">
      <div className="bt-tabbar__rail">
        {/* ── Tabs ── */}
        {teams.map((team) => {
          const isActive = team.id === activeTeamId;
          return (
            <button
              key={team.id}
              className={`bt-tabbar__tab ${isActive ? 'is-active' : ''}`}
              onClick={() => setActiveTeam(team.id)}
            >
              <TeamIconBadge iconKey={team.icon} teamId={team.id} size={12} />
              <span className="bt-tabbar__tab-name">{team.name}</span>
              <span className="bt-tabbar__tab-count">{team.members.length}</span>
              {teams.length > 1 && (
                <span
                  className="bt-tabbar__tab-close"
                  onClick={(e) => handleDelete(e, team.id)}
                  role="button"
                  tabIndex={-1}
                >
                  <X size={9} />
                </span>
              )}
            </button>
          );
        })}

        {/* ── Divider ── */}
        <span className="bt-tabbar__sep" />

        {/* ── New team ── */}
        <button
          className={`bt-tabbar__new ${panel !== 'none' ? 'is-open' : ''}`}
          onClick={() => setPanel(panel === 'none' ? 'create' : 'none')}
        >
          <Plus size={12} />
          <span>新建</span>
        </button>
      </div>

      {/* ── Create panel ── */}
      {panel === 'create' && (
        <div className="bt-tabbar__panel">
          {/* Icon selector */}
          <div className="bt-tabbar__icon-row">
            {ICON_OPTIONS.map(({ key, Icon }) => (
              <button
                key={key}
                className={`bt-tabbar__icon-opt ${form.icon === key ? 'is-sel' : ''}`}
                onClick={() => setForm((f) => ({ ...f, icon: key }))}
                style={form.icon === key ? { color: getTeamAccent(`team-${key}`) } : undefined}
              >
                <Icon size={14} />
              </button>
            ))}
          </div>

          <input
            className="bt-tabbar__field"
            placeholder="团队名称"
            value={form.name}
            onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
            onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
            autoFocus
          />
          <input
            className="bt-tabbar__field"
            placeholder="描述（可选）"
            value={form.description}
            onChange={(e) => setForm((f) => ({ ...f, description: e.target.value }))}
          />

          <div className="bt-tabbar__panel-row">
            <button
              className="bt-tabbar__action bt-tabbar__action--ghost"
              onClick={() => setPanel('templates')}
            >
              从模板
            </button>
            <div style={{ flex: 1 }} />
            <button className="bt-tabbar__action bt-tabbar__action--ghost" onClick={closePanel}>
              取消
            </button>
            <button
              className="bt-tabbar__action bt-tabbar__action--primary"
              onClick={handleCreate}
              disabled={!form.name.trim()}
            >
              创建
            </button>
          </div>
        </div>
      )}

      {/* ── Templates panel ── */}
      {panel === 'templates' && (
        <div className="bt-tabbar__panel bt-tabbar__panel--wide">
          <div className="bt-tabbar__tpl-head">
            <span className="bt-tabbar__tpl-title">选择团队模板</span>
            <button className="bt-tabbar__close-btn" onClick={closePanel}><X size={12} /></button>
          </div>
          <div className="bt-tabbar__tpl-grid">
            {TEAM_TEMPLATES.map((tpl) => {
              const key = tpl.icon as keyof typeof TEAM_ICON_MAP;
              const IconComp = TEAM_ICON_MAP[key] ?? Users;
              const accent = getTeamAccent(`team-${tpl.id}`);
              return (
                <button
                  key={tpl.id}
                  className="bt-tabbar__tpl-card"
                  onClick={() => handleUseTemplate(tpl)}
                >
                  <span className="bt-tabbar__tpl-icon" style={{ color: accent, borderColor: `${accent}30` }}>
                    <IconComp size={16} />
                  </span>
                  <div className="bt-tabbar__tpl-info">
                    <span className="bt-tabbar__tpl-name">{tpl.name}</span>
                    <span className="bt-tabbar__tpl-desc">{tpl.description}</span>
                  </div>
                  <span className="bt-tabbar__tpl-cnt">{tpl.memberIds.length}</span>
                </button>
              );
            })}
          </div>
          <button
            className="bt-tabbar__action bt-tabbar__action--ghost"
            style={{ width: '100%', marginTop: 4 }}
            onClick={() => setPanel('create')}
          >
            ← 空白创建
          </button>
        </div>
      )}

      {(panel !== 'none') && (
        <div className="bt-tabbar__backdrop" onClick={closePanel} />
      )}
    </div>
  );
};

export default TeamTabBar;
