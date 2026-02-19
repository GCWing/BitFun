/**
 * Workflows panel component.
 * Lists available workflows with search, grouping, and quick actions.
 */

import React, { useState, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Plus,
  Play,
  Pencil,
  ChevronDown,
  ChevronRight,
  Workflow as WorkflowIcon,
  Bot,
  Wrench,
  Puzzle,
  BookOpen,
  Mail,
  ScanSearch,
  Languages,
  FileBarChart,
  Hash,
  type LucideIcon,
} from 'lucide-react';
import { Search, Button, Switch, Tooltip } from '@/component-library';
import { PanelHeader } from '../base';
import { useNotification } from '@/shared/notification-system';
import { createWorkflowEditorTab } from '@/shared/utils/tabUtils';
import { MOCK_WORKFLOWS } from './mockData';
import type { Workflow, AgentNode } from './types';
import './WorkflowsPanel.scss';

const ICON_MAP: Record<string, LucideIcon> = {
  mail: Mail,
  'scan-search': ScanSearch,
  languages: Languages,
  'file-bar-chart': FileBarChart,
};

function getWorkflowIcon(iconName: string): LucideIcon {
  return ICON_MAP[iconName] || Hash;
}

function getTriggerLabel(wf: Workflow, t: (key: string) => string): string {
  if (wf.trigger.type === 'slash_command' && wf.trigger.command) {
    return wf.trigger.command;
  }
  if (wf.trigger.type === 'hotkey' && wf.trigger.hotkey) {
    return wf.trigger.hotkey;
  }
  return t(`trigger.${wf.trigger.type}`);
}

function countTools(agents: AgentNode[]): number {
  const all = new Set<string>();
  for (const a of agents) {
    if (a.inline) a.inline.tools.forEach((t) => all.add(t));
  }
  return all.size;
}

function countSkills(agents: AgentNode[]): number {
  const all = new Set<string>();
  for (const a of agents) {
    if (a.inline) a.inline.skills.forEach((s) => all.add(s));
  }
  return all.size;
}

