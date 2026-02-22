import React, { useState, useRef, useLayoutEffect, useCallback } from 'react';
import { LayoutGrid, List, Trash2, ChevronDown, Bot } from 'lucide-react';
import {
  useTeamStore,
  MOCK_AGENTS,
  CAPABILITY_COLORS,
  type Team,
  type TeamMember,
  type MemberRole,
  type AgentWithCapabilities,
  type CapabilityCategory,
} from '../teamStore';
import { AGENT_ICON_MAP } from '../teamIcons';
import './TeamComposer.scss';

// ─── Constants ────────────────────────────────────────────────────────────────

const ROLE_LABELS: Record<MemberRole, string> = {
  leader:   '主导',
  member:   '执行',
  reviewer: '审查',
};

const ROLE_COLORS: Record<MemberRole, string> = {
  leader:   '#60a5fa',
  member:   '#6eb88c',
  reviewer: '#c9944d',
};

// ─── Helpers ─────────────────────────────────────────────────────────────────

function getAgent(id: string): AgentWithCapabilities | undefined {
  return MOCK_AGENTS.find((a) => a.id === id);
}

const AgentIconSmall: React.FC<{ agent?: AgentWithCapabilities }> = ({ agent }) => {
  const primaryCap = agent?.capabilities[0]?.category;
  const color = primaryCap
    ? CAPABILITY_COLORS[primaryCap as CapabilityCategory]
    : 'var(--color-text-muted)';
  const key = (agent?.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP;
  const IconComp = AGENT_ICON_MAP[key] ?? Bot;
  return <IconComp size={13} style={{ color, flexShrink: 0 }} />;
};

// ─── Formation layout ─────────────────────────────────────────────────────────

interface NodePos { x: number; y: number; memberId: string }

function layoutNodes(members: TeamMember[]): NodePos[] {
  const leaders   = members.filter((m) => m.role === 'leader');
  const middles   = members.filter((m) => m.role === 'member');
  const reviewers = members.filter((m) => m.role === 'reviewer');
  const positions: NodePos[] = [];

  const placeRow = (group: TeamMember[], y: number) => {
    const n = group.length;
    group.forEach((m, i) => {
      const x = n === 1 ? 50 : 15 + (70 / Math.max(n - 1, 1)) * i;
      positions.push({ x, y, memberId: m.agentId });
    });
  };

  const rows = [leaders, middles, reviewers].filter((r) => r.length > 0);
  const ys   = rows.length === 1 ? [50] : rows.length === 2 ? [28, 72] : [18, 50, 82];
  rows.forEach((row, i) => placeRow(row, ys[i]));
  return positions;
}

function buildEdges(members: TeamMember[]): Array<[string, string]> {
  const l = members.filter((m) => m.role === 'leader').map((m) => m.agentId);
  const m = members.filter((m) => m.role === 'member').map((m) => m.agentId);
  const r = members.filter((m) => m.role === 'reviewer').map((m) => m.agentId);
  const edges: Array<[string, string]> = [];

  if (l.length && m.length) l.forEach((a) => m.forEach((b) => edges.push([a, b])));
  else if (l.length && r.length) l.forEach((a) => r.forEach((b) => edges.push([a, b])));

  if (m.length && r.length) m.forEach((a) => r.forEach((b) => edges.push([a, b])));

  if (!l.length && !r.length && m.length > 1) {
    for (let i = 0; i < m.length - 1; i++) edges.push([m[i], m[i + 1]]);
  }
  return edges;
}

// ─── Formation node ───────────────────────────────────────────────────────────

const NODE_W = 176;
const NODE_H = 72;

interface NodeProps {
  member: TeamMember;
  pos: NodePos;
  cw: number;
  ch: number;
  onRoleChange: (r: MemberRole) => void;
  onRemove: () => void;
}

const FormationNode: React.FC<NodeProps> = ({ member, pos, cw, ch, onRoleChange, onRemove }) => {
  const [roleOpen, setRoleOpen] = useState(false);
  const agent = getAgent(member.agentId);
  const x = (pos.x / 100) * cw - NODE_W / 2;
  const y = (pos.y / 100) * ch - NODE_H / 2;
  const roleColor = ROLE_COLORS[member.role];
  const primaryCap = agent?.capabilities[0]?.category;

  return (
    <div className="tcf__node" style={{ left: x, top: y, width: NODE_W }}>
      <div className="tcf__node-card" style={{ borderTopColor: roleColor }}>
        {/* Row 1: name + role + delete */}
        <div className="tcf__node-head">
          <AgentIconSmall agent={agent} />
          <span className="tcf__node-name">{agent?.name ?? member.agentId}</span>
          <button className="tcf__node-del" onClick={onRemove} title="移出">
            <Trash2 size={9} />
          </button>
        </div>

        {/* Row 2: role selector + capability */}
        <div className="tcf__node-foot">
          <div className="tcf__role-wrap">
            <button
              className="tcf__role-btn"
              style={{ color: roleColor }}
              onClick={() => setRoleOpen((v) => !v)}
            >
              {ROLE_LABELS[member.role]}
              <ChevronDown size={7} />
            </button>
            {roleOpen && (
              <>
                <div className="tcf__role-menu">
                  {(Object.keys(ROLE_LABELS) as MemberRole[]).map((r) => (
                    <button
                      key={r}
                      className={`tcf__role-opt ${member.role === r ? 'is-active' : ''}`}
                      style={member.role === r ? { color: ROLE_COLORS[r] } : undefined}
                      onClick={() => { onRoleChange(r); setRoleOpen(false); }}
                    >
                      {ROLE_LABELS[r]}
                    </button>
                  ))}
                </div>
                <div className="tcf__role-bd" onClick={() => setRoleOpen(false)} />
              </>
            )}
          </div>
          {primaryCap && (
            <span
              className="tcf__node-cap"
              style={{ color: CAPABILITY_COLORS[primaryCap as CapabilityCategory] }}
            >
              {primaryCap}
            </span>
          )}
          {agent?.model && (
            <span className="tcf__node-model">{agent.model}</span>
          )}
        </div>
      </div>
    </div>
  );
};

// ─── Formation View ───────────────────────────────────────────────────────────

const FormationView: React.FC<{ team: Team }> = ({ team }) => {
  const { removeMember, updateMemberRole } = useTeamStore();
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 600, h: 320 });

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ob = new ResizeObserver(() => setSize({ w: el.clientWidth, h: el.clientHeight }));
    ob.observe(el);
    setSize({ w: el.clientWidth, h: el.clientHeight });
    return () => ob.disconnect();
  }, []);

  if (team.members.length === 0) {
    return (
      <div className="tcf tcf--empty">
        <div className="tcf__empty-msg">
          <span className="tcf__empty-ico"><Bot size={24} strokeWidth={1.2} /></span>
          <p>从左侧选择 Agent 加入团队</p>
          <p className="tcf__empty-sub">点击 Agent 卡片的 + 按钮</p>
        </div>
      </div>
    );
  }

  const positions = layoutNodes(team.members);
  const edges     = buildEdges(team.members);

  const getCenter = (agentId: string) => {
    const p = positions.find((pos) => pos.memberId === agentId);
    return p ? { x: (p.x / 100) * size.w, y: (p.y / 100) * size.h } : { x: 0, y: 0 };
  };

  return (
    <div className="tcf" ref={ref}>
      {/* SVG edges */}
      <svg className="tcf__svg" width={size.w} height={size.h} aria-hidden>
        <defs>
          <marker id="tcf-arrow" markerWidth="5" markerHeight="5" refX="2.5" refY="2.5" orient="auto">
            <circle cx="2.5" cy="2.5" r="2" fill="var(--border-subtle)" />
          </marker>
        </defs>
        {edges.map(([a, b], i) => {
          const from = getCenter(a);
          const to   = getCenter(b);
          const cy   = (from.y + to.y) / 2;
          return (
            <path
              key={i}
              d={`M${from.x},${from.y} C${from.x},${cy} ${to.x},${cy} ${to.x},${to.y}`}
              fill="none"
              stroke="var(--border-subtle)"
              strokeWidth="1"
              strokeDasharray="3 3"
              markerEnd="url(#tcf-arrow)"
            />
          );
        })}
      </svg>

      {/* Nodes */}
      {team.members.map((member) => {
        const pos = positions.find((p) => p.memberId === member.agentId);
        if (!pos) return null;
        return (
          <FormationNode
            key={member.agentId}
            member={member}
            pos={pos}
            cw={size.w}
            ch={size.h}
            onRoleChange={(r) => updateMemberRole(team.id, member.agentId, r)}
            onRemove={() => removeMember(team.id, member.agentId)}
          />
        );
      })}
    </div>
  );
};

