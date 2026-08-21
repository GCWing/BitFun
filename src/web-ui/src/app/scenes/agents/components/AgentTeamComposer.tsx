import React, { useState, useRef, useLayoutEffect, useCallback } from 'react';
import { LayoutGrid, List, Trash2, ChevronDown, Bot, Unplug, ExternalLink } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  useAgentsStore,
  CAPABILITY_COLORS,
  type AgentTeam,
  type AgentTeamMember,
  type MemberRole,
  type MemberDisplayState,
  type AgentWithCapabilities,
  type CapabilityCategory,
} from '../agentsStore';
import { AGENT_ICON_MAP } from '../agentsIcons';
import { APPEARANCE_DOMAIN_TOKENS } from '@/infrastructure/appearance/appearanceDomainTokens';
import { computeDAGLayout } from '@/tools/bitfun-canvas/runtime/sdk/diagramLayout';
import { Badge } from '@/component-library';
import { openMainSession } from '@/flow_chat/services/sessionActivation';
import './AgentTeamComposer.scss';

// Constants

const ROLE_COLORS: Record<MemberRole, string> = {
  leader: APPEARANCE_DOMAIN_TOKENS.agentTeam.roleLeader,
  member: APPEARANCE_DOMAIN_TOKENS.agentTeam.roleMember,
  reviewer: APPEARANCE_DOMAIN_TOKENS.agentTeam.roleReviewer,
};

function getAgent(id: string): AgentWithCapabilities | undefined {
  return useAgentsStore.getState().teamComposerAgents.find((a) => a.id === id);
}

