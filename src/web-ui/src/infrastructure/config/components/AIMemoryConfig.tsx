/**
 * AI Memory settings: user-level CRUD; project-level placeholder.
 */

import React, { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Edit2, Trash2, Eye, EyeOff } from 'lucide-react';
import { Select, Input, Textarea, Button, IconButton, Modal } from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigCollectionItem } from './common';
import { Tabs, TabPane } from '@/component-library';
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
import './AIMemoryConfig.scss';

const log = createLogger('AIMemoryConfig');

type ScopeTab = 'user' | 'project';

function MemoryPanel() {
  const { t } = useTranslation('settings/ai-memory');
  const { t: tScope } = useTranslation('settings/ai-context');
  const { error: notifyError, success: notifySuccess } = useNotification();
  const [expandedMemoryIds, setExpandedMemoryIds] = useState<Set<string>>(new Set());
  const [memories, setMemories] = useState<AIMemory[]>([]);
  const [loading, setLoading] = useState(true);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isAddDialogOpen, setIsAddDialogOpen] = useState(false);
  const [editingMemory, setEditingMemory] = useState<AIMemory | null>(null);
  const [scopeTab, setScopeTab] = useState<ScopeTab>('user');

  const loadMemories = useCallback(async () => {
    try {
      setLoading(true);
      const data = await getAllMemories();
      setMemories(data);
    } catch (error) {
      notifyError(t('messages.loadFailed', { error: String(error) }));
    } finally {
      setLoading(false);
    }
  }, [notifyError, t]);

  React.useEffect(() => {
    if (scopeTab === 'user') loadMemories();
  }, [scopeTab, loadMemories]);

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
      notifySuccess(t('messages.deleteSuccess'));
      await loadMemories();
    } catch (error) {
      log.error('Failed to delete memory', { memoryId: id, error });
      notifyError(t('messages.deleteFailed', { error: String(error) }));
    } finally {
      setIsDeleting(false);
    }
  };

  const handleToggle = async (id: string) => {
    try {
      await toggleMemory(id);
      loadMemories();
    } catch (error) {
      notifyError(t('messages.toggleFailed', { error: String(error) }));
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
    <IconButton variant="ghost" size="small" onClick={handleAdd} tooltip={t('toolbar.addTooltip')}>
      <Plus size={16} />
    </IconButton>
  );

  return (
    <div className="bitfun-ai-memory-page__memory-panel">
      <ConfigPageSection
        title={t('section.memoryList.title')}
        description={t('section.memoryList.description')}
        extra={scopeTab === 'user' ? addButtonUser : undefined}
      >
        <Tabs type="line" size="small" activeKey={scopeTab} onChange={(k) => setScopeTab(k as ScopeTab)} className="bitfun-ai-memory-page__scope-tabs">
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

const AIMemoryConfig: React.FC = () => {
  const { t } = useTranslation('settings/ai-context');

  return (
    <ConfigPageLayout className="bitfun-ai-memory-page">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent>
        <MemoryPanel />
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default AIMemoryConfig;
