/**
 * AIRulesMemoryConfig — merged Rules & Memory settings page.
 * Two sections (Rules / Memory), each with inner tabs: User | Project.
 * Rules: full CRUD for user/project. Memory: user-level CRUD; project-level placeholder.
 */

import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Edit2, Trash2, X, Eye, EyeOff } from 'lucide-react';
import { Select, Input, Textarea, Button, IconButton, Switch, Tooltip, Modal } from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow, ConfigCollectionItem } from './common';
import { Tabs, TabPane } from '@/component-library';
import { useAIRules } from '../../hooks/useAIRules';
import {
  AIRulesAPI,
  RuleLevel,
  RuleApplyType,
  type CreateRuleRequest,
  type AIRule
} from '../../api/service-api/AIRulesAPI';
import {
  getAllMemories,
  addMemory,
  updateMemory,
  deleteMemory,
  toggleMemory,
  type AIMemory,
  type MemoryType
} from '../../api/aiMemoryApi';
import { useNotification } from '@/shared/notification-system';
import { i18nService } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import './AIRulesMemoryConfig.scss';

const log = createLogger('AIRulesMemoryConfig');

type ScopeTab = 'user' | 'project';

// ----- Rules panel (reused logic from AIRulesConfig) -----

function RulesPanel() {
  const { t } = useTranslation('settings/ai-rules');
  const { t: tScope } = useTranslation('settings/ai-context');
  const [showAddForm, setShowAddForm] = useState(false);
  const [editingRule, setEditingRule] = useState<AIRule | null>(null);
  const [expandedRuleKeys, setExpandedRuleKeys] = useState<Set<string>>(new Set());
  const [isDeleting, setIsDeleting] = useState(false);
  const [scopeTab, setScopeTab] = useState<ScopeTab>('user');
  const [formLevel, setFormLevel] = useState<RuleLevel>(RuleLevel.User);
  const [formData, setFormData] = useState<CreateRuleRequest>({
    name: '',
    apply_type: RuleApplyType.AlwaysApply,
    content: '',
    description: '',
    globs: '',
  });

  const userRules = useAIRules(RuleLevel.User);
  const projectRules = useAIRules(RuleLevel.Project);

  const handleSubmit = async () => {
    if (!formData.name?.trim()) {
      alert(t('messages.nameRequired'));
      return;
    }
    if (!formData.content?.trim()) {
      alert(t('messages.contentRequired'));
      return;
    }
    const finalApplyType = formLevel === RuleLevel.User ? RuleApplyType.AlwaysApply : formData.apply_type;
    const ruleData: CreateRuleRequest = {
      name: formData.name.trim(),
      apply_type: finalApplyType,
      content: formData.content,
    };
    if (finalApplyType === RuleApplyType.ApplyIntelligently && formData.description) ruleData.description = formData.description;
    if (finalApplyType === RuleApplyType.ApplyToSpecificFiles && formData.globs) ruleData.globs = formData.globs;

    try {
      if (editingRule) {
        await AIRulesAPI.updateRule(formLevel, editingRule.name, {
          name: ruleData.name !== editingRule.name ? ruleData.name : undefined,
          apply_type: ruleData.apply_type,
          content: ruleData.content,
          description: ruleData.description,
          globs: ruleData.globs,
        });
        if (formLevel === RuleLevel.User) await userRules.refresh();
        else await projectRules.refresh();
      } else {
        if (formLevel === RuleLevel.User) await userRules.createRule(ruleData);
        else await projectRules.createRule(ruleData);
      }
      resetForm();
    } catch (error) {
      log.error('Failed to save rule', error);
      alert(t('messages.saveFailed', { error: error instanceof Error ? error.message : String(error) }));
    }
  };

  const handleAdd = (level: RuleLevel) => {
    resetForm();
    setFormLevel(level);
    setShowAddForm(true);
    setEditingRule(null);
  };

  const handleEdit = (rule: AIRule) => {
    setFormData({
      name: rule.name,
      apply_type: rule.apply_type as RuleApplyType,
      content: rule.content,
      description: rule.description || '',
      globs: rule.globs || '',
    });
    setFormLevel(rule.level === 'user' ? RuleLevel.User : RuleLevel.Project);
    setEditingRule(rule);
    setShowAddForm(true);
  };

  const handleDelete = async (rule: AIRule, event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    if (isDeleting) return;
    if (!(await window.confirm(t('messages.confirmDelete', { name: rule.name })))) return;
    try {
      setIsDeleting(true);
      const level = rule.level === 'user' ? RuleLevel.User : RuleLevel.Project;
      await AIRulesAPI.deleteRule(level, rule.name);
      if (level === RuleLevel.User) await userRules.refresh();
      else await projectRules.refresh();
    } catch (error) {
      log.error('Failed to delete rule', { ruleName: rule.name, level: rule.level, error });
      alert(t('messages.deleteFailed', { error: error instanceof Error ? error.message : String(error) }));
    } finally {
      setIsDeleting(false);
    }
  };

  const resetForm = () => {
    setFormData({
      name: '',
      apply_type: RuleApplyType.AlwaysApply,
      content: '',
      description: '',
      globs: '',
    });
    setFormLevel(RuleLevel.User);
    setShowAddForm(false);
    setEditingRule(null);
  };

  const handleToggle = async (rule: AIRule, event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    try {
      const level = rule.level === 'user' ? RuleLevel.User : RuleLevel.Project;
      if (level === RuleLevel.User) await userRules.toggleRule(rule.name);
      else await projectRules.toggleRule(rule.name);
    } catch (error) {
      log.error('Failed to toggle rule', { ruleName: rule.name, level: rule.level, error });
      alert(t('messages.toggleFailed', { error: error instanceof Error ? error.message : String(error) }));
    }
  };

  const getRuleKey = (rule: AIRule) => `${rule.level}-${rule.name}`;
  const toggleRuleExpanded = (ruleKey: string) => {
    setExpandedRuleKeys((prev) => {
      const next = new Set(prev);
      if (next.has(ruleKey)) next.delete(ruleKey);
      else next.add(ruleKey);
      return next;
    });
  };

  const getApplyTypeOptions = () => [
    { label: t('form.fields.applyTypes.alwaysApply'), value: RuleApplyType.AlwaysApply },
    { label: t('form.fields.applyTypes.applyIntelligently'), value: RuleApplyType.ApplyIntelligently },
    { label: t('form.fields.applyTypes.applyToSpecificFiles'), value: RuleApplyType.ApplyToSpecificFiles },
    { label: t('form.fields.applyTypes.applyManually'), value: RuleApplyType.ApplyManually },
  ];

  const renderForm = () => {
    if (!showAddForm) return null;
    const isUserLevel = formLevel === RuleLevel.User;
    const showDescription = !isUserLevel && formData.apply_type === RuleApplyType.ApplyIntelligently;
    const showGlobs = !isUserLevel && formData.apply_type === RuleApplyType.ApplyToSpecificFiles;
    return (
      <div className="bitfun-ai-rules-config__form">
        <div className="bitfun-ai-rules-config__form-header">
          <h3>{editingRule ? t('form.titleEdit') : t('form.titleCreate')}</h3>
          <IconButton variant="ghost" size="small" onClick={resetForm} tooltip={t('form.closeTooltip')}>
            <X size={14} />
          </IconButton>
        </div>
        <div className="bitfun-ai-rules-config__form-body">
          <Input label={t('form.fields.name')} value={formData.name} onChange={(e) => setFormData({ ...formData, name: e.target.value })} placeholder={t('form.fields.namePlaceholder')} variant="outlined" size="small" />
          {!isUserLevel && (
            <Select label={t('form.fields.applyType')} options={getApplyTypeOptions()} value={formData.apply_type} onChange={(value) => setFormData({ ...formData, apply_type: value as RuleApplyType })} size="medium" />
          )}
          {showDescription && (
            <Input label={t('form.fields.description')} value={formData.description || ''} onChange={(e) => setFormData({ ...formData, description: e.target.value })} placeholder={t('form.fields.descriptionPlaceholder')} variant="outlined" size="small" />
          )}
          {showGlobs && (
            <Input label={t('form.fields.globs')} value={formData.globs || ''} onChange={(e) => setFormData({ ...formData, globs: e.target.value })} placeholder={t('form.fields.globsPlaceholder')} variant="outlined" size="small" />
          )}
          <Textarea label={t('form.fields.content')} value={formData.content} onChange={(e) => setFormData({ ...formData, content: e.target.value })} placeholder={t('form.fields.contentPlaceholder')} rows={6} variant="outlined" />
        </div>
        <div className="bitfun-ai-rules-config__form-footer">
          <Button variant="secondary" size="small" onClick={resetForm}>{t('form.actions.cancel')}</Button>
          <Button variant="primary" size="small" onClick={handleSubmit}>{editingRule ? t('form.actions.save') : t('form.actions.add')}</Button>
        </div>
      </div>
    );
  };

  const renderRuleSection = (level: RuleLevel) => {
    const isUser = level === RuleLevel.User;
    const currentRules = level === RuleLevel.User ? userRules : projectRules;
    const rules = currentRules.rules;
    return (
      <>
        {showAddForm && formLevel === level && renderForm()}
        {currentRules.isLoading && (
          <ConfigPageRow label={t('list.loading')} align="center"><span /></ConfigPageRow>
        )}
        {!currentRules.isLoading && rules.length === 0 && (
          <div className="bitfun-ai-rules-config__empty-row">
            <p>{t('list.empty.description')}</p>
            <Button variant="secondary" size="small" onClick={() => handleAdd(level)}>
              <Plus size={14} /> {t('form.actions.add')}
            </Button>
          </div>
        )}
        {rules.map((rule) => {
          const ruleKey = getRuleKey(rule);
          const isExpanded = expandedRuleKeys.has(ruleKey);
          return (
            <React.Fragment key={ruleKey}>
              <div
                className={`bitfun-config-page-row bitfun-config-page-row--center bitfun-ai-rules-config__rule-row ${!rule.enabled ? 'is-disabled' : ''}`}
                onClick={() => toggleRuleExpanded(ruleKey)}
              >
                <div className="bitfun-config-page-row__meta">
                  <p className="bitfun-config-page-row__label bitfun-ai-rules-config__rule-label">
                    <span className="bitfun-ai-rules-config__rule-name">{rule.name}</span>
                    <span className="bitfun-ai-rules-config__rule-badge">{AIRulesAPI.getApplyTypeLabel(rule.apply_type as RuleApplyType)}</span>
                  </p>
                </div>
                <div className="bitfun-config-page-row__control" onClick={(e) => e.stopPropagation()}>
                  <div className="bitfun-ai-rules-config__item-actions">
                    <Switch checked={rule.enabled} onChange={(e) => handleToggle(rule, e as unknown as React.MouseEvent)} size="small" />
                    <Tooltip content={t('list.item.editTooltip')}>
                      <button type="button" className="bitfun-ai-rules-config__action-btn" onClick={(e) => { e.stopPropagation(); handleEdit(rule); }}>
                        <Edit2 size={14} />
                      </button>
                    </Tooltip>
                    <Tooltip content={t('list.item.deleteTooltip')}>
                      <button type="button" className="bitfun-ai-rules-config__action-btn bitfun-ai-rules-config__action-btn--danger" onClick={(e) => handleDelete(rule, e)}>
                        <Trash2 size={14} />
                      </button>
                    </Tooltip>
                  </div>
                </div>
              </div>
              {isExpanded && (
                <div className="bitfun-ai-rules-config__details">
                  {rule.description && (
                    <div className="bitfun-ai-rules-config__details-field">
                      <span className="bitfun-ai-rules-config__details-label">{t('list.item.descriptionLabel')}</span>
                      <span>{rule.description}</span>
                    </div>
                  )}
                  {rule.globs && (
                    <div className="bitfun-ai-rules-config__details-field">
                      <span className="bitfun-ai-rules-config__details-label">{t('list.item.globsLabel')}</span>
                      <code className="bitfun-ai-rules-config__details-code">{rule.globs}</code>
                    </div>
                  )}
                  <div className="bitfun-ai-rules-config__details-content">
                    <div className="bitfun-ai-rules-config__details-label">{t('list.item.contentLabel')}</div>
                    <pre className="bitfun-ai-rules-config__details-pre">{rule.content}</pre>
                  </div>
                  <div className="bitfun-ai-rules-config__details-meta">{t('list.item.filePathPrefix')}{rule.file_path}</div>
                </div>
              )}
            </React.Fragment>
          );
        })}
      </>
    );
  };

  const levelForScope: Record<ScopeTab, RuleLevel> = { user: RuleLevel.User, project: RuleLevel.Project };
  const currentLevel = levelForScope[scopeTab];
  const addButton = (
    <IconButton variant="primary" size="small" onClick={() => handleAdd(currentLevel)} tooltip={t('toolbar.addTooltip')}>
      <Plus size={16} />
    </IconButton>
  );

  return (
    <div className="bitfun-ai-rules-memory-config__rules-panel">
      <ConfigPageSection
        title={t('title')}
        description={t('subtitle')}
        extra={addButton}
      >
        <Tabs type="line" size="small" activeKey={scopeTab} onChange={(k) => setScopeTab(k as ScopeTab)} className="bitfun-ai-rules-memory-config__scope-tabs">
          <TabPane tabKey="user" label={tScope('scope.user')}>
            {scopeTab === 'user' && renderRuleSection(RuleLevel.User)}
          </TabPane>
          <TabPane tabKey="project" label={tScope('scope.project')}>
            {scopeTab === 'project' && renderRuleSection(RuleLevel.Project)}
          </TabPane>
        </Tabs>
      </ConfigPageSection>
    </div>
  );
}

