import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Star, Edit2, Trash2, Copy, Download, Upload } from 'lucide-react';
import { Button, IconButton, Modal, Input, Textarea, ConfirmDialog } from '@/component-library';
import { ConfigPageLayout, ConfigPageHeader, ConfigPageContent, ConfigPageSection, ConfigCollectionItem } from './common';
import { promptTemplateService } from '@/infrastructure/services/PromptTemplateService';
import { notificationService } from '@/shared/notification-system';
import type { PromptTemplate } from '@/shared/types/prompt-template';
import { downloadDir, join } from '@tauri-apps/api/path';
import { writeFile } from '@tauri-apps/plugin-fs';
import './PromptTemplateConfig.scss';

export const PromptTemplateConfig: React.FC = () => {
  const { t } = useTranslation('settings/prompt-templates');
  const [templates, setTemplates] = useState<PromptTemplate[]>([]);
  const [globalShortcut, setGlobalShortcut] = useState('Ctrl+Shift+P');
  const [isEditing, setIsEditing] = useState(false);
  const [editingTemplate, setEditingTemplate] = useState<PromptTemplate | null>(null);
  const [expandedTemplateIds, setExpandedTemplateIds] = useState<Set<string>>(new Set());
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);

  useEffect(() => {
    const initializeAndLoad = async () => {
      await promptTemplateService.initialize();
      loadTemplates();
      const config = promptTemplateService.getConfig();
      setGlobalShortcut(config.globalShortcut);
    };
    initializeAndLoad();
    const unsubscribe = promptTemplateService.subscribe(() => { loadTemplates(); });
    return unsubscribe;
  }, []);

  const loadTemplates = () => {
    setTemplates(promptTemplateService.getAllTemplates());
  };

  const sortedTemplates = [...templates].sort((a, b) => {
    if (a.isFavorite && !b.isFavorite) return -1;
    if (!a.isFavorite && b.isFavorite) return 1;
    return b.usageCount - a.usageCount;
  });

  const handleCreateTemplate = () => {
    setEditingTemplate({
      id: '',
      name: '',
      description: '',
      content: '',
      category: t('categories.uncategorized'),
      isFavorite: false,
      order: templates.length,
      createdAt: Date.now(),
      updatedAt: Date.now(),
      usageCount: 0
    });
    setIsEditing(true);
  };

  const handleEditTemplate = (template: PromptTemplate, e: React.MouseEvent) => {
    e.stopPropagation();
    setEditingTemplate({ ...template });
    setIsEditing(true);
  };

  const handleSaveTemplate = async () => {
    if (!editingTemplate) return;
    if (!editingTemplate.name.trim()) { notificationService.error(t('messages.nameRequired')); return; }
    if (!editingTemplate.content.trim()) { notificationService.error(t('messages.contentRequired')); return; }
    try {
      if (editingTemplate.id) {
        await promptTemplateService.updateTemplate(editingTemplate.id, editingTemplate);
        notificationService.success(t('messages.templateUpdated'));
      } else {
        await promptTemplateService.createTemplate(editingTemplate);
        notificationService.success(t('messages.templateCreated'));
      }
      setIsEditing(false);
      setEditingTemplate(null);
      loadTemplates();
    } catch (error) {
      notificationService.error(t('messages.operationFailed', { error: (error as Error).message }));
    }
  };

  const handleDeleteTemplate = async (id: string) => {
    try {
      await promptTemplateService.deleteTemplate(id);
      notificationService.success(t('messages.templateDeleted'));
      loadTemplates();
    } catch (error) {
      notificationService.error(t('messages.deleteFailed', { error: (error as Error).message }));
    }
  };

  const handleToggleFavorite = async (template: PromptTemplate, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await promptTemplateService.updateTemplate(template.id, { isFavorite: !template.isFavorite });
      loadTemplates();
    } catch (error) {
      notificationService.error(t('messages.operationFailed', { error: (error as Error).message }));
    }
  };

  const handleDuplicateTemplate = async (template: PromptTemplate, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await promptTemplateService.createTemplate({
        ...template,
        name: `${template.name} (copy)`,
        order: templates.length
      });
      notificationService.success(t('messages.templateCopied'));
      loadTemplates();
    } catch (error) {
      notificationService.error(t('messages.copyFailed', { error: (error as Error).message }));
    }
  };

  const handleExport = async () => {
    try {
      const json = await promptTemplateService.exportConfig();
      const fileName = `prompt-templates-${Date.now()}.json`;
      const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
      if (isTauri) {
        try {
          const downloadsPath = await downloadDir();
          const filePath = await join(downloadsPath, fileName);
          const content = new TextEncoder().encode(json);
          await writeFile(filePath, content);
          notificationService.success(t('messages.configExported', { filePath }));
          return;
        } catch {
          // fallback to browser download
        }
      }
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = fileName;
      a.click();
      URL.revokeObjectURL(url);
      notificationService.success(t('messages.configExported', { filePath: t('messages.defaultDownloadDir') }));
    } catch (error) {
      notificationService.error(t('messages.exportFailed', { error: (error as Error).message }));
    }
  };

  const handleImport = () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json';
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;
      try {
        const text = await file.text();
        if (await promptTemplateService.importConfig(text)) {
          notificationService.success(t('messages.configImported'));
          loadTemplates();
        } else {
          notificationService.error(t('messages.importInvalid'));
        }
      } catch (error) {
        notificationService.error(t('messages.importFailed', { error: (error as Error).message }));
      }
    };
    input.click();
  };

  const toggleTemplateExpanded = (templateId: string) => {
    setExpandedTemplateIds(prev => {
      const next = new Set(prev);
      if (next.has(templateId)) next.delete(templateId);
      else next.add(templateId);
      return next;
    });
  };

  const sectionExtra = (
    <>
      <IconButton variant="ghost" size="small" onClick={handleExport} tooltip={t('toolbar.exportTooltip')}>
        <Download size={16} />
      </IconButton>
      <IconButton variant="ghost" size="small" onClick={handleImport} tooltip={t('toolbar.importTooltip')}>
        <Upload size={16} />
      </IconButton>
      <IconButton variant="primary" size="small" onClick={handleCreateTemplate} tooltip={t('toolbar.createTooltip')}>
        <Plus size={16} />
      </IconButton>
    </>
  );

  return (
    <ConfigPageLayout className="prompt-template-config">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />

      <ConfigPageContent>
        <ConfigPageSection
          title={t('section.knowledgeList.title', { defaultValue: '提示词模板列表' })}
          description={
            <>
              {t('section.knowledgeList.description', { defaultValue: '可创建、收藏与复用的提示词模板。' })}
              {' '}
              {t('shortcuts.openPickerReminder')}
              <kbd className="prompt-template-config__shortcut-key">{globalShortcut}</kbd>
            </>
          }
          extra={sectionExtra}
        >
          {sortedTemplates.length === 0 && (
            <div className="bitfun-collection-empty">
              <p>{t('empty.createFirst')}</p>
              <Button variant="dashed" size="small" onClick={handleCreateTemplate}>
                <Plus size={14} />
                {t('toolbar.createTooltip')}
              </Button>
            </div>
          )}

          {sortedTemplates.map(template => {
            const badge = (
              <>
                {template.category && (
                  <span className="bitfun-collection-item__badge">{template.category}</span>
                )}
                {template.isFavorite && (
                  <span className="prompt-template-config__fav-badge">
                    <Star size={10} fill="currentColor" />
                  </span>
                )}
              </>
            );

            const control = (
              <>
                <IconButton
                  variant={template.isFavorite ? 'primary' : 'ghost'}
                  size="small"
                  onClick={(e) => handleToggleFavorite(template, e)}
                  tooltip={template.isFavorite ? t('actions.unfavorite') : t('actions.favorite')}
                >
                  <Star size={14} fill={template.isFavorite ? 'currentColor' : 'none'} />
                </IconButton>
                <IconButton
                  variant="ghost"
                  size="small"
                  onClick={(e) => handleEditTemplate(template, e)}
                  tooltip={t('actions.edit')}
                >
                  <Edit2 size={14} />
                </IconButton>
                <IconButton
                  variant="ghost"
                  size="small"
                  onClick={(e) => handleDuplicateTemplate(template, e)}
                  tooltip={t('actions.copy')}
                >
                  <Copy size={14} />
                </IconButton>
                <IconButton
                  variant="danger"
                  size="small"
                  onClick={(e) => { e.stopPropagation(); setDeleteConfirmId(template.id); }}
                  tooltip={t('actions.delete')}
                >
                  <Trash2 size={14} />
                </IconButton>
              </>
            );

            const details = (
              <>
                {template.description && (
                  <div className="bitfun-collection-details__field">{template.description}</div>
                )}
                <div>
                  <div className="bitfun-collection-details__label">{t('template.contentLabel')}</div>
                  <pre className="bitfun-collection-details__pre">{template.content}</pre>
                </div>
                <div className="bitfun-collection-details__meta">
                  {t('template.usageCount', { count: template.usageCount })}
                  {template.shortcut && (
                    <kbd className="bitfun-collection-details__code">{template.shortcut}</kbd>
                  )}
                </div>
              </>
            );

            return (
              <ConfigCollectionItem
                key={template.id}
                label={template.name}
                badge={badge}
                control={control}
                details={details}
                expanded={expandedTemplateIds.has(template.id)}
                onToggle={() => toggleTemplateExpanded(template.id)}
              />
            );
          })}
        </ConfigPageSection>
      </ConfigPageContent>

      <Modal
        isOpen={isEditing && !!editingTemplate}
        onClose={() => setIsEditing(false)}
        title={editingTemplate?.id ? t('modal.titleEdit') : t('modal.titleCreate')}
        size="medium"
      >
        {editingTemplate && (
          <>
            <div className="prompt-template-config__modal-body">
              <Input
                label={t('modal.fields.name')}
                value={editingTemplate.name}
                onChange={(e) => setEditingTemplate({ ...editingTemplate, name: e.target.value })}
                placeholder={t('modal.fields.namePlaceholder')}
              />
              <Input
                label={t('modal.fields.description')}
                value={editingTemplate.description || ''}
                onChange={(e) => setEditingTemplate({ ...editingTemplate, description: e.target.value })}
                placeholder={t('modal.fields.descriptionPlaceholder')}
              />
              <Input
                label={t('modal.fields.category')}
                value={editingTemplate.category || ''}
                onChange={(e) => setEditingTemplate({ ...editingTemplate, category: e.target.value })}
                placeholder={t('modal.fields.categoryPlaceholder')}
              />
              <Textarea
                label={t('modal.fields.content')}
                hint={t('modal.fields.contentHint')}
                value={editingTemplate.content}
                onChange={(e) => setEditingTemplate({ ...editingTemplate, content: e.target.value })}
                placeholder={t('modal.fields.contentPlaceholder')}
                rows={10}
              />
              <Input
                label={t('modal.fields.shortcut')}
                value={editingTemplate.shortcut || ''}
                onChange={(e) => setEditingTemplate({ ...editingTemplate, shortcut: e.target.value })}
                placeholder={t('modal.fields.shortcutPlaceholder')}
              />
            </div>
            <div className="prompt-template-config__modal-footer">
              <Button variant="secondary" size="medium" onClick={() => setIsEditing(false)}>
                {t('modal.actions.cancel')}
              </Button>
              <Button variant="primary" size="medium" onClick={handleSaveTemplate}>
                {t('modal.actions.save')}
              </Button>
            </div>
          </>
        )}
      </Modal>

      <ConfirmDialog
        isOpen={!!deleteConfirmId}
        onClose={() => setDeleteConfirmId(null)}
        onConfirm={() => {
          if (deleteConfirmId) {
            handleDeleteTemplate(deleteConfirmId);
            setDeleteConfirmId(null);
          }
        }}
        title={t('messages.confirmDeleteTitle')}
        message={t('messages.confirmDelete')}
        type="warning"
        confirmDanger
      />
    </ConfigPageLayout>
  );
};

export default PromptTemplateConfig;
