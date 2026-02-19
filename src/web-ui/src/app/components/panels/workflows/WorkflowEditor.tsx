/**
 * Workflow editor component.
 * Multi-step form for creating and editing workflows.
 * Rendered inside the right panel's ContentCanvas.
 */

import React, { useState, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Bot,
  Plus,
  Trash2,
  Wrench,
  Puzzle,
  ChevronDown,
  ChevronRight,
  Workflow as WorkflowIcon,
  User,
  ArrowRight,
  Zap,
  Crown,
  Users,
  Eye,
  Mail,
  ScanSearch,
  Languages,
  FileBarChart,
  Hash,
  Terminal,
  Globe,
  FileCode,
  GitBranch,
  Settings,
  type LucideIcon,
} from 'lucide-react';
import {
  Button,
  Input,
  Textarea,
  Checkbox,
  Tooltip,
  Select,
  Tag,
} from '@/component-library';
import type {
  AgentNode,
  AgentNodeConfig,
  WorkflowEditorStep,
  AgentRole,
  OrchestrationPattern,
  TriggerType,
  WorkflowLocation,
} from './types';
import { MOCK_WORKFLOWS } from './mockData';
import './WorkflowEditor.scss';

interface WorkflowEditorProps {
  workflowId?: string;
}

const DEFAULT_AGENT: AgentNodeConfig = {
  name: '',
  description: '',
  prompt: '',
  model: 'primary',
  tools: [],
  skills: [],
  readonly: true,
};

const STEPS: WorkflowEditorStep[] = ['basic', 'agents', 'orchestration', 'preview'];

const AVAILABLE_TOOLS = [
  { group: 'Built-in', items: ['Read', 'Write', 'Edit', 'Bash', 'Grep', 'Glob', 'WebSearch', 'ReadLints', 'Git', 'TodoWrite'] },
  { group: 'Gmail (MCP)', items: ['mcp_gmail_send_email', 'mcp_gmail_read_email', 'mcp_gmail_search', 'mcp_gmail_create_draft'] },
  { group: 'Calendar (MCP)', items: ['mcp_calendar_get_events', 'mcp_calendar_create_event'] },
];

const AVAILABLE_SKILLS = [
  { name: 'email-writing', label: 'email-writing' },
  { name: 'code-review', label: 'code-review' },
  { name: 'git-commit', label: 'git-commit' },
  { name: 'translation-glossary', label: 'translation-glossary' },
];

const ICON_MAP: Record<string, LucideIcon> = {
  mail: Mail,
  'scan-search': ScanSearch,
  languages: Languages,
  'file-bar-chart': FileBarChart,
  terminal: Terminal,
  globe: Globe,
  'file-code': FileCode,
  'git-branch': GitBranch,
  settings: Settings,
  workflow: WorkflowIcon,
};

const ICON_OPTIONS = [
  'workflow', 'mail', 'scan-search', 'languages', 'file-bar-chart',
  'terminal', 'globe', 'file-code', 'git-branch', 'settings',
];

function getIconComponent(iconName: string): LucideIcon {
  return ICON_MAP[iconName] || Hash;
}

const ROLE_ICONS: Record<AgentRole, LucideIcon> = {
  orchestrator: Crown,
  worker: Bot,
  reviewer: Eye,
};

const PATTERN_ICONS: Record<OrchestrationPattern, LucideIcon> = {
  single: User,
  pipeline: ArrowRight,
  fan_out: Zap,
  supervisor: Crown,
  team: Users,
};