const WorkflowsPanel: React.FC = () => {
  const { t } = useTranslation('panels/workflows');
  const { success: notifySuccess } = useNotification();

  const [searchQuery, setSearchQuery] = useState('');
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [userCollapsed, setUserCollapsed] = useState(false);
  const [projectCollapsed, setProjectCollapsed] = useState(false);
  const [workflows, setWorkflows] = useState<Workflow[]>(MOCK_WORKFLOWS);

  const filtered = useMemo(() => {
    if (!searchQuery.trim()) return workflows;
    const q = searchQuery.toLowerCase();
    return workflows.filter(
      (wf) =>
        wf.displayName.toLowerCase().includes(q) ||
        wf.description.toLowerCase().includes(q) ||
        wf.tags.some((tag) => tag.toLowerCase().includes(q))
    );
  }, [workflows, searchQuery]);

  const grouped = useMemo(() => {
    const user: Workflow[] = [];
    const project: Workflow[] = [];
    for (const wf of filtered) {
      if (wf.location === 'project') project.push(wf);
      else user.push(wf);
    }
    return { user, project };
  }, [filtered]);

  const toggleExpand = useCallback((id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const handleToggleEnabled = useCallback((id: string) => {
    setWorkflows((prev) =>
      prev.map((wf) => (wf.id === id ? { ...wf, enabled: !wf.enabled } : wf))
    );
  }, []);

  const handleLaunch = useCallback(
    (wf: Workflow) => {
      notifySuccess(t('notifications.launchSuccess'));
    },
    [notifySuccess, t]
  );

  const handleEdit = useCallback((wf: Workflow) => {
    createWorkflowEditorTab(wf.id, wf.displayName);
  }, []);

  const handleCreate = useCallback(() => {
    createWorkflowEditorTab();
  }, []);

  const renderCard = (wf: Workflow) => {
    const isExpanded = expandedIds.has(wf.id);
    const triggerLabel = getTriggerLabel(wf, t);
    const totalTools = countTools(wf.agents);
    const totalSkills = countSkills(wf.agents);
    const IconComponent = getWorkflowIcon(wf.icon);

    return (
      <div
        key={wf.id}
        className={`bitfun-workflows-panel__card ${!wf.enabled ? 'bitfun-workflows-panel__card--disabled' : ''}`}
      >
        <div
          className="bitfun-workflows-panel__card-main"
          onClick={() => toggleExpand(wf.id)}
        >
          <div className="bitfun-workflows-panel__card-icon">
            <IconComponent size={15} />
          </div>
          <div className="bitfun-workflows-panel__card-body">
            <div className="bitfun-workflows-panel__card-title">
              {wf.displayName}
              {!wf.enabled && (
                <span className="bitfun-workflows-panel__card-badge--disabled">
                  {t('card.disabled')}
                </span>
              )}
            </div>
            <div className="bitfun-workflows-panel__card-desc">{wf.description}</div>
            <div className="bitfun-workflows-panel__card-meta">
              <span className="bitfun-workflows-panel__card-trigger">{triggerLabel}</span>
              <span className="bitfun-workflows-panel__card-sep">/</span>
              <span>{t('card.agents', { count: wf.agents.length })}</span>
            </div>
          </div>
          <div className="bitfun-workflows-panel__card-actions" onClick={(e) => e.stopPropagation()}>
            <Tooltip content={t('actions.run')}>
              <button
                className="bitfun-workflows-panel__card-btn bitfun-workflows-panel__card-btn--run"
                onClick={() => handleLaunch(wf)}
                disabled={!wf.enabled}
              >
                <Play size={14} />
              </button>
            </Tooltip>
            <Tooltip content={t('actions.edit')}>
              <button
                className="bitfun-workflows-panel__card-btn"
                onClick={() => handleEdit(wf)}
              >
                <Pencil size={14} />
              </button>
            </Tooltip>
          </div>
        </div>

        {isExpanded && (
          <div className="bitfun-workflows-panel__card-detail">
            <div className="bitfun-workflows-panel__card-agents">
              {wf.agents.map((agent) => (
                <div key={agent.id} className="bitfun-workflows-panel__agent-row">
                  <Bot size={12} />
                  <span className="bitfun-workflows-panel__agent-name">
                    {agent.inline?.name || agent.agentRef || agent.id}
                  </span>
                  <span className="bitfun-workflows-panel__agent-role">
                    {t(`role.${agent.role}`)}
                  </span>
                  <span className="bitfun-workflows-panel__agent-model">
                    {agent.inline?.model || 'inherit'}
                  </span>
                </div>
              ))}
            </div>
            <div className="bitfun-workflows-panel__card-stats">
              <span className="bitfun-workflows-panel__stat">
                <Wrench size={11} />
                {t('card.tools', { count: totalTools })}
              </span>
              <span className="bitfun-workflows-panel__stat">
                <Puzzle size={11} />
                {t('card.skills', { count: totalSkills })}
              </span>
              <span className="bitfun-workflows-panel__stat">
                {t(`pattern.${wf.orchestration.pattern}`)}
              </span>
            </div>
            <div className="bitfun-workflows-panel__card-toggle">
              <span>{wf.enabled ? t('actions.disable') : t('actions.enable')}</span>
              <Switch
                checked={wf.enabled}
                onChange={() => handleToggleEnabled(wf.id)}
                size="small"
              />
            </div>
          </div>
        )}
      </div>
    );
  };

  const renderGroup = (
    label: string,
    items: Workflow[],
    collapsed: boolean,
    onToggle: () => void
  ) => {
    if (items.length === 0) return null;
    return (
      <div className="bitfun-workflows-panel__group">
        <div className="bitfun-workflows-panel__group-header" onClick={onToggle}>
          {collapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
          <span className="bitfun-workflows-panel__group-title">{label}</span>
          <span className="bitfun-workflows-panel__group-count">{items.length}</span>
        </div>
        {!collapsed && (
          <div className="bitfun-workflows-panel__group-content">
            {items.map(renderCard)}
          </div>
        )}
      </div>
    );
  };

  const hasResults = filtered.length > 0;

  return (
    <div className="bitfun-workflows-panel">
      <PanelHeader title={t('title')} />

      <div className="bitfun-workflows-panel__search">
        <Search
          placeholder={t('search.placeholder')}
          value={searchQuery}
          onChange={setSearchQuery}
          onClear={() => setSearchQuery('')}
          clearable
          size="small"
        />
      </div>

      <div className="bitfun-workflows-panel__create-section">
        <Button
          variant="secondary"
          size="small"
          onClick={handleCreate}
          className="bitfun-workflows-panel__create-btn"
        >
          <Plus size={16} />
          <span>{t('actions.create')}</span>
        </Button>
      </div>

      <div className="bitfun-workflows-panel__list">
        {!hasResults ? (
          <div className="bitfun-workflows-panel__empty">
            <div className="bitfun-workflows-panel__empty-icon">
              <WorkflowIcon size={48} strokeWidth={1} />
            </div>
            <p className="bitfun-workflows-panel__empty-title">{t('empty.title')}</p>
            <p className="bitfun-workflows-panel__empty-desc">{t('empty.description')}</p>
            {!searchQuery && (
              <button className="bitfun-workflows-panel__empty-btn" onClick={handleCreate}>
                {t('empty.createFirst')}
              </button>
            )}
          </div>
        ) : (
          <>
            {renderGroup(
              t('groups.user'),
              grouped.user,
              userCollapsed,
              () => setUserCollapsed((v) => !v)
            )}
            {renderGroup(
              t('groups.project'),
              grouped.project,
              projectCollapsed,
              () => setProjectCollapsed((v) => !v)
            )}
          </>
        )}
      </div>

      <div className="bitfun-workflows-panel__footer">
        <button className="bitfun-workflows-panel__template-link" onClick={() => {}}>
          <BookOpen size={14} />
          <span>{t('actions.browseTemplates')}</span>
        </button>
      </div>
    </div>
  );
};

export default WorkflowsPanel;
