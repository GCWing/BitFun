import React, { useState, useCallback } from 'react';
import { Search, ChevronDown, ChevronUp, Plus, Check, Bot, Cpu } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/component-library';
import {
  useAgentsStore,
  CAPABILITY_CATEGORIES,
  CAPABILITY_COLORS,
  type AgentWithCapabilities,
  type CapabilityCategory,
} from '../agentsStore';
import { AGENT_ICON_MAP } from '../agentsIcons';
import { getAgentBadge } from '../utils';
import './AgentGallery.scss';

// Agent icon

const AgentIcon: React.FC<{ iconKey?: string; primaryCap?: string; size?: number }> = ({
  iconKey,
  primaryCap,
  size = 14,
}) => {
  const color = primaryCap ? CAPABILITY_COLORS[primaryCap as CapabilityCategory] : 'var(--bf-appearance-token-color-text-muted)';
  const key = (iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP;
  const IconComp = AGENT_ICON_MAP[key] ?? Bot;
  return <IconComp size={size} style={{ color, flexShrink: 0 }} />;
};

// Capability bars

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

// Agent card

interface AgentCardProps {
  agent: AgentWithCapabilities;
  isMember: boolean;
  onAdd: () => void;
  onRemove: () => void;
}

const AgentCard: React.FC<AgentCardProps> = ({ agent, isMember, onAdd, onRemove }) => {
  const { t } = useTranslation('scenes/agents');
  const [expanded, setExpanded] = useState(false);
  const primaryCap = agent.capabilities[0]?.category;
  const badge = getAgentBadge(t, agent.agentKind, agent.source ?? agent.subagentSource, agent.id);
  const isDisabled = agent.effectiveEnabled === false;

  return (
    <div className={`ag-card ${isMember ? 'is-member' : ''} ${isDisabled ? 'is-disabled' : ''}`}>
      {/* Summary row */}
      <div className="ag-card__row" onClick={() => setExpanded((v) => !v)}>
        {/* Icon cell */}
        <div
          className="ag-card__icon"
          style={{
            background: primaryCap
              ? `${CAPABILITY_COLORS[primaryCap as CapabilityCategory]}14`
              : 'var(--bf-appearance-token-element-bg-subtle)',
            borderColor: primaryCap
              ? `${CAPABILITY_COLORS[primaryCap as CapabilityCategory]}30`
              : 'var(--bf-appearance-token-border-subtle)',
          }}
        >
          <AgentIcon iconKey={agent.iconKey} primaryCap={primaryCap} size={14} />
        </div>

        {/* Meta */}
        <div className="ag-card__meta">
          <span className="ag-card__name">{agent.name}</span>
          <span className="ag-card__desc">{agent.description}</span>
          <div className="ag-card__name-row">
            {isDisabled && <span className="ag-card__badge ag-card__badge--dim">{t('agentCard.badges.disabled')}</span>}
            {/* Agent kind badge */}
            <Badge variant={badge.variant}>
              {agent.agentKind === 'mode' ? <Cpu size={9} /> : <Bot size={9} />}
              {badge.label}
            </Badge>
            {/* Capability chips */}
            {agent.capabilities.slice(0, 2).map((c) => (
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
            title={isMember ? t('gallery.removeFromTeam') : t('gallery.addToTeam')}
          >
            {isMember ? <Check size={11} /> : <Plus size={11} />}
          </button>
        </div>

        <span className="ag-card__chevron">
          {expanded ? <ChevronUp size={11} /> : <ChevronDown size={11} />}
        </span>
      </div>

      {/* Expanded detail */}
      {expanded && (
        <div className="ag-card__detail">
          <CapBars caps={agent.capabilities} />
          <div className="ag-card__detail-meta">
            <span>{t('gallery.toolCount', { count: agent.toolCount })}</span>
            {agent.model && <span>{t('gallery.modelLabel')} · {agent.model}</span>}
            <Badge variant={badge.variant}>
              {agent.agentKind === 'mode' ? <Cpu size={9} /> : <Bot size={9} />}
              {badge.label}
            </Badge>
          </div>
          <button
            className={`ag-card__add-full ${isMember ? 'is-added' : ''}`}
            onClick={isMember ? onRemove : onAdd}
          >
            {isMember ? t('gallery.joinedTeam') : t('gallery.addCurrentTeam')}
          </button>
        </div>
      )}
    </div>
  );
};

// Gallery

const AgentGallery: React.FC = () => {
  const { t } = useTranslation('scenes/agents');
  const { agentTeams, activeAgentTeamId, addMember, removeMember, teamComposerAgents } = useAgentsStore();
  const [query, setQuery] = useState('');
  const [activeCategories, setActiveCategories] = useState<Set<CapabilityCategory>>(new Set());
  const [showMembersOnly, setShowMembersOnly] = useState(false);

  const activeTeam = agentTeams.find((t) => t.id === activeAgentTeamId);
  const memberIds = new Set(activeTeam?.members.map((m) => m.agentId) ?? []);

  const agents = teamComposerAgents;

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
    <div className="ag" data-bf-component="agent-gallery" data-bf-part="root">
      {/* Search bar */}
      <div className="ag__search-bar" data-bf-component="agent-gallery" data-bf-part="searchBar">
        <Search size={12} className="ag__search-ico" />
        <input
          className="ag__search-input"
          placeholder={t('gallery.search')}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>

      {/* Filter pills */}
      <div className="ag__filters" data-bf-component="agent-gallery" data-bf-part="filters">
        <button
          className={`ag__pill ${showMembersOnly ? 'is-active' : ''}`}
          onClick={() => setShowMembersOnly((v) => !v)}
        >
          {t('gallery.filter.joined')}
          {memberIds.size > 0 && <span className="ag__pill-n">{memberIds.size}</span>}
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

      {/* List */}
      <div className="ag__list" data-bf-component="agent-gallery" data-bf-part="list">
        {filtered.length === 0 ? (
          <div className="ag__empty">{t('gallery.empty')}</div>
        ) : (
          filtered.map((agent) => (
            <AgentCard
              key={agent.id}
              agent={agent}
              isMember={memberIds.has(agent.id)}
              onAdd={() => activeAgentTeamId && addMember(activeAgentTeamId, agent.id)}
              onRemove={() => activeAgentTeamId && removeMember(activeAgentTeamId, agent.id)}
            />
          ))
        )}
      </div>

      {/* Footer */}
      <div className="ag__footer" data-bf-component="agent-gallery" data-bf-part="footer">
        {t('gallery.footer', {
          shown: filtered.length,
          total: agents.length,
          enabled: agents.filter((a) => a.effectiveEnabled !== false).length,
        })}
      </div>
    </div>
  );
};

export default AgentGallery;