// ----- Memory panel: user = real CRUD, project = placeholder -----

function MemoryPanel() {
  const { t } = useTranslation('settings/ai-memory');
  const { t: tScope } = useTranslation('settings/ai-context');
  const notification = useNotification();
  const [expandedMemoryIds, setExpandedMemoryIds] = useState<Set<string>>(new Set());
  const [memories, setMemories] = useState<AIMemory[]>([]);
  const [loading, setLoading] = useState(true);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isAddDialogOpen, setIsAddDialogOpen] = useState(false);
  const [editingMemory, setEditingMemory] = useState<AIMemory | null>(null);
  const [scopeTab, setScopeTab] = useState<ScopeTab>('user');

  const loadMemories = async () => {
    try {
      setLoading(true);
      const data = await getAllMemories();
      setMemories(data);
    } catch (error) {
      notification.error(t('messages.loadFailed', { error: String(error) }));
    } finally {
      setLoading(false);
    }
  };

  React.useEffect(() => {
    if (scopeTab === 'user') loadMemories();
  }, [scopeTab]);

  const memoryTypeMap: Record<MemoryType, { label: string; color: string }> = {
    tech_preference: { label: t('memoryTypes.tech_preference'), color: '#60a5fa' },
    project_context: { label: t('memoryTypes.project_context'), color: '#a78bfa' },
    user_habit: { label: t('memoryTypes.user_habit'), color: '#34d399' },
    code_pattern: { label: t('memoryTypes.code_pattern'), color: '#fbbf24' },
    decision: { label: t('memoryTypes.decision'), color: '#f87171' },
    other: { label: t('memoryTypes.other'), color: '#94a3b8' }
  };

  const sortedMemories = [...memories].sort((a, b) => b.importance - a.importance);
  const toggleMemoryExpanded = (memoryId: string) => {
    setExpandedMemoryIds((prev) => {
      const next = new Set(prev);
      if (next.has(memoryId)) next.delete(memoryId);
      else next.add(memoryId);
      return next;
    });
  };

  const handleDelete = async (id: string, event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    if (isDeleting) return;
    if (!(await window.confirm(t('messages.confirmDelete')))) return;
    try {
      setIsDeleting(true);
      await deleteMemory(id);
      notification.success(t('messages.deleteSuccess'));
      await loadMemories();
    } catch (error) {
      log.error('Failed to delete memory', { memoryId: id, error });
      notification.error(t('messages.deleteFailed', { error: String(error) }));
    } finally {
      setIsDeleting(false);
    }
  };

  const handleToggle = async (id: string) => {
    try {
      await toggleMemory(id);
      loadMemories();
    } catch (error) {
      notification.error(t('messages.toggleFailed', { error: String(error) }));
    }
  };

  const handleAdd = () => {
    setEditingMemory(null);
    setIsAddDialogOpen(true);
  };

  const handleEdit = (memory: AIMemory, e: React.MouseEvent) => {
    e.stopPropagation();
    setEditingMemory(memory);
    setIsAddDialogOpen(true);
  };

  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    const now = new Date();
    const diffDays = Math.floor((now.getTime() - date.getTime()) / (1000 * 60 * 60 * 24));
    if (diffDays === 0) return t('date.today');
    if (diffDays === 1) return t('date.yesterday');
    if (diffDays < 7) return t('date.daysAgo', { days: diffDays });
    return i18nService.formatDate(date, { year: 'numeric', month: '2-digit', day: '2-digit' });
  };

  const renderMemoryBadge = (memory: AIMemory) => {
    const typeInfo = memoryTypeMap[memory.type];
    return (
      <>
        <span className="bitfun-ai-memory-config__badge--type" style={{ background: `${typeInfo.color}20`, color: typeInfo.color }}>
          {typeInfo.label}
        </span>
        <span className="bitfun-collection-item__badge">{formatDate(memory.created_at)}</span>
      </>
    );
  };

  const renderMemoryControl = (memory: AIMemory) => (
    <>
      <IconButton tooltip={memory.enabled ? t('actions.disable') : t('actions.enable')} onClick={() => handleToggle(memory.id)} size="small" variant="ghost">
        {memory.enabled ? <Eye size={14} /> : <EyeOff size={14} />}
      </IconButton>
      <IconButton tooltip={t('actions.edit')} onClick={(e) => handleEdit(memory, e)} size="small" variant="ghost">
        <Edit2 size={14} />
      </IconButton>
      <IconButton tooltip={t('actions.delete')} onClick={(e) => handleDelete(memory.id, e)} size="small" variant="danger" disabled={isDeleting}>
        <Trash2 size={14} />
      </IconButton>
    </>
  );

  const renderMemoryDetails = (memory: AIMemory) => (
    <>
      <div className="bitfun-collection-details__field">
        <div className="bitfun-collection-details__label">{t('list.item.contentLabel')}</div>
        {memory.content}
      </div>
      <div className="bitfun-collection-details__meta">
        <span>{t('list.item.sourcePrefix')}{memory.source}</span>
        {' · '}
        <span>{t('list.item.createdPrefix')}{i18nService.formatDate(new Date(memory.created_at))}</span>
      </div>
    </>
  );

  const addButtonUser = (
    <IconButton variant="primary" size="small" onClick={handleAdd} tooltip={t('toolbar.addTooltip')}>
      <Plus size={16} />
    </IconButton>
  );

  return (
    <div className="bitfun-ai-rules-memory-config__memory-panel">
      <ConfigPageSection
        title={t('section.knowledgeList.title', { defaultValue: '知识记忆列表' })}
        description={t('section.knowledgeList.description', { defaultValue: '可新建与查看的知识记忆。' })}
        extra={scopeTab === 'user' ? addButtonUser : undefined}
      >
        <Tabs type="line" size="small" activeKey={scopeTab} onChange={(k) => setScopeTab(k as ScopeTab)} className="bitfun-ai-rules-memory-config__scope-tabs">
          <TabPane tabKey="user" label={tScope('scope.user')}>
            {scopeTab === 'user' && (
              <>
                {loading && (
                  <div className="bitfun-collection-empty"><p>{t('list.loading')}</p></div>
                )}
                {!loading && sortedMemories.length === 0 && (
                  <div className="bitfun-collection-empty">
                    <p>{t('list.empty.title')}</p>
                    <Button variant="dashed" size="small" onClick={handleAdd}>
                      <Plus size={14} /> {t('toolbar.addTooltip')}
                    </Button>
                  </div>
                )}
                {!loading && sortedMemories.map((memory) => (
                  <ConfigCollectionItem
                    key={memory.id}
                    label={memory.title}
                    badge={renderMemoryBadge(memory)}
                    control={renderMemoryControl(memory)}
                    details={renderMemoryDetails(memory)}
                    disabled={!memory.enabled}
                    expanded={expandedMemoryIds.has(memory.id)}
                    onToggle={() => toggleMemoryExpanded(memory.id)}
                  />
                ))}
              </>
            )}
          </TabPane>
          <TabPane tabKey="project" label={tScope('scope.project')}>
            {scopeTab === 'project' && (
              <div className="bitfun-collection-empty">
                <p>{tScope('memoryProjectPlaceholder')}</p>
              </div>
            )}
          </TabPane>
        </Tabs>
      </ConfigPageSection>

      {isAddDialogOpen && (
        <MemoryEditDialog
          memory={editingMemory}
          memoryTypeMap={memoryTypeMap}
          onClose={() => setIsAddDialogOpen(false)}
          onSave={loadMemories}
        />
      )}
    </div>
  );
}

