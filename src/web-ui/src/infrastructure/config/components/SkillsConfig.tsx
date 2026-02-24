 

import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2, RefreshCw, FolderOpen, X } from 'lucide-react';
import { Switch, Select, Input, Button, Search, IconButton, Card, CardBody, FilterPill, FilterPillGroup, ConfirmDialog } from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent } from './common';
import { useCurrentWorkspace } from '../../hooks/useWorkspace';
import { useNotification } from '@/shared/notification-system';
import { configAPI } from '../../api/service-api/ConfigAPI';
import type { SkillInfo, SkillLevel, SkillValidationResult } from '../types';
import { open } from '@tauri-apps/plugin-dialog';
import { createLogger } from '@/shared/utils/logger';
import './SkillsConfig.scss';

const log = createLogger('SkillsConfig');

type FilterLevel = 'all' | 'user' | 'project';

const SkillsConfig: React.FC = () => {
  const { t } = useTranslation('settings/skills');
  const [filterLevel, setFilterLevel] = useState<FilterLevel>('all');
  const [searchKeyword, setSearchKeyword] = useState('');
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

  useEffect(() => {
    loadSkills();
  }, [loadSkills]);
  
  
  useEffect(() => {
    if (hasWorkspace) {
      loadSkills();
    }
  }, [hasWorkspace, workspacePath, loadSkills]);
  
  
  const filteredSkills = skills.filter(skill => {
    
    if (filterLevel === 'user' && skill.level !== 'user') {
      return false;
    }
    if (filterLevel === 'project' && skill.level !== 'project') {
      return false;
    }

    
    if (searchKeyword) {
      const keyword = searchKeyword.toLowerCase();
      return (
        skill.name.toLowerCase().includes(keyword) ||
        skill.description.toLowerCase().includes(keyword) ||
        skill.path.toLowerCase().includes(keyword)
      );
    }

    return true;
  });

  const validatePath = useCallback(async (path: string) => {
    if (!path.trim()) {
      setValidationResult(null);
      return;
    }
    
    try {
      setIsValidating(true);
      const result = await configAPI.validateSkillPath(path);
      setValidationResult(result);
    } catch (err) {
      setValidationResult({
        valid: false,
        error: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setIsValidating(false);
    }
  }, []);
  
  
  useEffect(() => {
    const timer = setTimeout(() => {
      validatePath(formPath);
    }, 300);
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
  
  
  const showDeleteConfirm = (skill: SkillInfo) => {
    setDeleteConfirm({ show: true, skill });
  };
  
  
  const cancelDelete = () => {
    setDeleteConfirm({ show: false, skill: null });
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
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('form.path.label'),
      });
      
      if (selected) {
        setFormPath(selected as string);
      }
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
      if (next.has(skillId)) {
        next.delete(skillId);
      } else {
        next.add(skillId);
      }
      return next;
    });
  };
  
  
  const renderAddForm = () => {
    if (!showAddForm) return null;
    
    return (
      <div className="bitfun-skills-config__form">
        <div className="bitfun-skills-config__form-header">
          <h3>{t('form.title')}</h3>
          <IconButton variant="ghost" size="small" onClick={resetForm} tooltip={t('form.closeTooltip')}>
            <X size={14} />
          </IconButton>
        </div>
        
        <div className="bitfun-skills-config__form-body">
          
          <div className="bitfun-skills-config__form-group">
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
              <div style={{ fontSize: '12px', color: '#a0a0a0', marginTop: '4px' }}>
                {t('form.level.currentWorkspace', { path: workspacePath })}
              </div>
            )}
          </div>
          
          
          <div className="bitfun-skills-config__path-input">
            <Input
              label={t('form.path.label')}
              placeholder={t('form.path.placeholder')}
              value={formPath}
              onChange={(e) => setFormPath(e.target.value)}
              variant="outlined"
            />
            <IconButton
              variant="default"
              size="medium"
              onClick={handleBrowse}
              tooltip={t('form.path.browseTooltip')}
            >
              <FolderOpen size={16} />
            </IconButton>
          </div>
          <div className="bitfun-skills-config__path-hint">
            {t('form.path.hint')}
          </div>
          
          
          {isValidating && (
            <div className="bitfun-skills-config__validating">{t('form.validating')}</div>
          )}
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
        
        <div className="bitfun-skills-config__form-footer">
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

  const renderSkillsList = () => {
    if (loading) {
      return <div className="bitfun-skills-config__loading">{t('list.loading')}</div>;
    }
    
    if (error) {
      return <div className="bitfun-skills-config__error">{t('list.errorPrefix')}{error}</div>;
    }
    
    if (filteredSkills.length === 0) {
      return (
        <div className="bitfun-skills-config__empty">
          {searchKeyword ? t('list.empty.noMatch') : t('list.empty.noSkills')}
        </div>
      );
    }
    
    return (
      <div className="bitfun-skills-config__list">
        {filteredSkills.map((skill) => {
          const isExpanded = expandedSkillIds.has(skill.name);
          
          return (
            <Card
              key={skill.name}
              variant="default"
              padding="none"
              className={`bitfun-skills-config__item ${!skill.enabled ? 'is-disabled' : ''} ${isExpanded ? 'is-expanded' : ''}`}
            >
              
              <div
                className="bitfun-skills-config__item-header"
                onClick={() => toggleSkillExpanded(skill.name)}
              >
                <div className="bitfun-skills-config__item-main">
                  <div className="bitfun-skills-config__item-name">{skill.name}</div>
                  <div className="bitfun-skills-config__item-badges">
                    <span className={`bitfun-skills-config__badge bitfun-skills-config__badge--${skill.level}`}>
                      {skill.level === 'user' ? t('list.item.user') : t('list.item.project')}
                    </span>
                  </div>
                </div>
                <div className="bitfun-skills-config__item-actions" onClick={(e) => e.stopPropagation()}>
                  <Switch
                    checked={skill.enabled}
                    onChange={(e) => {
                      e.stopPropagation();
                      handleToggle(skill);
                    }}
                    size="small"
                  />
                  <IconButton
                    variant="danger"
                    size="small"
                    onClick={(e) => {
                      e.stopPropagation();
                      showDeleteConfirm(skill);
                    }}
                    tooltip={t('list.item.deleteTooltip')}
                  >
                    <Trash2 size={14} />
                  </IconButton>
                </div>
              </div>
              
              
              {isExpanded && (
                <CardBody className="bitfun-skills-config__item-details">
                  <div className="bitfun-skills-config__item-description">{skill.description}</div>
                  <div className="bitfun-skills-config__item-path">
                    <span className="bitfun-skills-config__path-label">{t('list.item.pathLabel')}</span>
                    <span className="bitfun-skills-config__path-value">{skill.path}</span>
                  </div>
                </CardBody>
              )}
            </Card>
          );
        })}
      </div>
    );
  };
  
  
  const userSkills = skills.filter(s => s.level === 'user');
  const projectSkills = skills.filter(s => s.level === 'project');
  const userEnabledCount = userSkills.filter(s => s.enabled).length;
  const projectEnabledCount = projectSkills.filter(s => s.enabled).length;
  const totalEnabledCount = userEnabledCount + projectEnabledCount;
  
  return (
    <ConfigPageLayout className="bitfun-skills-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />
      
      <ConfigPageContent className="bitfun-skills-config__content">
        
        {renderAddForm()}

        <div className="bitfun-skills-config__section">
          
          <div className="bitfun-skills-config__toolbar">
            <div className="bitfun-skills-config__search-box">
              <Search
                placeholder={t('toolbar.searchPlaceholder')}
                value={searchKeyword}
                onChange={(val) => setSearchKeyword(val)}
                clearable
                size="small"
              />
            </div>
            <IconButton 
              variant="default" 
              size="small"
              onClick={() => loadSkills(true)}
              tooltip={t('toolbar.refreshTooltip')}
            >
              <RefreshCw size={16} />
            </IconButton>
            <IconButton 
              variant="primary" 
              size="small"
              onClick={() => setShowAddForm(true)}
              tooltip={t('toolbar.addTooltip')}
            >
              <Plus size={16} />
            </IconButton>
          </div>
          
          
          <FilterPillGroup className="bitfun-skills-config__filters">
            <FilterPill
              label={t('filters.all')}
              count={`${totalEnabledCount}/${skills.length}`}
              active={filterLevel === 'all'}
              onClick={() => setFilterLevel('all')}
            />
            <FilterPill
              label={t('filters.user')}
              count={`${userEnabledCount}/${userSkills.length}`}
              active={filterLevel === 'user'}
              onClick={() => setFilterLevel('user')}
            />
            <FilterPill
              label={t('filters.project')}
              count={`${projectEnabledCount}/${projectSkills.length}`}
              active={filterLevel === 'project'}
              onClick={() => setFilterLevel('project')}
            />
          </FilterPillGroup>
        </div>
        
        
        {renderSkillsList()}
        
        
        <ConfirmDialog
          isOpen={deleteConfirm.show && !!deleteConfirm.skill}
          onClose={cancelDelete}
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
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default SkillsConfig;
