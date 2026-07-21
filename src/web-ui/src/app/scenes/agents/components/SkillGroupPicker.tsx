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
import type { UserSkillGroup } from '@/infrastructure/config/types';
import { useNotification } from '@/shared/notification-system';
import {
  type GroupableSkill,
  type ResolvedSkillGroup,
  builtinSkillGroupLabelKey,
  resolveSkillGroupSummary,
  resolveSkillGroups,
  setSkillGroupSelection,
  skillGroupKeys,
  toggleSkillSelection,
  unavailableUserSkillKeys,
} from './skillGroups';
import {
  AgentCapabilityTooltip,
  type AgentCapabilityTooltipField,
} from './AgentCapabilityTooltip';
import { capabilityTooltipAriaLabel } from './agentCapabilityTooltipUtils';
import './SkillGroupPicker.scss';

interface SkillGroupPickerProps {
  skills: GroupableSkill[];
  managementSkills?: GroupableSkill[];
  selectedSkillKeys: readonly string[];
  userGroups: UserSkillGroup[];
  onSelectionChange: (skillKeys: string[]) => void;
  onSaveUserGroups: (groups: UserSkillGroup[]) => Promise<void>;
  disabled?: boolean;
  testId?: string;
}

interface SkillGroupSummaryProps {
  skills: GroupableSkill[];
  selectedSkillKeys: readonly string[];
  userGroups: UserSkillGroup[];
}

export interface SkillGroupManagerModalProps {
  isOpen: boolean;
  onClose: () => void;
  skills: GroupableSkill[];
  groups: UserSkillGroup[];
  onSaveGroups: (groups: UserSkillGroup[]) => Promise<void>;
}

function createGroupId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `skill_group_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`;
}

function hasDuplicateName(groups: UserSkillGroup[], name: string, exceptId?: string): boolean {
  const normalized = name.trim().toLocaleLowerCase();
  return groups.some((group) => (
    group.id !== exceptId && group.name.trim().toLocaleLowerCase() === normalized
  ));
}

function isGroupEnabled(group: ResolvedSkillGroup, selectedSkillKeys: readonly string[]): boolean {
  const selected = new Set(selectedSkillKeys);
  return group.skills.length > 0 && group.skills.every((skill) => selected.has(skill.key));
}

function selectedGroupSkillCount(group: ResolvedSkillGroup, selectedSkillKeys: readonly string[]): number {
  const selected = new Set(selectedSkillKeys);
  return group.skills.filter((skill) => selected.has(skill.key)).length;
}

function builtinGroupLabel(groupKey: string, t: TFunction<'scenes/agents'>): string {
  const labelKey = builtinSkillGroupLabelKey(groupKey);
  return labelKey ? t(`agentsOverview.skillGroups.${labelKey}`) : groupKey;
}

function groupSectionLabel(group: ResolvedSkillGroup, t: TFunction<'scenes/agents'>): string {
  switch (group.kind) {
    case 'user':
      return t('agentsOverview.skillGroupPicker.myGroups');
    case 'builtin':
      return t('agentsOverview.skillGroupPicker.builtin');
    default:
      return t('agentsOverview.skillGroupPicker.otherSkills');
  }
}

function duplicateSkillNames(skills: GroupableSkill[]): Set<string> {
  const counts = new Map<string, number>();
  for (const skill of skills) {
    counts.set(skill.name, (counts.get(skill.name) ?? 0) + 1);
  }
  return new Set([...counts].flatMap(([name, count]) => count > 1 ? [name] : []));
}

function skillDisplayName(skill: GroupableSkill, duplicateNames: Set<string>): string {
  if (!duplicateNames.has(skill.name)) {
    return skill.name;
  }
  const source = [skill.sourceLabel ?? skill.sourceSlot, skill.level].filter(Boolean).join('/');
  return source ? `${skill.name} [${source}]` : `${skill.name} [${skill.key}]`;
}

function skillTooltipFields(
  skill: GroupableSkill,
  t: TFunction<'scenes/agents'>,
): AgentCapabilityTooltipField[] {
  const source = [skill.sourceLabel ?? skill.sourceSlot, skill.level].filter(Boolean).join('/');
  return [
    {
      label: t('agentsOverview.capabilityTooltip.skillKey'),
      value: skill.key,
      monospace: true,
    },
    ...(source ? [{
      label: t('agentsOverview.capabilityTooltip.sourceLevel'),
      value: source,
      monospace: true,
    }] : []),
    ...(skill.runtimeStatus ? [{
      label: t('agentsOverview.capabilityTooltip.status'),
      value: skill.runtimeStatus,
    }] : skill.isShadowed ? [{
      label: t('agentsOverview.capabilityTooltip.status'),
      value: t('agentsOverview.skillShadowed'),
    }] : []),
  ];
}

