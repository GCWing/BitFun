/**
 * InstalledView — installed skills list with filter chips.
 * List layout mirrors MarketView for visual consistency.
 * Inline "Add Skill" form toggled by the header "+" button.
 */

import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2, RefreshCw, FolderOpen, X, Package } from 'lucide-react';
import { Switch, Select, Input, Button, IconButton, ConfirmDialog, Badge } from '@/component-library';
import { useCurrentWorkspace } from '@/infrastructure/hooks/useWorkspace';
import { useNotification } from '@/shared/notification-system';
import { configAPI } from '@/infrastructure/api';
import type { SkillInfo, SkillLevel, SkillValidationResult } from '@/infrastructure/config/types';
import { open } from '@tauri-apps/plugin-dialog';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('SkillsScene:InstalledView');

type FilterType = 'all' | 'user' | 'project';

const FILTERS: { id: FilterType; labelKey: string }[] = [
  { id: 'all',     labelKey: 'filters.all'     },
  { id: 'user',    labelKey: 'filters.user'    },
  { id: 'project', labelKey: 'filters.project' },
];

const InstalledView: React.FC = () => {
  const { t } = useTranslation('scenes/skills');

  const [activeFilter, setActiveFilter] = useState<FilterType>('all');
  const [showAddForm, setShowAddForm] = useState(false);
  const [expandedSkillIds, setExpandedSkillIds] = useState<Set<string>>(new Set());
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [formLevel, setFormLevel] = useState<SkillLevel>('user');
  const [formPath, setFormPath] = useState('');
  const [validationResult, setValidationResult] = useState<SkillValidationResult | null>(null);
  const [isValidating, setIsValidating] = useState(false);
  const [isAdding, setIsAdding] = useState(false);

  const [deleteConfirm, setDeleteConfirm] = useState<{ show: boolean; skill: SkillInfo | null }>({
    show: false,
    skill: null,
  });

  const { workspacePath, hasWorkspace } = useCurrentWorkspace();
  const notification = useNotification();

  const loadSkills = useCallback(async (forceRefresh?: boolean) => {
    try {
      setLoading(true);
      setError(null);
      const list = await configAPI.getSkillConfigs(forceRefresh);
      setSkills(list);
    } catch (err) {
      log.error('Failed to load skills', err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadSkills(); }, [loadSkills]);

  const validatePath = useCallback(async (path: string) => {
    if (!path.trim()) { setValidationResult(null); return; }
    try {
      setIsValidating(true);
      const result = await configAPI.validateSkillPath(path);
      setValidationResult(result);
    } catch (err) {
      setValidationResult({ valid: false, error: err instanceof Error ? err.message : String(err) });
    } finally {
      setIsValidating(false);
    }
  }, []);

  useEffect(() => {
    const timer = setTimeout(() => { validatePath(formPath); }, 300);
    return () => clearTimeout(timer);
  }, [formPath, validatePath]);

  const handleAdd = async () => {
    if (!validationResult?.valid || !formPath.trim()) {
      notification.warning(t('messages.invalidPath'));
      return;
    }
    if (formLevel === 'project' && !hasWorkspace) {
      notification.warning(t('messages.noWorkspace'));
      return;
    }
    try {
      setIsAdding(true);
      await configAPI.addSkill(formPath, formLevel);
      notification.success(t('messages.addSuccess', { name: validationResult.name }));
      resetForm();
      loadSkills();
    } catch (err) {
      notification.error(t('messages.addFailed', { error: err instanceof Error ? err.message : String(err) }));
    } finally {
      setIsAdding(false);
    }
  };

  const confirmDelete = async () => {
    const skill = deleteConfirm.skill;
    if (!skill) return;
    try {
      await configAPI.deleteSkill(skill.name);
      notification.success(t('messages.deleteSuccess', { name: skill.name }));
      loadSkills();
    } catch (err) {
      notification.error(t('messages.deleteFailed', { error: err instanceof Error ? err.message : String(err) }));
    } finally {
      setDeleteConfirm({ show: false, skill: null });
    }
  };

  const handleToggle = async (skill: SkillInfo) => {
    const newEnabled = !skill.enabled;
    try {
      await configAPI.setSkillEnabled(skill.name, newEnabled);
      notification.success(t('messages.toggleSuccess', {
        name: skill.name,
        status: newEnabled ? t('messages.enabled') : t('messages.disabled'),
      }));
      loadSkills();
    } catch (err) {
      notification.error(t('messages.toggleFailed', { error: err instanceof Error ? err.message : String(err) }));
    }
  };

  const handleBrowse = async () => {
    try {
      const selected = await open({ directory: true, multiple: false, title: t('form.path.label') });
      if (selected) setFormPath(selected as string);
    } catch (err) {
      log.error('Failed to open file dialog', err);
    }
  };

  const resetForm = () => {
    setFormPath('');
    setFormLevel('user');
    setValidationResult(null);
    setShowAddForm(false);
  };

  const toggleSkillExpanded = useCallback((skillId: string) => {
    setExpandedSkillIds(prev => {
      const next = new Set(prev);
      next.has(skillId) ? next.delete(skillId) : next.add(skillId);
      return next;
    });
  }, []);

  const filteredSkills = useMemo(() => {
    if (activeFilter === 'all') return skills;
    return skills.filter(s => s.level === activeFilter);
  }, [skills, activeFilter]);

  const renderSkeletonList = () => (
    <div className="bitfun-market__list" aria-busy="true">
      {Array.from({ length: 5 }).map((_, i) => (
        <div
          key={i}
          className="bitfun-market__list-item bitfun-market__list-item--skeleton"
          style={{ '--item-index': i } as React.CSSProperties}
        >
          <div className="bitfun-market__list-item-row">
            <div className="bitfun-market__list-item-info">
              <div className="bitfun-market__skeleton-line bitfun-market__skeleton-line--title" />
              <div className="bitfun-market__skeleton-line bitfun-market__skeleton-line--body" />
            </div>
            <div className="bitfun-market__list-item-meta">
              <div className="bitfun-market__skeleton-chip bitfun-market__skeleton-chip--sm" />
            </div>
            <div className="bitfun-market__list-item-action">
              <div className="bitfun-installed__skeleton-actions" />
            </div>
          </div>
        </div>
      ))}
    </div>
  );

  const renderList = () => {
    if (loading) return renderSkeletonList();

    if (error) {
      return (
        <div className="bitfun-market__empty bitfun-market__empty--error">
          <Package size={32} strokeWidth={1.5} />
          <span>{t('list.errorPrefix')}{error}</span>
        </div>
      );
    }

    if (filteredSkills.length === 0) {
      return (
        <div className="bitfun-market__empty">
          <Package size={32} strokeWidth={1.5} />
          <span>{skills.length === 0 ? t('list.empty.noSkills') : t('list.empty.noMatch')}</span>
          {skills.length === 0 && (
            <Button variant="primary" size="small" onClick={() => setShowAddForm(true)}>
              <Plus size={13} />
              {t('toolbar.addTooltip')}
            </Button>
          )}
        </div>
      );
    }

    return (
      <div className="bitfun-market__list">
        {filteredSkills.map((skill, index) => {
          const isExpanded = expandedSkillIds.has(skill.name);
          return (
            <div
              key={skill.name}
              className={[
                'bitfun-market__list-item',
                !skill.enabled && 'is-disabled',
                isExpanded && 'is-expanded',
              ].filter(Boolean).join(' ')}
              style={{ '--item-index': index } as React.CSSProperties}
            >
              <div
                className="bitfun-market__list-item-row"
                onClick={() => toggleSkillExpanded(skill.name)}
                role="button"
                tabIndex={0}
                onKeyDown={(e) => e.key === 'Enter' && toggleSkillExpanded(skill.name)}
              >
                <div className="bitfun-market__list-item-info">
                  <div className="bitfun-market__card-name-row">
                    <span className="bitfun-market__card-name">{skill.name}</span>
                    <Badge variant={skill.level === 'user' ? 'info' : 'purple'}>
                      {skill.level === 'user' ? t('list.item.user') : t('list.item.project')}
                    </Badge>
                    {!skill.enabled && (
                      <Badge variant="neutral">{t('messages.disabled')}</Badge>
                    )}
                  </div>
                  <p className="bitfun-market__list-item-desc">
                    {skill.description?.trim() || '—'}
                  </p>
                </div>

                <div className="bitfun-market__list-item-meta" />

                <div
                  className="bitfun-market__list-item-action bitfun-installed__item-actions"
                  onClick={(e) => e.stopPropagation()}
                >
                  <Switch
                    checked={skill.enabled}
                    onChange={() => handleToggle(skill)}
                    size="small"
                  />
                  <button
                    type="button"
                    className="bitfun-installed__delete-btn"
                    onClick={() => setDeleteConfirm({ show: true, skill })}
                    title={t('list.item.deleteTooltip')}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>

              {isExpanded && (
                <div className="bitfun-market__list-item-details">
                  <div className="bitfun-market__detail-row">
                    <span className="bitfun-market__detail-label">{t('list.item.pathLabel')}</span>
                    <code className="bitfun-market__detail-value">{skill.path}</code>
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    );
  };

  return (
    <div className="bitfun-skills-scene__view">
      <div className="bitfun-skills-scene__view-header">
        <div className="bitfun-skills-scene__view-header-inner">
          <div className="bitfun-skills-scene__view-title-row">
            <div>
              <h2 className="bitfun-skills-scene__view-title">{t('installed.titleAll')}</h2>
              <p className="bitfun-skills-scene__view-subtitle">{t('installed.subtitleAll')}</p>
            </div>
            <div className="bitfun-installed__header-actions">
              <IconButton
                variant="ghost"
                size="small"
                onClick={() => loadSkills(true)}
                tooltip={t('toolbar.refreshTooltip')}
              >
                <RefreshCw size={16} />
              </IconButton>
              <IconButton
                variant="primary"
                size="small"
                onClick={() => setShowAddForm(v => !v)}
                tooltip={t('toolbar.addTooltip')}
              >
                <Plus size={16} />
              </IconButton>
            </div>
          </div>

          <div className="bitfun-installed__filter-bar">
            {FILTERS.map(({ id, labelKey }) => (
              <button
                key={id}
                type="button"
                className={[
                  'bitfun-installed__filter-chip',
                  activeFilter === id && 'is-active',
                ].filter(Boolean).join(' ')}
                onClick={() => setActiveFilter(id)}
              >
                {t(labelKey)}
                {id !== 'all' && (
                  <span className="bitfun-installed__filter-count">
                    {skills.filter(s => s.level === id).length}
                  </span>
                )}
              </button>
            ))}
          </div>
        </div>
      </div>

      <div className="bitfun-skills-scene__view-content">
        <div className="bitfun-skills-scene__view-content-inner">
          {showAddForm && (
            <div className="bitfun-collection-form">
              <div className="bitfun-collection-form__header">
                <h3>{t('form.title')}</h3>
                <IconButton variant="ghost" size="small" onClick={resetForm} tooltip={t('form.closeTooltip')}>
                  <X size={14} />
                </IconButton>
              </div>
              <div className="bitfun-collection-form__body">
                <Select
                  label={t('form.level.label')}
                  options={[
                    { label: t('form.level.user'), value: 'user' },
                    {
                      label: `${t('form.level.project')}${!hasWorkspace ? t('form.level.projectDisabled') : ''}`,
                      value: 'project',
                      disabled: !hasWorkspace,
                    },
                  ]}
                  value={formLevel}
                  onChange={(value) => setFormLevel(value as SkillLevel)}
                  size="medium"
                />
                {formLevel === 'project' && hasWorkspace && (
                  <div className="bitfun-skills-scene__form-hint">
                    {t('form.level.currentWorkspace', { path: workspacePath })}
                  </div>
                )}
                <div className="bitfun-skills-scene__path-input">
                  <Input
                    label={t('form.path.label')}
                    placeholder={t('form.path.placeholder')}
                    value={formPath}
                    onChange={(e) => setFormPath(e.target.value)}
                    variant="outlined"
                  />
                  <IconButton variant="default" size="medium" onClick={handleBrowse} tooltip={t('form.path.browseTooltip')}>
                    <FolderOpen size={16} />
                  </IconButton>
                </div>
                <div className="bitfun-skills-scene__path-hint">{t('form.path.hint')}</div>
                {isValidating && (
                  <div className="bitfun-skills-scene__validating">{t('form.validating')}</div>
                )}
                {validationResult && (
                  <div className={`bitfun-skills-scene__validation ${validationResult.valid ? 'is-valid' : 'is-invalid'}`}>
                    {validationResult.valid ? (
                      <>
                        <div className="bitfun-skills-scene__validation-name">✓ {validationResult.name}</div>
                        <div className="bitfun-skills-scene__validation-desc">{validationResult.description}</div>
                      </>
                    ) : (
                      <div className="bitfun-skills-scene__validation-error">✗ {validationResult.error}</div>
                    )}
                  </div>
                )}
              </div>
              <div className="bitfun-collection-form__footer">
                <Button variant="secondary" size="small" onClick={resetForm}>
                  {t('form.actions.cancel')}
                </Button>
                <Button
                  variant="primary"
                  size="small"
                  onClick={handleAdd}
                  disabled={!validationResult?.valid || isAdding}
                >
                  {isAdding ? t('form.actions.adding') : t('form.actions.add')}
                </Button>
              </div>
            </div>
          )}

          {renderList()}
        </div>
      </div>

      <ConfirmDialog
        isOpen={deleteConfirm.show && !!deleteConfirm.skill}
        onClose={() => setDeleteConfirm({ show: false, skill: null })}
        onConfirm={confirmDelete}
        title={t('deleteModal.title')}
        message={
          <>
            <p>{t('deleteModal.message', { name: deleteConfirm.skill?.name })}</p>
            <p style={{ marginTop: '8px', color: 'var(--color-warning)' }}>{t('deleteModal.warning')}</p>
          </>
        }
        type="warning"
        confirmDanger
        confirmText={t('deleteModal.delete')}
        cancelText={t('deleteModal.cancel')}
      />
    </div>
  );
};

export default InstalledView;
