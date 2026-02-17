import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Star, Edit2, Trash2, Copy, Download, Upload } from 'lucide-react';
import { Search, Button, IconButton, Card, CardBody, FilterPill, FilterPillGroup, Alert, Modal, Input, Textarea, ConfirmDialog } from '@/component-library';
import { ConfigPageLayout, ConfigPageHeader, ConfigPageContent } from './common';
import { promptTemplateService } from '@/infrastructure/services/PromptTemplateService';
import { notificationService } from '@/shared/notification-system';
import type { PromptTemplate } from '@/shared/types/prompt-template';
import { downloadDir, join } from '@tauri-apps/api/path';
import { writeFile } from '@tauri-apps/plugin-fs';
import './PromptTemplateConfig.scss';

export const PromptTemplateConfig: React.FC = () => {
  const { t } = useTranslation('settings/prompt-templates');
  const [templates, setTemplates] = useState<PromptTemplate[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<string>('all');
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

    
    const unsubscribe = promptTemplateService.subscribe(() => {
      loadTemplates();
    });

    return unsubscribe;
  }, []);

  const loadTemplates = () => {
    const allTemplates = promptTemplateService.getAllTemplates();
    setTemplates(allTemplates);
  };

  
  const categories = ['all', ...promptTemplateService.getCategories()];

  
  const filteredTemplates = templates.filter(template => {
    const matchSearch = !searchQuery || 
      template.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      template.description?.toLowerCase().includes(searchQuery.toLowerCase()) ||
      template.content.toLowerCase().includes(searchQuery.toLowerCase());
    
    const matchCategory = selectedCategory === 'all' || template.category === selectedCategory;
    
    return matchSearch && matchCategory;
  });

  
  const sortedTemplates = [...filteredTemplates].sort((a, b) => {
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

  
  const handleEditTemplate = (template: PromptTemplate) => {
    setEditingTemplate({ ...template });
    setIsEditing(true);
  };

  
  const handleSaveTemplate = async () => {
    if (!editingTemplate) return;

    if (!editingTemplate.name.trim()) {
      notificationService.error(t('messages.nameRequired'));
      return;
    }

    if (!editingTemplate.content.trim()) {
      notificationService.error(t('messages.contentRequired'));
      return;
    }

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

  
  const handleToggleFavorite = async (template: PromptTemplate) => {
    try {
      await promptTemplateService.updateTemplate(template.id, {
        isFavorite: !template.isFavorite
      });
      loadTemplates();
    } catch (error) {
      notificationService.error(t('messages.operationFailed', { error: (error as Error).message }));
    }
  };

  
  const handleDuplicateTemplate = async (template: PromptTemplate) => {
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
        } catch (saveError) {
          // Fallback to browser download if file write fails.
        }
      }

      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = fileName;
      a.click();
      URL.revokeObjectURL(url);
      notificationService.success(
        t('messages.configExported', { filePath: t('messages.defaultDownloadDir') })
      );
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
      if (next.has(templateId)) {
        next.delete(templateId);
      } else {
        next.add(templateId);
      }
      return next;
    });
  };

  return (
    <ConfigPageLayout className="prompt-template-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />

      <ConfigPageContent className="prompt-template-config__content">
        <div className="prompt-template-config__tab-content">
          
          <div className="prompt-template-config__message-container">
            <Alert
              type="info"
              message={
                <span className="prompt-template-config__shortcut-message">
                  {t('shortcuts.openPickerReminder')}
                  <kbd className="prompt-template-config__shortcut-key">{globalShortcut}</kbd>
                </span>
              }
            />
          </div>

          
          <div className="prompt-template-config__toolbar">
                <div className="prompt-template-config__search">
                  <Search
                    placeholder={t('toolbar.searchPlaceholder')}
                    value={searchQuery}
                    onChange={(val) => setSearchQuery(val)}
                    clearable
                    size="small"
                  />
                </div>

                <div className="prompt-template-config__toolbar-actions">
                  <IconButton
                    variant="primary"
                    size="small"
                    onClick={handleCreateTemplate}
                    tooltip={t('toolbar.createTooltip')}
                  >
                    <Plus size={16} />
                  </IconButton>

                  <IconButton
                    variant="ghost"
                    size="small"
                    onClick={handleExport}
                    tooltip={t('toolbar.exportTooltip')}
                  >
                    <Download size={16} />
                  </IconButton>

                  <IconButton
                    variant="ghost"
                    size="small"
                    onClick={handleImport}
                    tooltip={t('toolbar.importTooltip')}
                  >
                    <Upload size={16} />
                  </IconButton>
                </div>
              </div>

              
              <FilterPillGroup className="prompt-template-config__categories">
                {categories.map(category => (
                  <FilterPill
                    key={category}
                    label={category === 'all' ? t('categories.all') : category}
                    count={category !== 'all' ? templates.filter(tpl => tpl.category === category).length : undefined}
                    active={selectedCategory === category}
                    onClick={() => setSelectedCategory(category)}
                  />
                ))}
              </FilterPillGroup>

              
              <div className="prompt-template-config__templates">
                {sortedTemplates.length === 0 && (
                  <div className="prompt-template-config__empty">
                    <Button
                      variant="dashed"
                      size="medium"
                      onClick={handleCreateTemplate}
                    >
                      <Plus size={16} />
                      {t('empty.createFirst')}
                    </Button>
                  </div>
                )}

                {sortedTemplates.map(template => {
                  const isExpanded = expandedTemplateIds.has(template.id);
                  
                  return (
                    <Card
                      key={template.id}
                      variant="default"
                      padding="none"
                      className={`prompt-template-config__template-card ${isExpanded ? 'is-expanded' : ''}`}
                    >
                      <div 
                        className="prompt-template-config__template-card-header"
                        onClick={() => toggleTemplateExpanded(template.id)}
                      >
                        <div className="prompt-template-config__template-card-title">
                          <IconButton
                            variant={template.isFavorite ? 'primary' : 'ghost'}
                            size="small"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleToggleFavorite(template);
                            }}
                            tooltip={template.isFavorite ? t('actions.unfavorite') : t('actions.favorite')}
                          >
                            <Star size={14} fill={template.isFavorite ? 'currentColor' : 'none'} />
                          </IconButton>
                          <h3>{template.name}</h3>
                          {template.category && (
                            <span className="prompt-template-config__template-card-category">
                              {template.category}
                            </span>
                          )}
                        </div>
                        <div className="prompt-template-config__template-card-actions" onClick={(e) => e.stopPropagation()}>
                          <IconButton
                            variant="ghost"
                            size="small"
                            onClick={() => handleEditTemplate(template)}
                            tooltip={t('actions.edit')}
                          >
                            <Edit2 size={14} />
                          </IconButton>
                          <IconButton
                            variant="ghost"
                            size="small"
                            onClick={() => handleDuplicateTemplate(template)}
                            tooltip={t('actions.copy')}
                          >
                            <Copy size={14} />
                          </IconButton>
                          <IconButton
                            variant="danger"
                            size="small"
                            onClick={() => setDeleteConfirmId(template.id)}
                            tooltip={t('actions.delete')}
                          >
                            <Trash2 size={14} />
                          </IconButton>
                        </div>
                      </div>

                      
                      {isExpanded && (
                        <CardBody className="prompt-template-config__template-card-details">
                          {template.description && (
                            <p className="prompt-template-config__template-card-description">
                              {template.description}
                            </p>
                          )}

                          <div className="prompt-template-config__template-card-content">
                            <div className="prompt-template-config__content-label">{t('template.contentLabel')}</div>
                            <pre>{template.content}</pre>
                          </div>

                          <div className="prompt-template-config__template-card-footer">
                            <span className="prompt-template-config__template-card-usage">
                              {t('template.usageCount', { count: template.usageCount })}
                            </span>
                            {template.shortcut && (
                              <kbd className="prompt-template-config__template-card-shortcut">
                                {template.shortcut}
                              </kbd>
                            )}
                          </div>
                        </CardBody>
                      )}
                    </Card>
                  );
                })}
              </div>
        </div>

        
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
                <Button
                  variant="secondary"
                  size="medium"
                  onClick={() => setIsEditing(false)}
                >
                  {t('modal.actions.cancel')}
                </Button>
                <Button
                  variant="primary"
                  size="medium"
                  onClick={handleSaveTemplate}
                >
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
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default PromptTemplateConfig;