const AgentIconSmall: React.FC<{ agent?: AgentWithCapabilities }> = ({ agent }) => {
  const primaryCap = agent?.capabilities[0]?.category;
  const color = primaryCap
    ? CAPABILITY_COLORS[primaryCap as CapabilityCategory]
    : 'var(--bf-appearance-token-color-text-muted)';
  const key = (agent?.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP;
  const IconComp = AGENT_ICON_MAP[key] ?? Bot;
  return <IconComp size={13} style={{ color, flexShrink: 0 }} />;
};

// Formation layout

function edgeKey(from: string, to: string): string {
  return `${from}\u0000${to}`;
}

/** Edge fallback used when a team has no explicit edges yet (R-WF-17 data flow). */
function buildEdges(members: AgentTeamMember[]): Array<[string, string]> {
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

/** Editable member-edge map used by the formation canvas (R-WF-17). */
function editableEdges(team: AgentTeam): Array<[string, string]> {
  if (team.edges && team.edges.length > 0) {
    return team.edges.filter(([a, b]) => team.members.some((m) => m.agentId === a) && team.members.some((m) => m.agentId === b));
  }
  return buildEdges(team.members);
}

// Seven-state display mapping (R-WF-17 assertion 2) — reuses the backend
// SessionDisplayState value contract and the component-library Badge variants;
// no new state system is introduced.
const DISPLAY_STATE_BADGE: Record<MemberDisplayState, BadgeVariantLike> = {
  standby: 'neutral',
  processing: 'info',
  completed: 'success',
  hung: 'warning',
  interrupted: 'error',
  pending_attention: 'warning',
  viewed: 'neutral',
};

type BadgeVariantLike = 'neutral' | 'accent' | 'purple' | 'success' | 'warning' | 'error' | 'info';

// Formation node

const NODE_W = 176;

interface NodeProps {
  member: AgentTeamMember;
  pos: { x: number; y: number };
  onRoleChange: (r: MemberRole) => void;
  onRemove: () => void;
  onOpenSession: () => void;
  wireMode: boolean;
  onStartWire: () => void;
  onDropWire: () => void;
  onCancelWire: () => void;
}

const FormationNode: React.FC<NodeProps> = ({
  member,
  pos,
  onRoleChange,
  onRemove,
  onOpenSession,
  wireMode,
  onStartWire,
  onDropWire,
  onCancelWire,
}) => {
  const { t } = useTranslation('scenes/agents');
  const [roleOpen, setRoleOpen] = useState(false);
  const agent = getAgent(member.agentId);
  const roleColor = ROLE_COLORS[member.role];
  const primaryCap = agent?.capabilities[0]?.category;
  const roleLabels: Record<MemberRole, string> = {
    leader: t('composer.role.leader'),
    member: t('composer.role.member'),
    reviewer: t('composer.role.reviewer'),
  };
  const state = member.displayState ?? 'standby';
  // completed reuses the shared statuses.done term (same value in all three
  // locales) to avoid sharedTermDuplicates governance violations.
  const stateLabel = state === 'completed'
    ? t('shared:statuses.done')
    : t(`formation.state.${state}`);

  const nodeClick = wireMode
    ? (e: React.MouseEvent) => {
        // Interactive controls (port, delete, role, jump) never drop a wire;
        // only clicks on the node body are valid wire targets.
        const el = e.target as HTMLElement;
        if (el.closest('button')) return;
        onDropWire();
      }
    : undefined;

  return (
    <div
      className="tcf__node"
      style={{ left: pos.x, top: pos.y, width: NODE_W }}
      data-member-id={member.agentId}
      onClick={nodeClick}
    >
      <div className="tcf__node-card" style={{ borderTopColor: roleColor }}>
        {/* Row 1: name + role + delete */}
        <div className="tcf__node-head">
          <AgentIconSmall agent={agent} />
          <span className="tcf__node-name">{agent?.name ?? member.agentId}</span>
          <button className="tcf__node-del" onClick={onRemove} title={t('composer.remove')}>
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
              {roleLabels[member.role]}
              <ChevronDown size={7} />
            </button>
            {roleOpen && (
              <>
                <div className="tcf__role-menu">
                  {(Object.keys(roleLabels) as MemberRole[]).map((r) => (
                    <button
                      key={r}
                      className={`tcf__role-opt ${member.role === r ? 'is-active' : ''}`}
                      style={member.role === r ? { color: ROLE_COLORS[r] } : undefined}
                      onClick={() => { onRoleChange(r); setRoleOpen(false); }}
                    >
                      {roleLabels[r]}
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

        {/* Row 3: seven-state badge + wire port + session jump (R-WF-17) */}
        <div className="tcf__node-foot tcf__node-status">
          <Badge variant={DISPLAY_STATE_BADGE[state]} className="tcf__node-state">
            {stateLabel}
          </Badge>
          <button
            className={`tcf__node-port ${wireMode ? 'is-active' : ''}`}
            onClick={(e) => {
              e.stopPropagation();
              if (wireMode) {
                onCancelWire();
              } else {
                onStartWire();
              }
            }}
            title={wireMode ? t('formation.cancelWire') : t('formation.startWire')}
            data-bf-component="agent-team-composer"
            data-bf-part="wireStart"
            data-testid="tcf-node-port"
          >
            <Unplug size={9} />
          </button>
          <button
            className="tcf__node-jump"
            onClick={(e) => { e.stopPropagation(); onOpenSession(); }}
            title={t('formation.openSession')}
            data-bf-component="agent-team-composer"
            data-bf-part="openSession"
            data-testid="tcf-node-jump"
          >
            <ExternalLink size={9} />
          </button>
        </div>
      </div>
    </div>
  );
};

// Formation View

const FORMATION_NODE_W = 176;
const FORMATION_NODE_H = 72;

const FormationView: React.FC<{ team: AgentTeam }> = ({ team }) => {
  const { t } = useTranslation('scenes/agents');
  const { removeMember, updateMemberRole, addTeamEdge, removeTeamEdge } = useAgentsStore();
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 600, h: 320 });
  const [pendingEdge, setPendingEdge] = useState<string | null>(null);

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
          <p>{t('formation.empty')}</p>
          <p className="tcf__empty-sub">{t('formation.emptySub')}</p>
        </div>
      </div>
    );
  }

  const layoutEdges = editableEdges(team);
  // R-WF-17: delegate the real layered layout to the official computeDAGLayout
  // (nodes/edges -> rank layered x/y + edge paths). No handcrafted layout.
  const dagLayout = computeDAGLayout({
    nodes: team.members.map((m) => ({ id: m.agentId, label: m.agentId })),
    edges: layoutEdges.map(([from, to]) => ({ from, to })),
    direction: 'vertical',
    nodeWidth: FORMATION_NODE_W,
    nodeHeight: FORMATION_NODE_H,
    rankGap: 96,
    nodeGap: 72,
    padding: 16,
  });

  const nodePosById = new Map(dagLayout.nodes.map((node) => [String(node.id), node]));
  const canvasHeight = Math.max(size.h, dagLayout.height);

  const handleStartWire = (memberId: string) => setPendingEdge(memberId);
  const handleDropWire = (targetId: string) => {
    if (pendingEdge && pendingEdge !== targetId) {
      addTeamEdge(team.id, pendingEdge, targetId);
    }
    setPendingEdge(null);
  };

  return (
    <div className="tcf" data-bf-component="agent-team-composer" data-bf-part="formation" ref={ref}>
      <p className="tcf__hint">{t('formation.hint')}</p>
      {/* SVG edges: official layout.edges carry sourceX/Y + path (R-WF-17) */}
      <svg className="tcf__svg" width={size.w} height={canvasHeight} aria-hidden>
        <defs>
          <marker id="tcf-arrow" markerWidth="5" markerHeight="5" refX="2.5" refY="2.5" orient="auto">
            <circle cx="2.5" cy="2.5" r="2" fill="var(--bf-appearance-token-border-subtle)" />
          </marker>
        </defs>
        {dagLayout.edges.map((edge) => (
          <g key={edgeKey(edge.from, edge.to)}>
            <path
              d={edge.path}
              fill="none"
              stroke="var(--bf-appearance-token-border-subtle)"
              strokeWidth="1"
              strokeDasharray="3 3"
              markerEnd="url(#tcf-arrow)"
              className="tcf__edge"
            />
            <circle
              className="tcf__edge-remove"
              cx={edge.sourceX}
              cy={edge.sourceY}
              r={8}
              onClick={() => removeTeamEdge(team.id, edge.from, edge.to)}
            >
              <title>{t('formation.removeEdge')}</title>
            </circle>
          </g>
        ))}
      </svg>

      {/* Nodes */}
      {team.members.map((member) => {
        const node = nodePosById.get(member.agentId);
        if (!node) return null;
        return (
          <FormationNode
            key={member.agentId}
            member={member}
            pos={{ x: node.x, y: node.y }}
            onRoleChange={(r) => updateMemberRole(team.id, member.agentId, r)}
            onRemove={() => removeMember(team.id, member.agentId)}
            onOpenSession={() => void openMainSession(member.agentId)}
            wireMode={pendingEdge !== null}
            onStartWire={() => handleStartWire(member.agentId)}
            onDropWire={() => handleDropWire(member.agentId)}
            onCancelWire={() => setPendingEdge(null)}
          />
        );
      })}

      {pendingEdge && (
        <div className="tcf__wire" data-testid="tcf-wire-active">
          <span>{t('formation.wireActive', { from: pendingEdge })}</span>
        </div>
      )}
    </div>
  );
};

// List View

const ListView: React.FC<{ team: AgentTeam }> = ({ team }) => {
  const { t } = useTranslation('scenes/agents');
  const { removeMember, updateMemberRole } = useAgentsStore();
  const roleLabels: Record<MemberRole, string> = {
    leader: t('composer.role.leader'),
    member: t('composer.role.member'),
    reviewer: t('composer.role.reviewer'),
  };

  if (team.members.length === 0) {
    return (
      <div className="tcl tcl--empty">
        <Bot size={20} strokeWidth={1.2} style={{ color: 'var(--bf-appearance-token-color-text-disabled)' }} />
        <p>{t('composer.emptyMembers')}</p>
      </div>
    );
  }

  return (
    <div className="tcl" data-bf-component="agent-team-composer" data-bf-part="list">
      <table className="tcl__table">
        <thead>
          <tr>
            <th className="tcl__th">#</th>
            <th className="tcl__th">{t('composer.columns.agent')}</th>
            <th className="tcl__th">{t('composer.columns.role')}</th>
            <th className="tcl__th">{t('composer.columns.tools')}</th>
            <th className="tcl__th">{t('composer.columns.model')}</th>
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
                      background: primaryCap ? `${CAPABILITY_COLORS[primaryCap as CapabilityCategory]}12` : 'var(--bf-appearance-token-element-bg-subtle)',
                      borderColor: primaryCap ? `${CAPABILITY_COLORS[primaryCap as CapabilityCategory]}28` : 'var(--bf-appearance-token-border-subtle)',
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
                    {(Object.keys(roleLabels) as MemberRole[]).map((r) => (
                      <option key={r} value={r}>{roleLabels[r]}</option>
                    ))}
                  </select>
                </td>
                <td className="tcl__td tcl__muted">{agent?.toolCount ?? '—'}</td>
                <td className="tcl__td tcl__muted">{member.modelOverride ?? agent?.model ?? 'primary'}</td>
                <td className="tcl__td">
                  <button
                    className="tcl__del"
                    onClick={() => removeMember(team.id, member.agentId)}
                    title={t('composer.remove')}
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

// Composer shell

const AgentTeamComposer: React.FC = () => {
  const { t } = useTranslation('scenes/agents');
  const { agentTeams, activeAgentTeamId, viewMode, setViewMode, updateAgentTeam } = useAgentsStore();
  const [editingName, setEditingName] = useState(false);
  const [nameVal, setNameVal] = useState('');
  const nameRef = useRef<HTMLInputElement>(null);
  const roleLabels: Record<MemberRole, string> = {
    leader: t('composer.role.leader'),
    member: t('composer.role.member'),
    reviewer: t('composer.role.reviewer'),
  };

  const team = agentTeams.find((t) => t.id === activeAgentTeamId);

  const startEdit = useCallback(() => {
    if (!team) return;
    setNameVal(team.name);
    setEditingName(true);
    setTimeout(() => nameRef.current?.select(), 0);
  }, [team]);

  const commitName = useCallback(() => {
    if (team && nameVal.trim()) updateAgentTeam(team.id, { name: nameVal.trim() });
    setEditingName(false);
  }, [team, nameVal, updateAgentTeam]);

  const cancelNameEdit = useCallback(() => {
    if (team) setNameVal(team.name);
    setEditingName(false);
  }, [team]);

  if (!team) {
    return (
      <div className="tc tc--empty">
        <p>{t('composer.emptyTeam')}</p>
      </div>
    );
  }

  return (
    <div className="tc" data-bf-component="agent-team-composer" data-bf-part="root">
      {/* Compact header bar: name + meta + view toggle */}
      <div className="tc__bar" data-bf-component="agent-team-composer" data-bf-part="bar">
        <div className="tc__bar-left">
          {editingName ? (
            <>
              <input
                ref={nameRef}
                className="tc__name-input"
                data-bf-component="agent-team-composer"
                data-bf-part="name"
                value={nameVal}
                onChange={(e) => setNameVal(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') commitName();
                  if (e.key === 'Escape') cancelNameEdit();
                }}
                autoFocus
              />
              <button
                type="button"
                className="tc__edit-action tc__edit-action--save"
                data-bf-component="agent-team-composer"
                data-bf-part="renameSave"
                data-testid="tc-name-save"
                onClick={commitName}
              >
                {t('composer.saveTeam')}
              </button>
              <button
                type="button"
                className="tc__edit-action"
                data-bf-component="agent-team-composer"
                data-bf-part="renameCancel"
                data-testid="tc-name-cancel"
                onClick={cancelNameEdit}
              >
                {t('composer.cancelEdit')}
              </button>
            </>
          ) : (
            <span className="tc__name" data-bf-component="agent-team-composer" data-bf-part="name" onClick={startEdit} title={t('composer.rename')}>
              {team.name}
            </span>
          )}
          <span className="tc__sep">·</span>
          <span className="tc__meta">{t('composer.memberCount', { count: team.members.length })}</span>
          <span className="tc__meta">
            {team.strategy === 'collaborative'
              ? t('composer.strategy.collaborative')
              : team.strategy === 'sequential'
                ? t('composer.strategy.sequential')
                : t('composer.strategy.free')}
          </span>
        </div>

        <div className="tc__bar-right">
          {/* Role legend */}
          <div className="tc__legend">
            {(Object.keys(roleLabels) as MemberRole[]).map((r) => (
              <span key={r} className="tc__legend-item">
                <span className="tc__legend-dot" style={{ background: ROLE_COLORS[r] }} />
                {roleLabels[r]}
              </span>
            ))}
          </div>

          <span className="tc__bar-sep" />

          {/* View toggle */}
          <div className="tc__toggle" data-bf-component="agent-team-composer" data-bf-part="toggle">
            <button
              className={`tc__toggle-btn ${viewMode === 'formation' ? 'is-on' : ''}`}
              onClick={() => setViewMode('formation')}
            >
              <LayoutGrid size={11} />
              {t('composer.viewMode.formation')}
            </button>
            <button
              className={`tc__toggle-btn ${viewMode === 'list' ? 'is-on' : ''}`}
              onClick={() => setViewMode('list')}
            >
              <List size={11} />
              {t('composer.viewMode.list')}
            </button>
          </div>
        </div>
      </div>

      {/* Body */}
      <div className="tc__body" data-bf-component="agent-team-composer" data-bf-part="body">
        {viewMode === 'formation' ? (
          <FormationView team={team} />
        ) : (
          <ListView team={team} />
        )}
      </div>
    </div>
  );
};

export default AgentTeamComposer;