// ─── List View ────────────────────────────────────────────────────────────────

const ListView: React.FC<{ team: Team }> = ({ team }) => {
  const { removeMember, updateMemberRole } = useTeamStore();

  if (team.members.length === 0) {
    return (
      <div className="tcl tcl--empty">
        <Bot size={20} strokeWidth={1.2} style={{ color: 'var(--color-text-disabled)' }} />
        <p>暂无成员，从左侧 Agent 图鉴添加</p>
      </div>
    );
  }

  return (
    <div className="tcl">
      <table className="tcl__table">
        <thead>
          <tr>
            <th className="tcl__th">#</th>
            <th className="tcl__th">Agent</th>
            <th className="tcl__th">角色</th>
            <th className="tcl__th">工具</th>
            <th className="tcl__th">模型</th>
            <th className="tcl__th" />
          </tr>
        </thead>
        <tbody>
          {team.members.map((member, i) => {
            const agent = getAgent(member.agentId);
            const primaryCap = agent?.capabilities[0]?.category;
            return (
              <tr key={member.agentId} className="tcl__tr">
                <td className="tcl__td tcl__seq">{i + 1}</td>
                <td className="tcl__td tcl__agent">
                  <div
                    className="tcl__agent-icon"
                    style={{
                      background: primaryCap ? `${CAPABILITY_COLORS[primaryCap as CapabilityCategory]}12` : 'var(--element-bg-subtle)',
                      borderColor: primaryCap ? `${CAPABILITY_COLORS[primaryCap as CapabilityCategory]}28` : 'var(--border-subtle)',
                    }}
                  >
                    <AgentIconSmall agent={agent} />
                  </div>
                  <div className="tcl__agent-info">
                    <span className="tcl__agent-name">{agent?.name ?? member.agentId}</span>
                    <span className="tcl__agent-desc">
                      {agent?.description ? `${agent.description.slice(0, 28)}…` : ''}
                    </span>
                  </div>
                </td>
                <td className="tcl__td">
                  <select
                    className="tcl__role"
                    value={member.role}
                    onChange={(e) => updateMemberRole(team.id, member.agentId, e.target.value as MemberRole)}
                    style={{ color: ROLE_COLORS[member.role] }}
                  >
                    {(Object.keys(ROLE_LABELS) as MemberRole[]).map((r) => (
                      <option key={r} value={r}>{ROLE_LABELS[r]}</option>
                    ))}
                  </select>
                </td>
                <td className="tcl__td tcl__muted">{agent?.toolCount ?? '—'}</td>
                <td className="tcl__td tcl__muted">{member.modelOverride ?? agent?.model ?? 'primary'}</td>
                <td className="tcl__td">
                  <button
                    className="tcl__del"
                    onClick={() => removeMember(team.id, member.agentId)}
                    title="移出"
                  >
                    <Trash2 size={11} />
                  </button>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
};

// ─── Composer shell ───────────────────────────────────────────────────────────

const TeamComposer: React.FC = () => {
  const { teams, activeTeamId, viewMode, setViewMode, updateTeam } = useTeamStore();
  const [editingName, setEditingName] = useState(false);
  const [nameVal, setNameVal] = useState('');
  const nameRef = useRef<HTMLInputElement>(null);

  const team = teams.find((t) => t.id === activeTeamId);

  const startEdit = useCallback(() => {
    if (!team) return;
    setNameVal(team.name);
    setEditingName(true);
    setTimeout(() => nameRef.current?.select(), 0);
  }, [team]);

  const commitName = useCallback(() => {
    if (team && nameVal.trim()) updateTeam(team.id, { name: nameVal.trim() });
    setEditingName(false);
  }, [team, nameVal, updateTeam]);

  if (!team) {
    return (
      <div className="tc tc--empty">
        <p>请选择或新建一个团队</p>
      </div>
    );
  }

  return (
    <div className="tc">
      {/* ── Compact header bar: name + meta + view toggle ── */}
      <div className="tc__bar">
        <div className="tc__bar-left">
          {editingName ? (
            <input
              ref={nameRef}
              className="tc__name-input"
              value={nameVal}
              onChange={(e) => setNameVal(e.target.value)}
              onBlur={commitName}
              onKeyDown={(e) => {
                if (e.key === 'Enter') commitName();
                if (e.key === 'Escape') setEditingName(false);
              }}
              autoFocus
            />
          ) : (
            <span className="tc__name" onClick={startEdit} title="点击编辑">
              {team.name}
            </span>
          )}
          <span className="tc__sep">·</span>
          <span className="tc__meta">{team.members.length} 名成员</span>
          <span className="tc__meta">
            {team.strategy === 'collaborative' ? '协作' : team.strategy === 'sequential' ? '顺序' : '自由'}
          </span>
        </div>

        <div className="tc__bar-right">
          {/* Role legend */}
          <div className="tc__legend">
            {(Object.keys(ROLE_LABELS) as MemberRole[]).map((r) => (
              <span key={r} className="tc__legend-item">
                <span className="tc__legend-dot" style={{ background: ROLE_COLORS[r] }} />
                {ROLE_LABELS[r]}
              </span>
            ))}
          </div>

          <span className="tc__bar-sep" />

          {/* View toggle */}
          <div className="tc__toggle">
            <button
              className={`tc__toggle-btn ${viewMode === 'formation' ? 'is-on' : ''}`}
              onClick={() => setViewMode('formation')}
            >
              <LayoutGrid size={11} />
              阵型
            </button>
            <button
              className={`tc__toggle-btn ${viewMode === 'list' ? 'is-on' : ''}`}
              onClick={() => setViewMode('list')}
            >
              <List size={11} />
              列表
            </button>
          </div>
        </div>
      </div>

      {/* ── Body ── */}
      <div className="tc__body">
        {viewMode === 'formation' ? (
          <FormationView team={team} />
        ) : (
          <ListView team={team} />
        )}
      </div>
    </div>
  );
};

export default TeamComposer;
