import React, { useState, useCallback, useEffect } from 'react';
import { Search, ChevronDown, ChevronUp, Plus, Check, Bot } from 'lucide-react';
import { SubagentAPI } from '@/infrastructure/api/service-api/SubagentAPI';
import {
  useTeamStore,
  MOCK_AGENTS,
  CAPABILITY_CATEGORIES,
  CAPABILITY_COLORS,
  type AgentWithCapabilities,
  type CapabilityCategory,
} from '../teamStore';
import { AGENT_ICON_MAP } from '../teamIcons';
import { isBuiltinSubAgent } from '@/infrastructure/agents/constants';
import './AgentGallery.scss';

// ─── Agent icon ───────────────────────────────────────────────────────────────

const AgentIcon: React.FC<{ iconKey?: string; primaryCap?: string; size?: number }> = ({
  iconKey,
  primaryCap,
  size = 14,
}) => {
  const color = primaryCap ? CAPABILITY_COLORS[primaryCap as CapabilityCategory] : 'var(--color-text-muted)';
  const key = (iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP;
  const IconComp = AGENT_ICON_MAP[key] ?? Bot;
  return <IconComp size={size} style={{ color, flexShrink: 0 }} />;
};

// ─── Capability bars ──────────────────────────────────────────────────────────

const CapBars: React.FC<{ caps: AgentWithCapabilities['capabilities'] }> = ({ caps }) => (
  <div className="ag-cap-bars">
    {caps.map((c) => (
      <div key={c.category} className="ag-cap-bar">
        <span className="ag-cap-label">{c.category}</span>
        <div className="ag-cap-track">
          {Array.from({ length: 5 }, (_, i) => (
            <span
              key={i}
              className="ag-cap-seg"
              style={i < c.level
                ? { background: CAPABILITY_COLORS[c.category as CapabilityCategory] }
                : undefined}
            />
          ))}
        </div>
        <span className="ag-cap-level">{c.level}/5</span>
      </div>
    ))}
  </div>
);

// ─── Agent card ───────────────────────────────────────────────────────────────

interface AgentCardProps {
  agent: AgentWithCapabilities;
  isMember: boolean;
  onAdd: () => void;
  onRemove: () => void;
}

const AgentCard: React.FC<AgentCardProps> = ({ agent, isMember, onAdd, onRemove }) => {
  const [expanded, setExpanded] = useState(false);
  const primaryCap = agent.capabilities[0]?.category;

  return (
    <div className={`ag-card ${isMember ? 'is-member' : ''} ${!agent.enabled ? 'is-disabled' : ''}`}>
      {/* ── Summary row ── */}
      <div className="ag-card__row" onClick={() => setExpanded((v) => !v)}>
        {/* Icon cell */}
        <div
          className="ag-card__icon"
          style={{
            background: primaryCap
              ? `${CAPABILITY_COLORS[primaryCap as CapabilityCategory]}14`
              : 'var(--element-bg-subtle)',
            borderColor: primaryCap
              ? `${CAPABILITY_COLORS[primaryCap as CapabilityCategory]}30`
              : 'var(--border-subtle)',
          }}
        >
          <AgentIcon iconKey={agent.iconKey} primaryCap={primaryCap} size={14} />
        </div>

        {/* Meta */}
        <div className="ag-card__meta">
          <span className="ag-card__name">{agent.name}</span>
          <span className="ag-card__desc">{agent.description}</span>
          <div className="ag-card__name-row">
            {!agent.enabled && <span className="ag-card__badge ag-card__badge--dim">已禁用</span>}
            {agent.capabilities.slice(0, 3).map((c) => (
              <span
                key={c.category}
                className="ag-card__badge"
                style={{
                  color: CAPABILITY_COLORS[c.category as CapabilityCategory],
                  borderColor: `${CAPABILITY_COLORS[c.category as CapabilityCategory]}44`,
                }}
              >
                {c.category}
              </span>
            ))}
          </div>
        </div>

        {/* Actions */}
        <div className="ag-card__actions" onClick={(e) => e.stopPropagation()}>
          <button
            className={`ag-card__add ${isMember ? 'is-added' : ''}`}
            onClick={isMember ? onRemove : onAdd}
            title={isMember ? '移出团队' : '加入团队'}
          >
            {isMember ? <Check size={11} /> : <Plus size={11} />}
          </button>
        </div>

        <span className="ag-card__chevron">
          {expanded ? <ChevronUp size={11} /> : <ChevronDown size={11} />}
        </span>
      </div>

      {/* ── Expanded detail ── */}
      {expanded && (
        <div className="ag-card__detail">
          <CapBars caps={agent.capabilities} />
          <div className="ag-card__detail-meta">
            <span>{agent.toolCount} 个工具</span>
            {agent.model && <span>模型 · {agent.model}</span>}
            <span>{agent.subagentSource === 'builtin' ? (isBuiltinSubAgent(agent.id) ? 'Sub-Agent' : '内置') : agent.subagentSource === 'user' ? '用户' : '项目'}</span>
          </div>
          <button
            className={`ag-card__add-full ${isMember ? 'is-added' : ''}`}
            onClick={isMember ? onRemove : onAdd}
          >
            {isMember ? '已加入团队' : '加入当前团队'}
          </button>
        </div>
      )}
    </div>
  );
};

// ─── Gallery ──────────────────────────────────────────────────────────────────

function enrichAgent(agent: AgentWithCapabilities): AgentWithCapabilities {
  if (agent.capabilities?.length) return agent;
  const name = agent.name.toLowerCase();
  if (name.includes('code') || name.includes('debug') || name.includes('test')) {
    return { ...agent, capabilities: [{ category: '编码', level: 4 }] };
  }
  if (name.includes('doc') || name.includes('write')) {
    return { ...agent, capabilities: [{ category: '文档', level: 4 }] };
  }
  return { ...agent, capabilities: [{ category: '分析', level: 3 }] };
}

const AgentGallery: React.FC = () => {
  const { teams, activeTeamId, addMember, removeMember } = useTeamStore();
  const [agents, setAgents] = useState<AgentWithCapabilities[]>(MOCK_AGENTS);
  const [query, setQuery] = useState('');
  const [activeCategories, setActiveCategories] = useState<Set<CapabilityCategory>>(new Set());
  const [showMembersOnly, setShowMembersOnly] = useState(false);

  const activeTeam = teams.find((t) => t.id === activeTeamId);
  const memberIds = new Set(activeTeam?.members.map((m) => m.agentId) ?? []);

  useEffect(() => {
    SubagentAPI.listSubagents()
      .then((list) => {
        const enriched = list.map((a) => enrichAgent(a as AgentWithCapabilities));
        if (enriched.length > 0) {
          const realIds = new Set(enriched.map((a) => a.id));
          const mockOnly = MOCK_AGENTS.filter((a) => !realIds.has(a.id));
          setAgents([...enriched, ...mockOnly]);
        }
      })
      .catch(() => { /* keep mock */ });
  }, []);

  const toggleCategory = useCallback((cat: CapabilityCategory) => {
    setActiveCategories((prev) => {
      const next = new Set(prev);
      if (next.has(cat)) next.delete(cat);
      else next.add(cat);
      return next;
    });
  }, []);

  const filtered = agents.filter((a) => {
    if (showMembersOnly && !memberIds.has(a.id)) return false;
    if (query) {
      const q = query.toLowerCase();
      if (!a.name.toLowerCase().includes(q) && !a.description.toLowerCase().includes(q)) return false;
    }
    if (activeCategories.size > 0) {
      const agentCats = new Set(a.capabilities.map((c) => c.category));
      if (![...activeCategories].some((c) => agentCats.has(c))) return false;
    }
    return true;
  });

  const categoryCounts = CAPABILITY_CATEGORIES.reduce<Record<string, number>>((acc, cat) => {
    acc[cat] = agents.filter((a) => a.capabilities.some((c) => c.category === cat)).length;
    return acc;
  }, {});

  return (
    <div className="ag">
      {/* ── Search bar ── */}
      <div className="ag__search-bar">
        <Search size={12} className="ag__search-ico" />
        <input
          className="ag__search-input"
          placeholder="搜索 Agent 名称、能力..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>

      {/* ── Filter pills ── */}
      <div className="ag__filters">
        <button
          className={`ag__pill ${showMembersOnly ? 'is-active' : ''}`}
          onClick={() => setShowMembersOnly((v) => !v)}
        >
          已加入{memberIds.size > 0 && <span className="ag__pill-n">{memberIds.size}</span>}
        </button>
        {CAPABILITY_CATEGORIES.map((cat) => (
          <button
            key={cat}
            className={`ag__pill ${activeCategories.has(cat) ? 'is-active' : ''}`}
            style={
              activeCategories.has(cat)
                ? { color: CAPABILITY_COLORS[cat], borderColor: `${CAPABILITY_COLORS[cat]}55` }
                : undefined
            }
            onClick={() => toggleCategory(cat)}
          >
            {cat}
            <span className="ag__pill-n">{categoryCounts[cat]}</span>
          </button>
        ))}
      </div>

      {/* ── List ── */}
      <div className="ag__list">
        {filtered.length === 0 ? (
          <div className="ag__empty">暂无匹配结果</div>
        ) : (
          filtered.map((agent) => (
            <AgentCard
              key={agent.id}
              agent={agent}
              isMember={memberIds.has(agent.id)}
              onAdd={() => activeTeamId && addMember(activeTeamId, agent.id)}
              onRemove={() => activeTeamId && removeMember(activeTeamId, agent.id)}
            />
          ))
        )}
      </div>

      {/* ── Footer ── */}
      <div className="ag__footer">
        {filtered.length} / {agents.length} · {agents.filter((a) => a.enabled).length} 已启用
      </div>
    </div>
  );
};

export default AgentGallery;
