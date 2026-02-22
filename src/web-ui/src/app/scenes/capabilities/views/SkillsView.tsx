/**
 * SkillsView — skills card list with toggle and expand details (path copy).
 */

import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Puzzle, Check, Copy, RefreshCw } from 'lucide-react';
import { Switch, Card, CardBody } from '@/component-library';
import { configAPI } from '@/infrastructure/api';
import type { SkillInfo } from '@/infrastructure/config/types';
import { useNotification } from '@/shared/notification-system';
import './capabilities-views.scss';

const SkillsView: React.FC = () => {
  const { t } = useTranslation('scenes/capabilities');
  const { error: notifyError, success: notifySuccess } = useNotification();
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(() => new Set());
  const [copiedPath, setCopiedPath] = useState<string | null>(null);

  const load = useCallback(
    async (silent = false) => {
      try {
        const list = await configAPI.getSkillConfigs();
        setSkills(list);
      } catch (err) {
        if (!silent) notifyError(t('loadFailed'));
      }
    },
    [notifyError, t]
  );

  useEffect(() => {
    load();
  }, [load]);

  const handleRefresh = useCallback(async () => {
    setIsRefreshing(true);
    try {
      await load(true);
    } finally {
      setIsRefreshing(false);
    }
  }, [load]);

  const handleCopyPath = useCallback(
    async (path: string) => {
      try {
        await navigator.clipboard.writeText(path);
        setCopiedPath(path);
        notifySuccess(t('pathCopied'));
        setTimeout(() => setCopiedPath(null), 2000);
      } catch {
        notifyError(t('pathCopyFailed'));
      }
    },
    [notifySuccess, notifyError, t]
  );

  const toggleExpanded = useCallback((id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const handleToggle = useCallback(
    async (skill: SkillInfo) => {
      try {
        await configAPI.setSkillEnabled(skill.name, !skill.enabled);
        await load(true);
      } catch {
        notifyError(t('toggleFailed'));
      }
    },
    [load, notifyError, t]
  );

  return (
    <div className="bitfun-cap__view">
      <div className="bitfun-cap__content">
        {skills.length === 0 ? (
          <div className="bitfun-cap__empty">
            <span>{t('emptySkills')}</span>
          </div>
        ) : (
          <div className="bitfun-cap__cards-grid">
            {skills.map((skill) => {
              const isExpanded = expandedIds.has(`skill:${skill.name}`);
              return (
                <Card
                  key={skill.name}
                  variant="default"
                  padding="none"
                  className={`bitfun-cap__card ${!skill.enabled ? 'is-disabled' : ''} ${isExpanded ? 'is-expanded' : ''}`}
                >
                  <div
                    className="bitfun-cap__card-header"
                    onClick={() => toggleExpanded(`skill:${skill.name}`)}
                  >
                    <div className="bitfun-cap__card-icon bitfun-cap__card-icon--skill">
                      <Puzzle size={13} />
                    </div>
                    <div className="bitfun-cap__card-info">
                      <span className="bitfun-cap__card-name">{skill.name}</span>
                      <span className="bitfun-cap__badge bitfun-cap__badge--purple">{skill.level}</span>
                    </div>
                    <div className="bitfun-cap__card-actions" onClick={(e) => e.stopPropagation()}>
                      <Switch
                        checked={skill.enabled}
                        onChange={() => handleToggle(skill)}
                        size="small"
                      />
                    </div>
                  </div>
                  {isExpanded && (
                    <CardBody className="bitfun-cap__card-details">
                      {skill.description && (
                        <div className="bitfun-cap__card-desc">{skill.description}</div>
                      )}
                      <button
                        type="button"
                        className="bitfun-cap__path"
                        onClick={() => handleCopyPath(skill.path)}
                        title={t('clickToCopy')}
                      >
                        <span className="bitfun-cap__path-label">{t('path')}</span>
                        <span className="bitfun-cap__path-value">{skill.path}</span>
                        <span className="bitfun-cap__path-copy">
                          {copiedPath === skill.path ? <Check size={11} /> : <Copy size={11} />}
                        </span>
                      </button>
                    </CardBody>
                  )}
                </Card>
              );
            })}
          </div>
        )}
      </div>
      <div className="bitfun-cap__footer">
        <button
          type="button"
          className={`bitfun-cap__refresh ${isRefreshing ? 'is-spinning' : ''}`}
          onClick={handleRefresh}
          title={t('refresh')}
        >
          <RefreshCw size={11} />
          <span>{t('refresh')}</span>
        </button>
      </div>
    </div>
  );
};

export default SkillsView;
