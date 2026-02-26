import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2, RefreshCw, FolderOpen, X, Download, CheckCircle2, TrendingUp } from 'lucide-react';
import { Switch, Select, Input, Button, Search, IconButton, ConfirmDialog, Card, CardBody, Tooltip } from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigCollectionItem } from './common';
import { useCurrentWorkspace } from '../../hooks/useWorkspace';
import { useNotification } from '@/shared/notification-system';
import { configAPI } from '../../api/service-api/ConfigAPI';
import type { SkillInfo, SkillLevel, SkillMarketItem, SkillValidationResult } from '../types';
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

  const [marketKeyword, setMarketKeyword] = useState('');
  const [marketSkills, setMarketSkills] = useState<SkillMarketItem[]>([]);
  const [marketLoading, setMarketLoading] = useState(false);
  const [marketError, setMarketError] = useState<string | null>(null);
  const [downloadingPackage, setDownloadingPackage] = useState<string | null>(null);

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

  const loadMarketSkills = useCallback(async (query?: string) => {
    try {
      setMarketLoading(true);
      setMarketError(null);
      const normalized = query?.trim();
      const skillList = normalized
        ? await configAPI.searchSkillMarket(normalized, 20)
        : await configAPI.listSkillMarket(undefined, 20);
      setMarketSkills(skillList);
    } catch (err) {
      log.error('Failed to load skill market', err);
      setMarketError(err instanceof Error ? err.message : String(err));
    } finally {
      setMarketLoading(false);
    }
  }, []);

  useEffect(() => { loadSkills(); }, [loadSkills]);
  useEffect(() => { if (hasWorkspace) loadSkills(); }, [hasWorkspace, workspacePath, loadSkills]);
  useEffect(() => { loadMarketSkills(); }, [loadMarketSkills]);

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

  const handleDownload = async (skill: SkillMarketItem) => {
    if (!hasWorkspace) {
      notification.warning(t('messages.noWorkspace'));
      return;
    }

    try {
      setDownloadingPackage(skill.installId);
      const result = await configAPI.downloadSkillMarket(skill.installId, 'project');
      const installedName = result.installedSkills[0] ?? skill.name;
      notification.success(t('messages.marketDownloadSuccess', { name: installedName }));
      await loadSkills(true);
    } catch (err) {
      notification.error(t('messages.marketDownloadFailed', { error: err instanceof Error ? err.message : String(err) }));
    } finally {
      setDownloadingPackage(null);
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

  const renderMarketList = () => {
    if (marketLoading) {
      return (
        <div className="bitfun-skills-config__market-list" aria-busy="true" aria-label={t('market.loading')}>
          {Array.from({ length: 5 }).map((_, index) => (
            <Card
              key={`market-loading-${index}`}
              variant="elevated"
              padding="none"
              className="bitfun-skills-config__market-item is-loading"
            >
              <CardBody className="bitfun-skills-config__market-item-body">
                <div className="bitfun-skills-config__market-skeleton-main">
                  <div className="bitfun-skills-config__market-skeleton-line bitfun-skills-config__market-skeleton-line--title" />
                  <div className="bitfun-skills-config__market-skeleton-line bitfun-skills-config__market-skeleton-line--desc" />
                  <div className="bitfun-skills-config__market-skeleton-line bitfun-skills-config__market-skeleton-line--desc is-short" />
                  <div className="bitfun-skills-config__market-skeleton-chip" />
                </div>
                <div className="bitfun-skills-config__market-skeleton-btn" />
              </CardBody>
            </Card>
          ))}
        </div>
      );
    }

    if (marketError) {
      return <div className="bitfun-skills-config__market-state bitfun-skills-config__market-state--error">{t('market.errorPrefix')}{marketError}</div>;
    }

    if (marketSkills.length === 0) {
      return (
        <div className="bitfun-skills-config__market-state">
          {marketKeyword.trim() ? t('market.empty.noMatch') : t('market.empty.noSkills')}
        </div>
      );
    }

    return (
      <div className="bitfun-skills-config__market-list">
        {displayMarketSkills.map((skill) => {
          const isDownloading = downloadingPackage === skill.installId;
          const isInstalled = installedSkillNames.has(skill.name);
          const sourceLabel = formatMarketSource(skill.source);
          const tooltipText = !hasWorkspace
            ? t('messages.noWorkspace')
            : isInstalled
              ? t('market.item.installedTooltip')
              : t('market.item.downloadProject');

          return (
            <Card
              key={skill.installId}
              variant="elevated"
              padding="none"
              className={`bitfun-skills-config__market-item${isInstalled ? ' is-installed' : ''}`}
            >
              <CardBody className="bitfun-skills-config__market-item-body">
                <div className="bitfun-skills-config__market-item-main">
                  <div className="bitfun-skills-config__market-item-head">
                    <div className="bitfun-skills-config__market-item-name-wrap">
                      <div className="bitfun-skills-config__market-item-name">{skill.name}</div>
                      {isInstalled ? (
                        <span className="bitfun-skills-config__market-item-badge bitfun-skills-config__market-item-badge--installed">
                          <CheckCircle2 size={12} />
                          {t('market.item.installed')}
                        </span>
                      ) : null}
                    </div>
                    <span className="bitfun-skills-config__market-item-installs">
                      <TrendingUp size={12} />
                      {t('market.item.installs', { count: skill.installs })}
                    </span>
                  </div>
                  <div className="bitfun-skills-config__market-item-description">
                    {skill.description?.trim() || t('market.item.noDescription')}
                  </div>
                  <div className="bitfun-skills-config__market-item-meta">
                    {skill.source ? (
                      sourceLabel !== skill.source ? (
                        <Tooltip content={skill.source}>
                          <span className="bitfun-skills-config__market-item-chip bitfun-skills-config__market-item-source">
                            {t('market.item.sourceLabel')}{sourceLabel}
                          </span>
                        </Tooltip>
                      ) : (
                        <span className="bitfun-skills-config__market-item-chip bitfun-skills-config__market-item-source">
                          {t('market.item.sourceLabel')}{sourceLabel}
                        </span>
                      )
                    ) : null}
                  </div>
                </div>

                <div className="bitfun-skills-config__market-item-action">
                  <Tooltip content={tooltipText}>
                    <span>
                      <Button
                        variant="primary"
                        size="small"
                        onClick={() => handleDownload(skill)}
                        disabled={isDownloading || !hasWorkspace || isInstalled}
                      >
                        <Download size={14} />
                        {isDownloading
                          ? t('market.item.downloading')
                          : isInstalled
                            ? t('market.item.installed')
                            : t('market.item.downloadProject')}
                      </Button>
                    </span>
                  </Tooltip>
                </div>
              </CardBody>
            </Card>
          );
        })}
      </div>
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

  const installedSkillNames = useMemo(
    () => new Set(skills.map((skill) => skill.name)),
    [skills]
  );

  const formatMarketSource = useCallback((source: string): string => {
    const raw = source.trim();
    if (!raw) return raw;

    const compact = raw
      .replace(/^https?:\/\//i, '')
      .replace(/^www\./i, '')
      .replace(/\/+$/, '');

    const parts = compact.split('/').filter(Boolean);
    if (parts.length === 0) return raw;
    if (parts.length === 1) return parts[0];

    if (parts[0].includes('.')) {
      return parts.slice(0, 2).join('/');
    }

    return parts.slice(0, 2).join('/');
  }, []);

  const displayMarketSkills = useMemo(() => {
    const entries = marketSkills.map((skill, index) => ({
      skill,
      index,
      installed: installedSkillNames.has(skill.name),
    }));

    entries.sort((a, b) => {
      if (a.installed !== b.installed) {
        return a.installed ? -1 : 1;
      }

      const installDelta = (b.skill.installs ?? 0) - (a.skill.installs ?? 0);
      if (installDelta !== 0) {
        return installDelta;
      }

      return a.index - b.index;
    });

    return entries.map((entry) => entry.skill);
  }, [marketSkills, installedSkillNames]);

  const handleMarketSearch = useCallback(() => {
    loadMarketSkills(marketKeyword);
  }, [loadMarketSkills, marketKeyword]);

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
          title={t('market.title')}
          description={t('market.subtitle')}
          extra={(
            <IconButton
              variant="ghost"
              size="small"
              onClick={() => loadMarketSkills(marketKeyword)}
              tooltip={t('market.refreshTooltip')}
            >
              <RefreshCw size={16} />
            </IconButton>
          )}
        >
          <div className="bitfun-skills-config__market-toolbar">
            <Search
              placeholder={t('market.searchPlaceholder')}
              value={marketKeyword}
              onChange={(value) => setMarketKeyword(value)}
              onSearch={handleMarketSearch}
              showSearchButton
              clearable
              size="small"
            />
          </div>
          {renderMarketList()}
        </ConfigPageSection>

        <ConfigPageSection
          title={t('filters.user', { defaultValue: 'User Skills' })}
          description={t('section.user.description', { defaultValue: 'Skills installed for current user.' })}
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
          title={t('filters.project', { defaultValue: 'Project Skills' })}
          description={t('section.project.description', { defaultValue: 'Project-level Skills in current workspace.' })}
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
