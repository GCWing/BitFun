import React, { useMemo, useState } from 'react';
import type { TFunction } from 'i18next';
import {
  ArrowDown,
  ArrowUp,
  Pencil,
  Plus,
  Settings2,
  Trash2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  Button,
  IconButton,
  Input,
  Modal,
  Switch,
  confirmDanger,
} from '@/component-library';
import type { UserToolGroup } from '@/infrastructure/config/types';
import { useNotification } from '@/shared/notification-system';
import {
  type GroupableTool,
  type ResolvedToolGroup,
  groupToolNames,
  resolveToolGroupSummary,
  resolveToolGroups,
  setToolGroupSelection,
  toggleToolSelection,
  unavailableUserToolNames,
} from './toolGroups';
import {
  AgentCapabilityTooltip,
  type AgentCapabilityTooltipField,
} from './AgentCapabilityTooltip';
import { capabilityTooltipAriaLabel } from './agentCapabilityTooltipUtils';
import './ToolGroupPicker.scss';

interface ToolGroupPickerProps {
  tools: GroupableTool[];
  managementTools?: GroupableTool[];
  selectedToolNames: readonly string[];
  userGroups: UserToolGroup[];
  onSelectionChange: (toolNames: string[]) => void;
  onSaveUserGroups: (groups: UserToolGroup[]) => Promise<void>;
  disabled?: boolean;
  testId?: string;
}

interface ToolGroupSummaryProps {
  tools: GroupableTool[];
  selectedToolNames: readonly string[];
  userGroups: UserToolGroup[];
}

function createGroupId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `tool_group_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`;
}

function hasDuplicateName(groups: UserToolGroup[], name: string, exceptId?: string): boolean {
  const normalized = name.trim().toLocaleLowerCase();
  return groups.some((group) => (
    group.id !== exceptId && group.name.trim().toLocaleLowerCase() === normalized
  ));
}

function isGroupEnabled(group: ResolvedToolGroup, selectedToolNames: readonly string[]): boolean {
  const selected = new Set(selectedToolNames);
  return group.tools.length > 0 && group.tools.every((tool) => selected.has(tool.name));
}

function selectedGroupToolCount(group: ResolvedToolGroup, selectedToolNames: readonly string[]): number {
  const selected = new Set(selectedToolNames);
  return group.tools.filter((tool) => selected.has(tool.name)).length;
}

function groupSectionLabel(group: ResolvedToolGroup, t: TFunction<'scenes/agents'>): string {
  switch (group.kind) {
    case 'user':
      return t('agentsOverview.toolGroups.myGroups');
    case 'extension':
      return t('agentsOverview.toolGroups.extensions');
    case 'other':
      return t('agentsOverview.toolGroups.otherTools');
    default:
      return t('agentsOverview.toolGroups.builtin');
  }
}

function toolTooltipFields(
  tool: GroupableTool,
  t: TFunction<'scenes/agents'>,
): AgentCapabilityTooltipField[] {
  const mcpServerName = tool.dynamic_info?.mcp?.serverName?.trim();
  const providerId = tool.dynamic_info?.providerId?.trim();
  const provider = mcpServerName || providerId;
  const access = [
    tool.is_readonly
      ? t('agentsOverview.toolGroups.readonly')
      : t('agentsOverview.capabilityTooltip.standardPermission'),
    tool.needs_permissions ? t('agentsOverview.toolGroups.permissionRequired') : null,
  ].filter(Boolean).join(' · ');

  return [
    {
      label: t('agentsOverview.capabilityTooltip.executionPermission'),
      value: access,
    },
    ...(provider ? [{
      label: mcpServerName
        ? t('agentsOverview.capabilityTooltip.mcpServer')
        : t('agentsOverview.capabilityTooltip.provider'),
      value: provider,
      monospace: true,
    }] : []),
  ];
}

interface GroupManagerModalProps {
  isOpen: boolean;
  onClose: () => void;
  tools: GroupableTool[];
  groups: UserToolGroup[];
  onSaveGroups: (groups: UserToolGroup[]) => Promise<void>;
}