export const SkillGroupManagerModal: React.FC<SkillGroupManagerModalProps> = ({
  isOpen,
  onClose,
  skills,
  groups,
  onSaveGroups,
}) => {
  const { t } = useTranslation('scenes/agents');
  const notification = useNotification();
  const [editingId, setEditingId] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [nameError, setNameError] = useState(false);
  const [skillKeys, setSkillKeys] = useState<Set<string>>(new Set());
  const [saving, setSaving] = useState(false);
  const duplicateNames = useMemo(() => duplicateSkillNames(skills), [skills]);
  const selectableSkills = useMemo(
    () => [...skills].sort((left, right) => left.name.localeCompare(right.name) || left.key.localeCompare(right.key)),
    [skills],
  );
  const editingGroup = groups.find((group) => group.id === editingId) ?? null;
  const isEditing = editingId !== null;

  const closeEditor = () => {
    setEditingId(null);
    setName('');
    setNameError(false);
    setSkillKeys(new Set());
  };

  const startCreate = () => {
    setEditingId('__new__');
    setName('');
    setNameError(false);
    setSkillKeys(new Set());
  };

  const startEdit = (group: UserSkillGroup) => {
    setEditingId(group.id);
    setName(group.name);
    setNameError(false);
    setSkillKeys(new Set(group.skillKeys));
  };

  const toggleSkill = (skillKey: string) => {
    setSkillKeys((current) => {
      const next = new Set(current);
      if (next.has(skillKey)) {
        next.delete(skillKey);
      } else {
        next.add(skillKey);
      }
      return next;
    });
  };

  const saveEditor = async () => {
    const trimmedName = name.trim();
    const selectedKeys = Array.from(skillKeys);
    const existingId = editingGroup?.id;
    if (!trimmedName) {
      setNameError(true);
      return;
    }
    if (hasDuplicateName(groups, trimmedName, existingId)) {
      notification.error(t('agentsOverview.skillGroupPicker.validation.nameDuplicate'));
      return;
    }
    if (selectedKeys.length === 0) {
      notification.error(t('agentsOverview.skillGroupPicker.validation.skillsRequired'));
      return;
    }

    const nextGroup: UserSkillGroup = {
      id: existingId ?? createGroupId(),
      name: trimmedName,
      skillKeys: selectedKeys,
    };
    const nextGroups = existingId
      ? groups.map((group) => group.id === existingId ? nextGroup : group)
      : [...groups, nextGroup];

    setSaving(true);
    try {
      await onSaveGroups(nextGroups);
      closeEditor();
    } catch {
      notification.error(t('agentsOverview.skillGroupPicker.saveFailed'));
    } finally {
      setSaving(false);
    }
  };

  const deleteGroup = async (group: UserSkillGroup) => {
    const confirmed = await confirmDanger(
      t('agentsOverview.skillGroupPicker.deleteTitle'),
      t('agentsOverview.skillGroupPicker.deleteMessage', { name: group.name }),
      { confirmText: t('agentsOverview.skillGroupPicker.deleteConfirm') },
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
      notification.error(t('agentsOverview.skillGroupPicker.saveFailed'));
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
      notification.error(t('agentsOverview.skillGroupPicker.saveFailed'));
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
      title={t('agentsOverview.skillGroupPicker.manageTitle')}
      size="large"
      contentInset
      testId="skill-group-manager"
    >
      <div className="skill-group-manager">
        {isEditing ? (
          <div className="skill-group-manager__editor">
            <div className="skill-group-manager__field">
              <label htmlFor="skill-group-name">{t('agentsOverview.skillGroupPicker.groupName')}</label>
              <Input
                id="skill-group-name"
                value={name}
                onChange={(event) => {
                  setName(event.target.value);
                  if (nameError) {
                    setNameError(false);
                  }
                }}
                placeholder={t('agentsOverview.skillGroupPicker.groupNamePlaceholder')}
                inputSize="small"
                error={nameError}
                disabled={saving}
              />
            </div>
            <div className="skill-group-manager__field">
              <span>{t('agentsOverview.skillGroupPicker.groupSkills')}</span>
              <div className="skill-group-manager__token-grid">
                {selectableSkills.map((skill) => {
                  const selected = skillKeys.has(skill.key);
                  const tooltipFields = skillTooltipFields(skill, t);
                  return (
                    <AgentCapabilityTooltip
                      key={skill.key}
                      title={skillDisplayName(skill, duplicateNames)}
                      description={skill.description}
                      fields={tooltipFields}
                      placement="top"
                    >
                      <button
                        type="button"
                        className={`skill-group-manager__token${selected ? ' is-on' : ''}`}
                        onClick={() => toggleSkill(skill.key)}
                        disabled={saving}
                        aria-label={capabilityTooltipAriaLabel(
                          skillDisplayName(skill, duplicateNames),
                          skill.description,
                          tooltipFields,
                        )}
                        aria-pressed={selected}
                      >
                        {skillDisplayName(skill, duplicateNames)}
                      </button>
                    </AgentCapabilityTooltip>
                  );
                })}
              </div>
            </div>
            <div className="skill-group-manager__footer">
              <Button variant="ghost" size="small" onClick={closeEditor} disabled={saving}>
                {t('agentsOverview.cancel')}
              </Button>
              <Button variant="primary" size="small" onClick={() => void saveEditor()} isLoading={saving}>
                {isEditing && editingGroup
                  ? t('agentsOverview.skillGroupPicker.saveGroup')
                  : t('agentsOverview.skillGroupPicker.createGroup')}
              </Button>
            </div>
          </div>
        ) : (
          <>
            <div className="skill-group-manager__head">
              <span>{t('agentsOverview.skillGroupPicker.manageSubtitle')}</span>
              <Button variant="secondary" size="small" onClick={startCreate} disabled={saving}>
                <Plus size={14} />
                {t('agentsOverview.skillGroupPicker.createGroup')}
              </Button>
            </div>
            {groups.length === 0 ? (
              <p className="skill-group-manager__empty">{t('agentsOverview.skillGroupPicker.noUserGroups')}</p>
            ) : (
              <div className="skill-group-manager__list">
                {groups.map((group, index) => {
                  const unavailable = unavailableUserSkillKeys(group, skills);
                  return (
                    <div key={group.id} className="skill-group-manager__group-row">
                      <div className="skill-group-manager__group-copy">
                        <span className="skill-group-manager__group-name">{group.name}</span>
                        <span className="skill-group-manager__group-meta">
                          {t('agentsOverview.skillGroupPicker.groupCount', { count: group.skillKeys.length })}
                          {unavailable.length > 0
                            ? ` · ${t('agentsOverview.skillGroupPicker.unavailableCount', { count: unavailable.length })}`
                            : ''}
                        </span>
                      </div>
                      <div className="skill-group-manager__group-actions">
                        <IconButton
                          type="button"
                          size="small"
                          variant="ghost"
                          aria-label={t('agentsOverview.skillGroupPicker.moveUp')}
                          tooltip={t('agentsOverview.skillGroupPicker.moveUp')}
                          onClick={() => void moveGroup(index, -1)}
                          disabled={saving || index === 0}
                        >
                          <ArrowUp size={13} />
                        </IconButton>
                        <IconButton
                          type="button"
                          size="small"
                          variant="ghost"
                          aria-label={t('agentsOverview.skillGroupPicker.moveDown')}
                          tooltip={t('agentsOverview.skillGroupPicker.moveDown')}
                          onClick={() => void moveGroup(index, 1)}
                          disabled={saving || index === groups.length - 1}
                        >
                          <ArrowDown size={13} />
                        </IconButton>
                        <IconButton
                          type="button"
                          size="small"
                          variant="ghost"
                          aria-label={t('agentsOverview.skillGroupPicker.editGroup')}
                          tooltip={t('agentsOverview.skillGroupPicker.editGroup')}
                          onClick={() => startEdit(group)}
                          disabled={saving}
                        >
                          <Pencil size={13} />
                        </IconButton>
                        <IconButton
                          type="button"
                          size="small"
                          variant="ghost"
                          aria-label={t('agentsOverview.skillGroupPicker.deleteGroup')}
                          tooltip={t('agentsOverview.skillGroupPicker.deleteGroup')}
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

export const SkillGroupPicker: React.FC<SkillGroupPickerProps> = ({
  skills,
  managementSkills,
  selectedSkillKeys,
  userGroups,
  onSelectionChange,
  onSaveUserGroups,
  disabled = false,
  testId,
}) => {
  const { t } = useTranslation('scenes/agents');
  const [isManagerOpen, setIsManagerOpen] = useState(false);
  const duplicateNames = useMemo(() => duplicateSkillNames(skills), [skills]);
  const groups = useMemo(() => resolveSkillGroups(skills, userGroups, {
    builtin: (groupKey) => builtinGroupLabel(groupKey, t),
    other: t('agentsOverview.skillGroupPicker.otherSkills'),
  }), [skills, t, userGroups]);
  const selectedCount = new Set(selectedSkillKeys).size;
  const sections = useMemo(() => {
    const grouped = new Map<string, ResolvedSkillGroup[]>();
    for (const group of groups) {
      const label = groupSectionLabel(group, t);
      const entries = grouped.get(label) ?? [];
      entries.push(group);
      grouped.set(label, entries);
    }
    return [...grouped.entries()];
  }, [groups, t]);

  return (
    <div className="skill-group-picker" data-testid={testId}>
      <div className="skill-group-picker__head">
        <span className="skill-group-picker__selected-count">
          {t('agentsOverview.skillGroupPicker.selectedCount', { count: selectedCount })}
        </span>
        <Button
          variant="ghost"
          size="small"
          onClick={() => setIsManagerOpen(true)}
          disabled={disabled}
        >
          <Settings2 size={14} />
          {t('agentsOverview.skillGroupPicker.manageGroups')}
        </Button>
      </div>
      <div className="skill-group-picker__sections">
        {sections.map(([sectionLabel, sectionGroups]) => (
          <section key={sectionLabel} className="skill-group-picker__section">
            <span className="skill-group-picker__section-label">{sectionLabel}</span>
            {sectionGroups.map((group) => {
              const selectedInGroup = selectedGroupSkillCount(group, selectedSkillKeys);
              const allSelected = isGroupEnabled(group, selectedSkillKeys);
              return (
                <div key={group.id} className="skill-group-picker__group">
                  <div className="skill-group-picker__group-head">
                    <div className="skill-group-picker__group-title-wrap">
                      <span className="skill-group-picker__group-name">{group.label}</span>
                      <span className="skill-group-picker__group-count">
                        {selectedInGroup}/{group.skills.length}
                      </span>
                    </div>
                    <div className="skill-group-picker__group-actions">
                      {selectedInGroup > 0 && !allSelected ? (
                        <Button
                          variant="ghost"
                          size="small"
                          onClick={() => onSelectionChange(
                            setSkillGroupSelection(selectedSkillKeys, skillGroupKeys(group), false),
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
                          setSkillGroupSelection(
                            selectedSkillKeys,
                            skillGroupKeys(group),
                            event.target.checked,
                          ),
                        )}
                        disabled={disabled}
                        aria-label={allSelected
                          ? t('agentsOverview.skillGroupPicker.clearGroupSkills', { name: group.label })
                          : t('agentsOverview.skillGroupPicker.enableGroupSkills', { name: group.label })}
                      />
                    </div>
                  </div>
                  <div className="skill-group-picker__token-grid">
                    {group.skills.map((skill) => {
                      const selected = selectedSkillKeys.includes(skill.key);
                      const tooltipFields = skillTooltipFields(skill, t);
                      return (
                        <AgentCapabilityTooltip
                          key={skill.key}
                          title={skillDisplayName(skill, duplicateNames)}
                          description={skill.description}
                          fields={tooltipFields}
                          placement="top"
                        >
                          <button
                            type="button"
                            className={`skill-group-picker__token${selected ? ' is-on' : ''}`}
                            onClick={() => onSelectionChange(
                              toggleSkillSelection(selectedSkillKeys, skill.key),
                            )}
                            disabled={disabled}
                            aria-label={capabilityTooltipAriaLabel(
                              skillDisplayName(skill, duplicateNames),
                              skill.description,
                              tooltipFields,
                            )}
                            aria-pressed={selected}
                          >
                            {skillDisplayName(skill, duplicateNames)}
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
      <SkillGroupManagerModal
        isOpen={isManagerOpen}
        onClose={() => setIsManagerOpen(false)}
        skills={managementSkills ?? skills}
        groups={userGroups}
        onSaveGroups={onSaveUserGroups}
      />
    </div>
  );
};

export const SkillGroupSummary: React.FC<SkillGroupSummaryProps> = ({
  skills,
  selectedSkillKeys,
  userGroups,
}) => {
  const { t } = useTranslation('scenes/agents');
  const duplicateNames = useMemo(() => duplicateSkillNames(skills), [skills]);
  const groups = useMemo(() => resolveSkillGroupSummary(skills, userGroups, selectedSkillKeys, {
    builtin: (groupKey) => builtinGroupLabel(groupKey, t),
    other: t('agentsOverview.skillGroupPicker.otherSkills'),
  }), [selectedSkillKeys, skills, t, userGroups]);

  if (groups.length === 0) {
    return <span className="agent-card__empty-inline">{t('agentsOverview.noSkills')}</span>;
  }

  return (
    <div className="skill-group-summary">
      {groups.map((group) => (
        <div key={group.id} className="skill-group-summary__group">
          <span className="skill-group-summary__label">{group.label}</span>
          <div className="skill-group-summary__skills">
            {group.skills.map((skill) => {
              const tooltipFields = skillTooltipFields(skill, t);
              return (
                <AgentCapabilityTooltip
                  key={skill.key}
                  title={skillDisplayName(skill, duplicateNames)}
                  description={skill.description}
                  fields={tooltipFields}
                >
                  <span className="agent-card__chip">
                    {skillDisplayName(skill, duplicateNames)}
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
