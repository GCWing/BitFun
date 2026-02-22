import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2, RefreshCw, FolderOpen, X } from 'lucide-react';
import { Switch, Select, Input, Button, IconButton, ConfirmDialog } from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigCollectionItem } from './common';
import { useCurrentWorkspace } from '../../hooks/useWorkspace';
import { useNotification } from '@/shared/notification-system';
import { configAPI } from '../../api/service-api/ConfigAPI';
import type { SkillInfo, SkillLevel, SkillValidationResult } from '../types';
import { open } from '@tauri-apps/plugin-dialog';
import { createLogger } from '@/shared/utils/logger';
import './SkillsConfig.scss';

const log = createLogger('SkillsConfig');

const SkillsConfig: React.FC = () => {
  const { t } = useTranslation('settings/skills');
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
      const skillsList = await configAPI.getSkillConfigs(forceRefresh);
      setSkills(skillsList);
    } catch (err) {
      log.error('Failed to load skills', err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadSkills(); }, [loadSkills]);
  useEffect(() => { if (hasWorkspace) loadSkills(); }, [hasWorkspace, workspacePath, loadSkills]);

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
      notification.success(t('messages.toggleSuccess', { name: skill.name, status: newEnabled ? t('messages.enabled') : t('messages.disabled') }));
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

  const toggleSkillExpanded = (skillId: string) => {
    setExpandedSkillIds(prev => {
      const next = new Set(prev);
      if (next.has(skillId)) next.delete(skillId);
      else next.add(skillId);
      return next;
    });
  };

  const renderAddForm = (level: SkillLevel) => {
    if (!showAddForm || formLevel !== level) return null;
    return (
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
                disabled: !hasWorkspace
              }
            ]}
            value={formLevel}
            onChange={(value) => setFormLevel(value as SkillLevel)}
            size="medium"
          />
          {formLevel === 'project' && hasWorkspace && (
            <div className="bitfun-skills-config__form-hint">
              {t('form.level.currentWorkspace', { path: workspacePath })}
            </div>
          )}
          <div className="bitfun-skills-config__path-input">
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
          <div className="bitfun-skills-config__path-hint">{t('form.path.hint')}</div>
          {isValidating && <div className="bitfun-skills-config__validating">{t('form.validating')}</div>}
          {validationResult && (
            <div className={`bitfun-skills-config__validation ${validationResult.valid ? 'is-valid' : 'is-invalid'}`}>
              {validationResult.valid ? (
                <>
                  <div className="bitfun-skills-config__validation-name">✓ {validationResult.name}</div>
                  <div className="bitfun-skills-config__validation-desc">{validationResult.description}</div>
                </>
              ) : (
                <div className="bitfun-skills-config__validation-error">✗ {validationResult.error}</div>
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
    );
  };

  const renderSkillRow = (skill: SkillInfo) => {
    const badge = (
      <span className="bitfun-collection-item__badge">
        {skill.level === 'user' ? t('list.item.user') : t('list.item.project')}
      </span>
    );
    const control = (
      <>
        <Switch
          checked={skill.enabled}
          onChange={(e) => { e.stopPropagation(); handleToggle(skill); }}
          size="small"
        />
        <button
          type="button"
          className="bitfun-collection-btn bitfun-collection-btn--danger"
          onClick={(e) => { e.stopPropagation(); setDeleteConfirm({ show: true, skill }); }}
          title={t('list.item.deleteTooltip')}
        >
          <Trash2 size={14} />
        </button>
      </>
    );
    const details = (
      <>
        <div className="bitfun-collection-details__field">{skill.description}</div>
        <div className="bitfun-collection-details__meta">
          <span className="bitfun-collection-details__label">{t('list.item.pathLabel')}</span>
          <code className="bitfun-skills-config__path-value">{skill.path}</code>
        </div>
      </>
    );
    return (
      <ConfigCollectionItem
        key={skill.name}
        label={skill.name}
        badge={badge}
        control={control}
        details={details}
        disabled={!skill.enabled}
        expanded={expandedSkillIds.has(skill.name)}
        onToggle={() => toggleSkillExpanded(skill.name)}
      />
    );
  };

  const refreshExtra = (
    <IconButton
      variant="ghost"
      size="small"
      onClick={() => loadSkills(true)}
      tooltip={t('toolbar.refreshTooltip')}
    >
      <RefreshCw size={16} />
    </IconButton>
  );

  const makeAddExtra = (level: SkillLevel) => (
    <>
      {level === 'user' && refreshExtra}
      <IconButton
        variant="primary"
        size="small"
        onClick={() => { setFormLevel(level); setShowAddForm(true); }}
        tooltip={t('toolbar.addTooltip')}
        disabled={level === 'project' && !hasWorkspace}
      >
        <Plus size={16} />
      </IconButton>
    </>
  );

  if (loading) {
    return (
      <ConfigPageLayout className="bitfun-skills-config">
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <div className="bitfun-collection-empty"><p>{t('list.loading')}</p></div>
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  if (error) {
    return (
      <ConfigPageLayout className="bitfun-skills-config">
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <div className="bitfun-collection-empty"><p>{t('list.errorPrefix')}{error}</p></div>
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  const userSkills = skills.filter(s => s.level === 'user');
  const projectSkills = skills.filter(s => s.level === 'project');

  return (
    <ConfigPageLayout className="bitfun-skills-config">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />

      <ConfigPageContent>
        <ConfigPageSection
          title={t('filters.user', { defaultValue: '用户技能' })}
          description={t('section.user.description', { defaultValue: '当前用户安装的技能。' })}
          extra={makeAddExtra('user')}
        >
          {renderAddForm('user')}
          {userSkills.length === 0 && !(showAddForm && formLevel === 'user') ? (
            <div className="bitfun-collection-empty">
              <Button variant="dashed" size="small" onClick={() => { setFormLevel('user'); setShowAddForm(true); }}>
                <Plus size={14} />
                {t('toolbar.addTooltip')}
              </Button>
            </div>
          ) : userSkills.map(renderSkillRow)}
        </ConfigPageSection>

        <ConfigPageSection
          title={t('filters.project', { defaultValue: '项目技能' })}
          description={t('section.project.description', { defaultValue: '当前工作区的项目级技能。' })}
          extra={makeAddExtra('project')}
        >
          {renderAddForm('project')}
          {projectSkills.length === 0 && !(showAddForm && formLevel === 'project') ? (
            <div className="bitfun-collection-empty">
              {!hasWorkspace && <p>{t('messages.noWorkspace')}</p>}
              {hasWorkspace && (
                <Button variant="dashed" size="small" onClick={() => { setFormLevel('project'); setShowAddForm(true); }}>
                  <Plus size={14} />
                  {t('toolbar.addTooltip')}
                </Button>
              )}
            </div>
          ) : projectSkills.map(renderSkillRow)}
        </ConfigPageSection>
      </ConfigPageContent>

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
    </ConfigPageLayout>
  );
};

export default SkillsConfig;