const WorkflowEditor: React.FC<WorkflowEditorProps> = ({ workflowId }) => {
  const { t } = useTranslation('panels/workflows');
  const isNew = !workflowId;

  const existingWorkflow = useMemo(
    () => (workflowId ? MOCK_WORKFLOWS.find((w) => w.id === workflowId) : undefined),
    [workflowId]
  );

  const [step, setStep] = useState<WorkflowEditorStep>('basic');
  const [editingAgentId, setEditingAgentId] = useState<string | null>(null);

  const [name, setName] = useState(existingWorkflow?.name || '');
  const [displayName, setDisplayName] = useState(existingWorkflow?.displayName || '');
  const [description, setDescription] = useState(existingWorkflow?.description || '');
  const [icon, setIcon] = useState(existingWorkflow?.icon || 'workflow');
  const [triggerType, setTriggerType] = useState<TriggerType>(existingWorkflow?.trigger.type || 'manual');
  const [triggerCommand, setTriggerCommand] = useState(existingWorkflow?.trigger.command || '');
  const [triggerHotkey, setTriggerHotkey] = useState(existingWorkflow?.trigger.hotkey || '');
  const [location, setLocation] = useState<WorkflowLocation>(existingWorkflow?.location || 'user');
  const [tags, setTags] = useState<string[]>(existingWorkflow?.tags || []);
  const [tagInput, setTagInput] = useState('');

  const [agents, setAgents] = useState<AgentNode[]>(
    existingWorkflow?.agents || []
  );
  const [pattern, setPattern] = useState<OrchestrationPattern>(
    existingWorkflow?.orchestration.pattern || 'single'
  );
  const [supervisorAgentId, setSupervisorAgentId] = useState(
    existingWorkflow?.orchestration.supervisor?.agentId || ''
  );

  const currentStepIndex = STEPS.indexOf(step);
  const IconComp = getIconComponent(icon);

  const goNext = () => {
    const idx = STEPS.indexOf(step);
    if (idx < STEPS.length - 1) setStep(STEPS[idx + 1]);
  };
  const goPrev = () => {
    const idx = STEPS.indexOf(step);
    if (idx > 0) setStep(STEPS[idx - 1]);
  };

  const addAgent = useCallback(() => {
    const id = `agent-${Date.now()}`;
    setAgents((prev) => [
      ...prev,
      { id, role: 'worker' as AgentRole, inline: { ...DEFAULT_AGENT } },
    ]);
    setEditingAgentId(id);
  }, []);

  const removeAgent = useCallback((id: string) => {
    setAgents((prev) => prev.filter((a) => a.id !== id));
    setEditingAgentId(null);
  }, []);

  const updateAgent = useCallback((id: string, updates: Partial<AgentNode>) => {
    setAgents((prev) =>
      prev.map((a) => (a.id === id ? { ...a, ...updates } : a))
    );
  }, []);

  const updateAgentConfig = useCallback((id: string, updates: Partial<AgentNodeConfig>) => {
    setAgents((prev) =>
      prev.map((a) =>
        a.id === id && a.inline
          ? { ...a, inline: { ...a.inline, ...updates } }
          : a
      )
    );
  }, []);

  const toggleTool = useCallback((agentId: string, tool: string) => {
    setAgents((prev) =>
      prev.map((a) => {
        if (a.id !== agentId || !a.inline) return a;
        const tools = a.inline.tools.includes(tool)
          ? a.inline.tools.filter((t) => t !== tool)
          : [...a.inline.tools, tool];
        return { ...a, inline: { ...a.inline, tools } };
      })
    );
  }, []);

  const toggleSkill = useCallback((agentId: string, skill: string) => {
    setAgents((prev) =>
      prev.map((a) => {
        if (a.id !== agentId || !a.inline) return a;
        const skills = a.inline.skills.includes(skill)
          ? a.inline.skills.filter((s) => s !== skill)
          : [...a.inline.skills, skill];
        return { ...a, inline: { ...a.inline, skills } };
      })
    );
  }, []);

  const handleTagKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'Enter' && tagInput.trim()) {
        e.preventDefault();
        if (!tags.includes(tagInput.trim())) {
          setTags((prev) => [...prev, tagInput.trim()]);
        }
        setTagInput('');
      }
    },
    [tagInput, tags]
  );

  const removeTag = useCallback((tag: string) => {
    setTags((prev) => prev.filter((t) => t !== tag));
  }, []);

  // ==================== Step Renderers ====================

  const renderBasicStep = () => (
    <div className="bitfun-wf-editor__form">
      <div className="bitfun-wf-editor__field">
        <label>{t('editor.basic.displayName')}</label>
        <Input
          value={displayName}
          onChange={(e) => setDisplayName(e.target.value)}
          placeholder={t('editor.basic.displayNamePlaceholder')}
          inputSize="small"
        />
      </div>
      <div className="bitfun-wf-editor__field">
        <label>{t('editor.basic.name')}</label>
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder={t('editor.basic.namePlaceholder')}
          inputSize="small"
        />
      </div>
      <div className="bitfun-wf-editor__field">
        <label>{t('editor.basic.description')}</label>
        <Textarea
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder={t('editor.basic.descriptionPlaceholder')}
          rows={3}
        />
      </div>
      <div className="bitfun-wf-editor__field bitfun-wf-editor__field--row">
        <label>{t('editor.basic.icon')}</label>
        <div className="bitfun-wf-editor__icon-picker">
          {ICON_OPTIONS.map((iconKey) => {
            const Ic = getIconComponent(iconKey);
            return (
              <button
                key={iconKey}
                className={`bitfun-wf-editor__icon-option ${icon === iconKey ? 'is-active' : ''}`}
                onClick={() => setIcon(iconKey)}
              >
                <Ic size={15} />
              </button>
            );
          })}
        </div>
      </div>
      <div className="bitfun-wf-editor__field">
        <label>{t('editor.basic.tags')}</label>
        <div className="bitfun-wf-editor__tags">
          {tags.map((tag) => (
            <Tag key={tag} size="small" color="gray" closable onClose={() => removeTag(tag)}>
              {tag}
            </Tag>
          ))}
          <Input
            value={tagInput}
            onChange={(e) => setTagInput(e.target.value)}
            onKeyDown={handleTagKeyDown}
            placeholder={tags.length === 0 ? t('editor.basic.tagsPlaceholder') : ''}
            inputSize="small"
            className="bitfun-wf-editor__tag-input-wrapper"
          />
        </div>
      </div>

      <div className="bitfun-wf-editor__section-title">{t('editor.basic.trigger')}</div>
      <Select
        value={triggerType}
        onChange={(val) => setTriggerType(val as TriggerType)}
        size="small"
        options={[
          { value: 'manual', label: t('trigger.manual') },
          { value: 'slash_command', label: t('trigger.slash_command') },
          { value: 'hotkey', label: t('trigger.hotkey') },
        ]}
      />
      {triggerType === 'slash_command' && (
        <div className="bitfun-wf-editor__field">
          <label>{t('editor.basic.triggerCommand')}</label>
          <Input
            value={triggerCommand}
            onChange={(e) => setTriggerCommand(e.target.value)}
            placeholder={t('editor.basic.triggerCommandPlaceholder')}
            inputSize="small"
          />
        </div>
      )}
      {triggerType === 'hotkey' && (
        <div className="bitfun-wf-editor__field">
          <label>{t('editor.basic.triggerHotkey')}</label>
          <Input
            value={triggerHotkey}
            onChange={(e) => setTriggerHotkey(e.target.value)}
            placeholder={t('editor.basic.triggerHotkeyPlaceholder')}
            inputSize="small"
          />
        </div>
      )}

      <div className="bitfun-wf-editor__section-title">{t('editor.basic.location')}</div>
      <Select
        value={location}
        onChange={(val) => setLocation(val as WorkflowLocation)}
        size="small"
        options={[
          { value: 'user', label: t('editor.basic.locationUser') },
          { value: 'project', label: t('editor.basic.locationProject') },
        ]}
      />
    </div>
  );

  const renderAgentEditor = (agent: AgentNode) => {
    if (!agent.inline) return null;
    const config = agent.inline;

    return (
      <div className="bitfun-wf-editor__agent-editor">
        <div className="bitfun-wf-editor__field">
          <label>{t('editor.agent.name')}</label>
          <Input
            value={config.name}
            onChange={(e) => updateAgentConfig(agent.id, { name: e.target.value })}
            placeholder={t('editor.agent.namePlaceholder')}
            inputSize="small"
          />
        </div>
        <div className="bitfun-wf-editor__field">
          <label>{t('editor.agent.role')}</label>
          <div className="bitfun-wf-editor__role-grid">
            {(['orchestrator', 'worker', 'reviewer'] as AgentRole[]).map((r) => {
              const RoleIcon = ROLE_ICONS[r];
              return (
                <button
                  key={r}
                  className={`bitfun-wf-editor__role-card ${agent.role === r ? 'is-active' : ''}`}
                  onClick={() => updateAgent(agent.id, { role: r })}
                >
                  <span className="bitfun-wf-editor__role-card-icon">
                    <RoleIcon size={16} />
                  </span>
                  <span className="bitfun-wf-editor__role-card-label">{t(`role.${r}`)}</span>
                  <span className="bitfun-wf-editor__role-card-desc">{t(`roleDesc.${r}`)}</span>
                </button>
              );
            })}
          </div>
        </div>
        <div className="bitfun-wf-editor__field">
          <label>{t('editor.agent.model')}</label>
          <Select
            value={config.model}
            onChange={(val) => updateAgentConfig(agent.id, { model: val as string })}
            size="small"
            options={[
              { value: 'primary', label: 'Primary' },
              { value: 'fast', label: 'Fast' },
              { value: 'inherit', label: 'Inherit' },
            ]}
          />
        </div>
        <div className="bitfun-wf-editor__field">
          <label>{t('editor.agent.prompt')}</label>
          <Textarea
            value={config.prompt}
            onChange={(e) => updateAgentConfig(agent.id, { prompt: e.target.value })}
            placeholder={t('editor.agent.promptPlaceholder')}
            rows={5}
          />
        </div>

        <div className="bitfun-wf-editor__field">
          <label>{t('editor.agent.tools')}</label>
          <div className="bitfun-wf-editor__checklist">
            {AVAILABLE_TOOLS.map((group) => (
              <div key={group.group} className="bitfun-wf-editor__checklist-group">
                <div className="bitfun-wf-editor__checklist-group-label">{group.group}</div>
                <div className="bitfun-wf-editor__checklist-items">
                  {group.items.map((tool) => (
                    <label key={tool} className="bitfun-wf-editor__check-item">
                      <Checkbox
                        checked={config.tools.includes(tool)}
                        onChange={() => toggleTool(agent.id, tool)}
                      />
                      <span>{tool}</span>
                    </label>
                  ))}
                </div>
              </div>
            ))}
          </div>
          <span className="bitfun-wf-editor__hint">
            {t('editor.agent.selectedTools', { count: config.tools.length })}
          </span>
        </div>

        <div className="bitfun-wf-editor__field">
          <label>{t('editor.agent.skills')}</label>
          <div className="bitfun-wf-editor__checklist-items">
            {AVAILABLE_SKILLS.map((skill) => (
              <label key={skill.name} className="bitfun-wf-editor__check-item">
                <Checkbox
                  checked={config.skills.includes(skill.name)}
                  onChange={() => toggleSkill(agent.id, skill.name)}
                />
                <span>{skill.label}</span>
              </label>
            ))}
          </div>
          <span className="bitfun-wf-editor__hint">
            {t('editor.agent.selectedSkills', { count: config.skills.length })}
          </span>
        </div>

        <div className="bitfun-wf-editor__field bitfun-wf-editor__field--row">
          <Checkbox
            checked={config.readonly}
            onChange={(e) => updateAgentConfig(agent.id, { readonly: e.target.checked })}
          />
          <span>{t('editor.agent.readonly')}</span>
        </div>

        <div className="bitfun-wf-editor__agent-editor-actions">
          <Button size="small" variant="secondary" onClick={() => setEditingAgentId(null)}>
            {t('actions.saveAgent')}
          </Button>
        </div>
      </div>
    );
  };

  const renderAgentsStep = () => (
    <div className="bitfun-wf-editor__agents-step">
      <div className="bitfun-wf-editor__agents-header">
        <span>{t('editor.steps.agents')}</span>
        <Button size="small" variant="secondary" onClick={addAgent}>
          <Plus size={14} />
          <span>{t('actions.addAgent')}</span>
        </Button>
      </div>
      <div className="bitfun-wf-editor__agents-list">
        {agents.map((agent) => (
          <div key={agent.id} className="bitfun-wf-editor__agent-card">
            <div
              className="bitfun-wf-editor__agent-card-header"
              onClick={() =>
                setEditingAgentId(editingAgentId === agent.id ? null : agent.id)
              }
            >
              <Bot size={14} />
              <span className="bitfun-wf-editor__agent-card-name">
                {agent.inline?.name || agent.id}
              </span>
              <span className="bitfun-wf-editor__agent-card-role">
                {t(`role.${agent.role}`)}
              </span>
              <span className="bitfun-wf-editor__agent-card-model">
                {agent.inline?.model || 'inherit'}
              </span>
              <div className="bitfun-wf-editor__agent-card-info">
                <Wrench size={11} />
                <span>{agent.inline?.tools.length || 0}</span>
                <Puzzle size={11} />
                <span>{agent.inline?.skills.length || 0}</span>
              </div>
              <Tooltip content={t('actions.removeAgent')}>
                <button
                  className="bitfun-wf-editor__agent-remove"
                  onClick={(e) => {
                    e.stopPropagation();
                    removeAgent(agent.id);
                  }}
                >
                  <Trash2 size={13} />
                </button>
              </Tooltip>
              {editingAgentId === agent.id ? (
                <ChevronDown size={14} />
              ) : (
                <ChevronRight size={14} />
              )}
            </div>
            {editingAgentId === agent.id && renderAgentEditor(agent)}
          </div>
        ))}
        {agents.length === 0 && (
          <div className="bitfun-wf-editor__agents-empty">
            <Bot size={32} strokeWidth={1} />
            <p>{t('actions.addAgent')}</p>
          </div>
        )}
      </div>
    </div>
  );

  const renderOrchestrationStep = () => {
    const patterns: OrchestrationPattern[] = ['single', 'pipeline', 'fan_out', 'supervisor', 'team'];

    return (
      <div className="bitfun-wf-editor__orchestration-step">
        <p className="bitfun-wf-editor__section-desc">{t('editor.orchestration.title')}</p>
        <div className="bitfun-wf-editor__pattern-grid">
          {patterns.map((p) => {
            const PatternIcon = PATTERN_ICONS[p];
            return (
              <button
                key={p}
                className={`bitfun-wf-editor__pattern-card ${pattern === p ? 'is-active' : ''}`}
                onClick={() => setPattern(p)}
              >
                <span className="bitfun-wf-editor__pattern-icon">
                  <PatternIcon size={20} />
                </span>
                <span className="bitfun-wf-editor__pattern-label">{t(`pattern.${p}`)}</span>
                <span className="bitfun-wf-editor__pattern-desc">{t(`patternDesc.${p}`)}</span>
              </button>
            );
          })}
        </div>

        {pattern === 'supervisor' && agents.length > 0 && (
          <div className="bitfun-wf-editor__field">
            <label>{t('editor.orchestration.supervisorAgent')}</label>
            <div className="bitfun-wf-editor__supervisor-picker">
              {agents.map((a) => {
                const RoleIcon = ROLE_ICONS[a.role];
                const isSelected = supervisorAgentId === a.id;
                return (
                  <button
                    key={a.id}
                    className={`bitfun-wf-editor__supervisor-option ${isSelected ? 'is-active' : ''}`}
                    onClick={() => setSupervisorAgentId(a.id)}
                  >
                    <span className="bitfun-wf-editor__supervisor-option-icon">
                      <RoleIcon size={14} />
                    </span>
                    <div className="bitfun-wf-editor__supervisor-option-body">
                      <span className="bitfun-wf-editor__supervisor-option-name">
                        {a.inline?.name || a.id}
                      </span>
                      <span className="bitfun-wf-editor__supervisor-option-role">
                        {t(`role.${a.role}`)}
                      </span>
                    </div>
                    {isSelected && (
                      <span className="bitfun-wf-editor__supervisor-option-badge">
                        {t('role.orchestrator')}
                      </span>
                    )}
                  </button>
                );
              })}
            </div>
          </div>
        )}

        <div className="bitfun-wf-editor__section-title">{t('editor.orchestration.topologyPreview')}</div>
        <div className={`bitfun-wf-editor__topology bitfun-wf-editor__topology--${pattern}`}>
          {pattern === 'single' && agents.length > 0 && (
            <div className="bitfun-wf-editor__topo-single">
              <div className="bitfun-wf-editor__topo-node bitfun-wf-editor__topo-node--primary">
                <Bot size={14} />
                <span>{agents[0]?.inline?.name || agents[0]?.id}</span>
              </div>
            </div>
          )}

          {pattern === 'pipeline' && (
            <div className="bitfun-wf-editor__topo-pipeline">
              {agents.map((a, i) => (
                <React.Fragment key={a.id}>
                  <div className="bitfun-wf-editor__topo-node">
                    <Bot size={12} />
                    <span>{a.inline?.name || a.id}</span>
                    <span className="bitfun-wf-editor__topo-role">{t(`role.${a.role}`)}</span>
                  </div>
                  {i < agents.length - 1 && (
                    <div className="bitfun-wf-editor__topo-arrow bitfun-wf-editor__topo-arrow--down">
                      <svg width="12" height="20" viewBox="0 0 12 20">
                        <line x1="6" y1="0" x2="6" y2="14" stroke="currentColor" strokeWidth="1" strokeDasharray="3 2" />
                        <path d="M2 12 L6 18 L10 12" fill="none" stroke="currentColor" strokeWidth="1" />
                      </svg>
                    </div>
                  )}
                </React.Fragment>
              ))}
            </div>
          )}

          {pattern === 'fan_out' && (
            <div className="bitfun-wf-editor__topo-fanout">
              <div className="bitfun-wf-editor__topo-node bitfun-wf-editor__topo-node--io">{t('editor.topology.input')}</div>
              <div className="bitfun-wf-editor__topo-arrow bitfun-wf-editor__topo-arrow--fan">
                <svg width="40" height="12" viewBox="0 0 40 12">
                  <line x1="0" y1="6" x2="34" y2="6" stroke="currentColor" strokeWidth="1" strokeDasharray="3 2" />
                  <path d="M32 2 L38 6 L32 10" fill="none" stroke="currentColor" strokeWidth="1" />
                </svg>
              </div>
              <div className="bitfun-wf-editor__topo-parallel">
                {agents.map((a) => (
                  <div key={a.id} className="bitfun-wf-editor__topo-node">
                    <Bot size={12} />
                    <span>{a.inline?.name || a.id}</span>
                  </div>
                ))}
              </div>
              <div className="bitfun-wf-editor__topo-arrow bitfun-wf-editor__topo-arrow--fan">
                <svg width="40" height="12" viewBox="0 0 40 12">
                  <line x1="0" y1="6" x2="34" y2="6" stroke="currentColor" strokeWidth="1" strokeDasharray="3 2" />
                  <path d="M32 2 L38 6 L32 10" fill="none" stroke="currentColor" strokeWidth="1" />
                </svg>
              </div>
              <div className="bitfun-wf-editor__topo-node bitfun-wf-editor__topo-node--io">{t('editor.topology.output')}</div>
            </div>
          )}

          {pattern === 'supervisor' && (
            <div className="bitfun-wf-editor__topo-supervisor">
              {(() => {
                const sup = agents.find((a) => a.id === supervisorAgentId) || agents.find((a) => a.role === 'orchestrator');
                const workers = agents.filter((a) => a.id !== sup?.id);
                return (
                  <>
                    <div className="bitfun-wf-editor__topo-node bitfun-wf-editor__topo-node--primary">
                      <Crown size={14} />
                      <span>{sup?.inline?.name || t('role.orchestrator')}</span>
                    </div>
                    {workers.length > 0 && (
                      <>
                        <div className="bitfun-wf-editor__topo-branch-lines">
                          <svg width="100%" height="20" preserveAspectRatio="none">
                            <line x1="50%" y1="0" x2="50%" y2="20" stroke="currentColor" strokeWidth="1" strokeDasharray="3 2" />
                          </svg>
                        </div>
                        <div className="bitfun-wf-editor__topo-workers">
                          {workers.map((w) => (
                            <div key={w.id} className="bitfun-wf-editor__topo-node">
                              <Bot size={12} />
                              <span>{w.inline?.name || w.id}</span>
                            </div>
                          ))}
                        </div>
                      </>
                    )}
                  </>
                );
              })()}
            </div>
          )}

          {pattern === 'team' && (
            <div className="bitfun-wf-editor__topo-team">
              {agents.map((a, i) => (
                <React.Fragment key={a.id}>
                  <div className="bitfun-wf-editor__topo-node">
                    <Bot size={12} />
                    <span>{a.inline?.name || a.id}</span>
                  </div>
                  {i < agents.length - 1 && (
                    <div className="bitfun-wf-editor__topo-arrow bitfun-wf-editor__topo-arrow--bi">
                      <svg width="28" height="12" viewBox="0 0 28 12">
                        <path d="M4 2 L0 6 L4 10" fill="none" stroke="currentColor" strokeWidth="1" />
                        <line x1="2" y1="6" x2="26" y2="6" stroke="currentColor" strokeWidth="1" strokeDasharray="3 2" />
                        <path d="M24 2 L28 6 L24 10" fill="none" stroke="currentColor" strokeWidth="1" />
                      </svg>
                    </div>
                  )}
                </React.Fragment>
              ))}
            </div>
          )}

          {agents.length === 0 && (
            <div className="bitfun-wf-editor__topo-empty">
              <span>{t('editor.topology.empty')}</span>
            </div>
          )}
        </div>
      </div>
    );
  };

  const renderPreviewStep = () => {
    const PatternIcon = PATTERN_ICONS[pattern];
    return (
      <div className="bitfun-wf-editor__preview-step">
        <div className="bitfun-wf-editor__preview-header">
          <div className="bitfun-wf-editor__preview-header-icon">
            <IconComp size={18} />
          </div>
          <div>
            <h3>{displayName || name || '(untitled)'}</h3>
            <p>{description}</p>
          </div>
        </div>

        <div className="bitfun-wf-editor__preview-info">
          <div className="bitfun-wf-editor__preview-row">
            <span className="bitfun-wf-editor__preview-label">{t('editor.preview.trigger')}</span>
            <span className="bitfun-wf-editor__preview-value">
              {triggerType === 'slash_command'
                ? triggerCommand
                : triggerType === 'hotkey'
                  ? triggerHotkey
                  : t('trigger.manual')}
            </span>
          </div>
          <div className="bitfun-wf-editor__preview-row">
            <span className="bitfun-wf-editor__preview-label">{t('editor.preview.pattern')}</span>
            <span className="bitfun-wf-editor__preview-value">
              <PatternIcon size={13} /> {t(`pattern.${pattern}`)}
            </span>
          </div>
          <div className="bitfun-wf-editor__preview-row">
            <span className="bitfun-wf-editor__preview-label">{t('editor.preview.location')}</span>
            <span className="bitfun-wf-editor__preview-value">
              {location === 'user' ? t('editor.basic.locationUser') : t('editor.basic.locationProject')}
            </span>
          </div>
        </div>

        <div className="bitfun-wf-editor__section-title">{t('editor.preview.agentSummary')}</div>
        <div className="bitfun-wf-editor__preview-agents">
          {agents.map((a) => {
            const RoleIcon = ROLE_ICONS[a.role];
            return (
              <div key={a.id} className="bitfun-wf-editor__preview-agent-card">
                <div className="bitfun-wf-editor__preview-agent-card-header">
                  <span className="bitfun-wf-editor__preview-agent-card-icon">
                    <RoleIcon size={14} />
                  </span>
                  <span className="bitfun-wf-editor__preview-agent-card-name">
                    {a.inline?.name || a.id}
                  </span>
                  <span className="bitfun-wf-editor__preview-agent-card-role">
                    {t(`role.${a.role}`)}
                  </span>
                </div>
                <div className="bitfun-wf-editor__preview-agent-card-meta">
                  <span className="bitfun-wf-editor__preview-agent-card-model">
                    {a.inline?.model || 'inherit'}
                  </span>
                  <span className="bitfun-wf-editor__preview-agent-card-stat">
                    <Wrench size={11} /> {a.inline?.tools.length || 0}
                  </span>
                  <span className="bitfun-wf-editor__preview-agent-card-stat">
                    <Puzzle size={11} /> {a.inline?.skills.length || 0}
                  </span>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    );
  };

  return (
    <div className="bitfun-wf-editor">
      <div className="bitfun-wf-editor__header">
        <div className="bitfun-wf-editor__header-icon">
          <IconComp size={16} />
        </div>
        <span>{isNew ? t('editor.createTitle') : `${t('editor.title')}: ${displayName || name}`}</span>
      </div>

      <div className="bitfun-wf-editor__steps">
        {STEPS.map((s, i) => (
          <button
            key={s}
            className={`bitfun-wf-editor__step ${step === s ? 'is-active' : ''} ${i < currentStepIndex ? 'is-done' : ''}`}
            onClick={() => setStep(s)}
          >
            <span className="bitfun-wf-editor__step-num">{i + 1}</span>
            <span className="bitfun-wf-editor__step-label">{t(`editor.steps.${s}`)}</span>
          </button>
        ))}
      </div>

      <div className="bitfun-wf-editor__content">
        {step === 'basic' && renderBasicStep()}
        {step === 'agents' && renderAgentsStep()}
        {step === 'orchestration' && renderOrchestrationStep()}
        {step === 'preview' && renderPreviewStep()}
      </div>

      <div className="bitfun-wf-editor__footer">
        <Button size="small" variant="ghost" onClick={() => {}}>
          {t('actions.cancel')}
        </Button>
        <div className="bitfun-wf-editor__footer-right">
          {currentStepIndex > 0 && (
            <Button size="small" variant="secondary" onClick={goPrev}>
              {t('actions.prevStep')}
            </Button>
          )}
          {currentStepIndex < STEPS.length - 1 ? (
            <Button size="small" variant="primary" onClick={goNext}>
              {t('actions.nextStep')}
            </Button>
          ) : (
            <Button size="small" variant="primary" onClick={() => {}}>
              {t('actions.save')}
            </Button>
          )}
        </div>
      </div>
    </div>
  );
};

export default WorkflowEditor;
