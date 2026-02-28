/**
 * SceneTab — a single tab in the SceneBar.
 *
 * Pinned tabs show no close button.
 * Dynamic tabs show a close button on hover/active.
 * When `subtitle` is provided it renders as "label / subtitle" (e.g. "AI Agent / session name").
 * Optional action (e.g. new session) shown inside __content when onActionClick is provided.
 */

import React, { useCallback } from 'react';
import { Plus, X } from 'lucide-react';
import type { SceneTab as SceneTabType, SceneTabDef } from './types';

interface SceneTabProps {
  tab: SceneTabType;
  def: SceneTabDef;
  isActive: boolean;
  subtitle?: string;
  /** When set, shows a Plus icon in tab content for quick action (e.g. new session) */
  onActionClick?: () => void;
  /** Accessible label for the action icon */
  actionTitle?: string;
  onActivate: (id: SceneTabType['id']) => void;
  onClose: (id: SceneTabType['id']) => void;
}

const SceneTab: React.FC<SceneTabProps> = ({
  tab,
  def,
  isActive,
  subtitle,
  onActionClick,
  actionTitle,
  onActivate,
  onClose,
}) => {
  const { Icon, label, pinned } = def;

  const handleClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onActivate(tab.id);
  }, [onActivate, tab.id]);

  const handleClose = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onClose(tab.id);
  }, [onClose, tab.id]);

  const handleActionClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onActionClick?.();
  }, [onActionClick]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onActivate(tab.id);
    }
  }, [onActivate, tab.id]);

  return (
    <div
      role="tab"
      aria-selected={isActive}
      tabIndex={isActive ? 0 : -1}
      className={[
        'bitfun-scene-tab',
        isActive && 'bitfun-scene-tab--active',
        pinned && 'bitfun-scene-tab--pinned',
      ].filter(Boolean).join(' ')}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
    >
      {/* Centered content group */}
      <div className="bitfun-scene-tab__content">
        {Icon && <Icon size={13} className="bitfun-scene-tab__icon" aria-hidden="true" />}
        <span className="bitfun-scene-tab__label">{label}</span>
        {subtitle && (
          <>
            <span className="bitfun-scene-tab__sep" aria-hidden="true">/</span>
            <span className="bitfun-scene-tab__subtitle">{subtitle}</span>
          </>
        )}
        {onActionClick && (
          <span
            className="bitfun-scene-tab__action"
            onClick={handleActionClick}
            onMouseDown={e => e.stopPropagation()}
            role="button"
            tabIndex={-1}
            aria-label={actionTitle ?? ''}
            title={actionTitle ?? ''}
          >
            <Plus size={12} aria-hidden="true" />
          </span>
        )}
      </div>

      {!pinned && (
        <button
          type="button"
          className="bitfun-scene-tab__close"
          aria-label={`Close ${label}`}
          onClick={handleClose}
          tabIndex={-1}
        >
          <X size={11} aria-hidden="true" />
        </button>
      )}
    </div>
  );
};

export default SceneTab;