interface MemoryEditDialogProps {
  memory: AIMemory | null;
  memoryTypeMap: Record<MemoryType, { label: string; color: string }>;
  onClose: () => void;
  onSave: () => void;
}

const MemoryEditDialog: React.FC<MemoryEditDialogProps> = ({ memory, memoryTypeMap, onClose, onSave }) => {
  const { t } = useTranslation('settings/ai-memory');
  const notification = useNotification();
  const [title, setTitle] = useState(memory?.title || '');
  const [content, setContent] = useState(memory?.content || '');
  const [memoryType, setMemoryType] = useState<MemoryType>(memory?.type || 'other');
  const [importance, setImportance] = useState(memory?.importance || 3);
  const [tags, setTags] = useState(memory?.tags?.join(', ') || '');
  const [saving, setSaving] = useState(false);

  const handleSave = async () => {
    if (!title.trim() || !content.trim()) {
      notification.error(t('messages.validationError'));
      return;
    }
    try {
      setSaving(true);
      const tagsArray = tags.split(',').map((s) => s.trim()).filter(Boolean);
      if (memory) {
        await updateMemory({ id: memory.id, title, content, type: memoryType, importance, tags: tagsArray, enabled: memory.enabled });
        notification.success(t('messages.updateSuccess'));
      } else {
        await addMemory({ title, content, type: memoryType, importance, tags: tagsArray });
        notification.success(t('messages.createSuccess'));
      }
      onSave();
      onClose();
    } catch (error) {
      notification.error(t('messages.saveFailed', { error: String(error) }));
    } finally {
      setSaving(false);
    }
  };

  const typeOptions = Object.entries(memoryTypeMap).map(([key, info]) => ({ value: key, label: info.label }));

  return (
    <Modal isOpen onClose={onClose} title={memory ? t('dialog.titleEdit') : t('dialog.titleCreate')} size="medium">
      <div className="bitfun-ai-memory-config__dialog-body">
        <Input label={t('dialog.fields.title')} value={title} onChange={(e) => setTitle(e.target.value)} placeholder={t('dialog.fields.titlePlaceholder')} />
        <Select label={t('dialog.fields.type')} options={typeOptions} value={memoryType} onChange={(val) => setMemoryType(val as MemoryType)} />
        <div className="bitfun-ai-memory-config__form-group">
          <label>{t('dialog.fields.importance')} ({importance}/5)</label>
          <input type="range" min={1} max={5} value={importance} onChange={(e) => setImportance(Number(e.target.value))} />
        </div>
        <Textarea label={t('dialog.fields.content')} value={content} onChange={(e) => setContent(e.target.value)} placeholder={t('dialog.fields.contentPlaceholder')} rows={6} />
        <Input label={t('dialog.fields.tags')} value={tags} onChange={(e) => setTags(e.target.value)} placeholder={t('dialog.fields.tagsPlaceholder')} />
      </div>
      <div className="bitfun-ai-memory-config__dialog-footer">
        <Button variant="secondary" onClick={onClose} disabled={saving}>{t('dialog.actions.cancel')}</Button>
        <Button variant="primary" onClick={handleSave} disabled={saving} isLoading={saving}>
          {saving ? t('dialog.actions.saving') : t('dialog.actions.save')}
        </Button>
      </div>
    </Modal>
  );
};

// ----- Main page: two sections (Rules + Memory), each with User/Project tabs -----

const AIRulesMemoryConfig: React.FC = () => {
  const { t } = useTranslation('settings/ai-context');

  return (
    <ConfigPageLayout className="bitfun-ai-rules-memory-config">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent>
        <RulesPanel />
        <MemoryPanel />
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default AIRulesMemoryConfig;