const GroupManagerModal: React.FC<GroupManagerModalProps> = ({
  isOpen,
  onClose,
  tools,
  groups,
  onSaveGroups,
}) => {
  const { t } = useTranslation('scenes/agents');
  const notification = useNotification();
  const [editingId, setEditingId] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [nameError, setNameError] = useState(false);
  const [toolNames, setToolNames] = useState<Set<string>>(new Set());
  const [saving, setSaving] = useState(false);

  const selectableTools = useMemo(
    () => [...tools].sort((left, right) => left.name.localeCompare(right.name)),
    [tools],
  );
  const editingGroup = groups.find((group) => group.id === editingId) ?? null;
  const isEditing = editingId !== null;

  const closeEditor = () => {
    setEditingId(null);
    setName('');
    setNameError(false);
    setToolNames(new Set());
  };

  const startCreate = () => {
    setEditingId('__new__');
    setName('');
    setNameError(false);
    setToolNames(new Set());
  };

  const startEdit = (group: UserToolGroup) => {
    setEditingId(group.id);
    setName(group.name);
    setNameError(false);
    setToolNames(new Set(group.toolNames));
  };

  const toggleTool = (toolName: string) => {
    setToolNames((current) => {
      const next = new Set(current);
      if (next.has(toolName)) {
        next.delete(toolName);
      } else {
        next.add(toolName);
      }
      return next;
    });
  };

  const saveEditor = async () => {
    const trimmedName = name.trim();
    const selectedNames = Array.from(toolNames);
    const existingId = editingGroup?.id;
    if (!trimmedName) {
      setNameError(true);
      return;
    }
    if (hasDuplicateName(groups, trimmedName, existingId)) {
      notification.error(t('agentsOverview.toolGroups.validation.nameDuplicate'));
      return;
    }
    if (selectedNames.length === 0) {
      notification.error(t('agentsOverview.toolGroups.validation.toolsRequired'));
      return;
    }

    const nextGroup: UserToolGroup = {
      id: existingId ?? createGroupId(),
      name: trimmedName,
      toolNames: selectedNames,
    };
    const nextGroups = existingId
      ? groups.map((group) => group.id === existingId ? nextGroup : group)
      : [...groups, nextGroup];

    setSaving(true);
    try {
      await onSaveGroups(nextGroups);
      closeEditor();
    } catch {
      notification.error(t('agentsOverview.toolGroups.saveFailed'));
    } finally {
      setSaving(false);
    }
  };

  const deleteGroup = async (group: UserToolGroup) => {
    const confirmed = await confirmDanger(
      t('agentsOverview.toolGroups.deleteTitle'),
      t('agentsOverview.toolGroups.deleteMessage', { name: group.name }),
      { confirmText: t('agentsOverview.toolGroups.deleteConfirm') },
    );
    if (!confirmed) {
      return;
    }
    setSaving(true);
    try {
      await onSaveGroups(groups.filter((candidate) => candidate.id !== group.id));
      if (editingId === group.id) {
        closeEditor();
      }
    } catch {
      notification.error(t('agentsOverview.toolGroups.saveFailed'));
    } finally {
      setSaving(false);
    }
  };

  const moveGroup = async (index: number, direction: -1 | 1) => {
    const nextIndex = index + direction;
    if (nextIndex < 0 || nextIndex >= groups.length) {
      return;
    }
    const nextGroups = [...groups];
    [nextGroups[index], nextGroups[nextIndex]] = [nextGroups[nextIndex], nextGroups[index]];
    setSaving(true);
    try {
      await onSaveGroups(nextGroups);
    } catch {
      notification.error(t('agentsOverview.toolGroups.saveFailed'));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={() => {
        closeEditor();
        onClose();
      }}
      title={t('agentsOverview.toolGroups.manageTitle')}
      size="large"
      contentInset
      testId="tool-group-manager"
    >
      <div className="tool-group-manager">
        {isEditing ? (
          <div className="tool-group-manager__editor">
            <div className="tool-group-manager__field">
              <label htmlFor="tool-group-name">{t('agentsOverview.toolGroups.groupName')}</label>
              <Input
                id="tool-group-name"
                value={name}
                onChange={(event) => {
                  setName(event.target.value);
                  if (nameError) {
                    setNameError(false);
                  }
                }}
                placeholder={t('agentsOverview.toolGroups.groupNamePlaceholder')}
                inputSize="small"
                error={nameError}
                disabled={saving}
              />
            </div>
            <div className="tool-group-manager__field">
              <span>{t('agentsOverview.toolGroups.groupTools')}</span>
              <div className="tool-group-manager__token-grid">
                {selectableTools.map((tool) => {
                  const selected = toolNames.has(tool.name);
                  const tooltipFields = toolTooltipFields(tool, t);
                  return (
                    <AgentCapabilityTooltip
                      key={tool.name}
                      title={tool.name}
                      description={tool.description}
                      fields={tooltipFields}
                      titleMonospace
                      placement="top"
                    >
                      <button
                        type="button"
                        className={`tool-group-manager__token${selected ? ' is-on' : ''}`}
                        onClick={() => toggleTool(tool.name)}
                        disabled={saving}
                        aria-label={capabilityTooltipAriaLabel(tool.name, tool.description, tooltipFields)}
                        aria-pressed={selected}
                      >
                        {tool.name}
                      </button>
                    </AgentCapabilityTooltip>
                  );
                })}
              </div>
            </div>
            <div className="tool-group-manager__footer">
              <Button variant="ghost" size="small" onClick={closeEditor} disabled={saving}>
                {t('agentsOverview.cancel')}
              </Button>
              <Button variant="primary" size="small" onClick={() => void saveEditor()} isLoading={saving}>
                {isEditing && editingGroup
                  ? t('agentsOverview.toolGroups.saveGroup')
                  : t('agentsOverview.toolGroups.createGroup')}
              </Button>
            </div>
          </div>
        ) : (
          <>
            <div className="tool-group-manager__head">
              <span>{t('agentsOverview.toolGroups.manageSubtitle')}</span>
              <Button variant="secondary" size="small" onClick={startCreate} disabled={saving}>
                <Plus size={14} />
                {t('agentsOverview.toolGroups.createGroup')}
              </Button>
            </div>
            {groups.length === 0 ? (
              <p className="tool-group-manager__empty">{t('agentsOverview.toolGroups.noUserGroups')}</p>
            ) : (
              <div className="tool-group-manager__list">
                {groups.map((group, index) => {
                  const unavailable = unavailableUserToolNames(group, tools);
                  return (
                    <div key={group.id} className="tool-group-manager__group-row">
                      <div className="tool-group-manager__group-copy">
                        <span className="tool-group-manager__group-name">{group.name}</span>
                        <span className="tool-group-manager__group-meta">
                          {t('agentsOverview.toolGroups.groupCount', { count: group.toolNames.length })}
                          {unavailable.length > 0
                            ? ` · ${t('agentsOverview.toolGroups.unavailableCount', { count: unavailable.length })}`
                            : ''}
                        </span>
                      </div>
                      <div className="tool-group-manager__group-actions">
                        <IconButton
                          type="button"
                          size="small"
                          variant="ghost"
                          aria-label={t('agentsOverview.toolGroups.moveUp')}
                          tooltip={t('agentsOverview.toolGroups.moveUp')}
                          onClick={() => void moveGroup(index, -1)}
                          disabled={saving || index === 0}
                        >
                          <ArrowUp size={13} />
                        </IconButton>
                        <IconButton
                          type="button"
                          size="small"
                          variant="ghost"
                          aria-label={t('agentsOverview.toolGroups.moveDown')}
                          tooltip={t('agentsOverview.toolGroups.moveDown')}
                          onClick={() => void moveGroup(index, 1)}
                          disabled={saving || index === groups.length - 1}
                        >
                          <ArrowDown size={13} />
                        </IconButton>
                        <IconButton
                          type="button"
                          size="small"
                          variant="ghost"
                          aria-label={t('agentsOverview.toolGroups.editGroup')}
                          tooltip={t('agentsOverview.toolGroups.editGroup')}
                          onClick={() => startEdit(group)}
                          disabled={saving}
                        >
                          <Pencil size={13} />
                        </IconButton>
                        <IconButton
                          type="button"
                          size="small"
                          variant="ghost"
                          aria-label={t('agentsOverview.toolGroups.deleteGroup')}
                          tooltip={t('agentsOverview.toolGroups.deleteGroup')}
                          onClick={() => void deleteGroup(group)}
                          disabled={saving}
                        >
                          <Trash2 size={13} />
                        </IconButton>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </>
        )}
      </div>
    </Modal>
  );
};

export const ToolGroupPicker: React.FC<ToolGroupPickerProps> = ({
  tools,
  managementTools,
  selectedToolNames,
  userGroups,
  onSelectionChange,
  onSaveUserGroups,
  disabled = false,
  testId,
}) => {
  const { t } = useTranslation('scenes/agents');
  const [isManagerOpen, setIsManagerOpen] = useState(false);
  const groups = useMemo(() => resolveToolGroups(tools, userGroups, t), [t, tools, userGroups]);
  const selectedCount = new Set(selectedToolNames).size;
  const sections = useMemo(() => {
    const grouped = new Map<string, ResolvedToolGroup[]>();
    for (const group of groups) {
      const label = groupSectionLabel(group, t);
      const entries = grouped.get(label) ?? [];
      entries.push(group);
      grouped.set(label, entries);
    }
    return [...grouped.entries()];
  }, [groups, t]);

  return (
    <div className="tool-group-picker" data-testid={testId}>
      <div className="tool-group-picker__head">
        <span className="tool-group-picker__selected-count">
          {t('agentsOverview.toolGroups.selectedCount', { count: selectedCount })}
        </span>
        <Button
          variant="ghost"
          size="small"
          onClick={() => setIsManagerOpen(true)}
          disabled={disabled}
        >
          <Settings2 size={14} />
          {t('agentsOverview.toolGroups.manageGroups')}
        </Button>
      </div>
      <div className="tool-group-picker__sections">
        {sections.map(([sectionLabel, sectionGroups]) => (
          <section key={sectionLabel} className="tool-group-picker__section">
            <span className="tool-group-picker__section-label">{sectionLabel}</span>
            {sectionGroups.map((group) => {
              const selectedInGroup = selectedGroupToolCount(group, selectedToolNames);
              const allSelected = isGroupEnabled(group, selectedToolNames);
              return (
                <div key={group.id} className="tool-group-picker__group">
                  <div className="tool-group-picker__group-head">
                    <div className="tool-group-picker__group-title-wrap">
                      <span className="tool-group-picker__group-name">{group.label}</span>
                      <span className="tool-group-picker__group-count">
                        {selectedInGroup}/{group.tools.length}
                      </span>
                    </div>
                    <div className="tool-group-picker__group-actions">
                      {selectedInGroup > 0 && !allSelected ? (
                        <Button
                          variant="ghost"
                          size="small"
                          onClick={() => onSelectionChange(
                            setToolGroupSelection(selectedToolNames, groupToolNames(group), false),
                          )}
                          disabled={disabled}
                        >
                          {t('agentsOverview.clearGroup')}
                        </Button>
                      ) : null}
                      <Switch
                        size="small"
                        checked={allSelected}
                        onChange={(event) => onSelectionChange(
                          setToolGroupSelection(
                            selectedToolNames,
                            groupToolNames(group),
                            event.target.checked,
                          ),
                        )}
                        disabled={disabled}
                        aria-label={allSelected
                          ? t('agentsOverview.toolGroups.clearGroupTools', { name: group.label })
                          : t('agentsOverview.toolGroups.enableGroupTools', { name: group.label })}
                      />
                    </div>
                  </div>
                  <div className="tool-group-picker__token-grid">
                    {group.tools.map((tool) => {
                      const selected = selectedToolNames.includes(tool.name);
                      const tooltipFields = toolTooltipFields(tool, t);
                      return (
                        <AgentCapabilityTooltip
                          key={tool.name}
                          title={tool.name}
                          description={tool.description}
                          fields={tooltipFields}
                          titleMonospace
                          placement="top"
                        >
                          <button
                            type="button"
                            className={`tool-group-picker__token${selected ? ' is-on' : ''}`}
                            onClick={() => onSelectionChange(
                              toggleToolSelection(selectedToolNames, tool.name),
                            )}
                            disabled={disabled}
                            aria-label={capabilityTooltipAriaLabel(tool.name, tool.description, tooltipFields)}
                          >
                            {tool.name}
                          </button>
                        </AgentCapabilityTooltip>
                      );
                    })}
                  </div>
                </div>
              );
            })}
          </section>
        ))}
      </div>
      <GroupManagerModal
        isOpen={isManagerOpen}
        onClose={() => setIsManagerOpen(false)}
        tools={managementTools ?? tools}
        groups={userGroups}
        onSaveGroups={onSaveUserGroups}
      />
    </div>
  );
};

export const ToolGroupSummary: React.FC<ToolGroupSummaryProps> = ({
  tools,
  selectedToolNames,
  userGroups,
}) => {
  const { t } = useTranslation('scenes/agents');
  const groups = useMemo(
    () => resolveToolGroupSummary(tools, userGroups, selectedToolNames, t),
    [selectedToolNames, t, tools, userGroups],
  );

  if (groups.length === 0) {
    return <span className="agent-card__empty-inline">{t('agentsOverview.toolGroups.noEnabledTools')}</span>;
  }

  return (
    <div className="tool-group-summary">
      {groups.map((group) => (
        <div key={group.id} className="tool-group-summary__group">
          <span className="tool-group-summary__label">{group.label}</span>
          <div className="tool-group-summary__tools">
            {group.tools.map((tool) => {
              const tooltipFields = toolTooltipFields(tool, t);
              return (
                <AgentCapabilityTooltip
                  key={tool.name}
                  title={tool.name}
                  description={tool.description}
                  fields={tooltipFields}
                  titleMonospace
                >
                  <span className="agent-card__chip">
                    {tool.name.replace(/_/g, ' ')}
                  </span>
                </AgentCapabilityTooltip>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
};
