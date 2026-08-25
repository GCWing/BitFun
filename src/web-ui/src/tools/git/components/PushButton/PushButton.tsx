/** Push button with optional force-push dropdown. */

import { Button, IconButton } from '@bitfun/ui';
import React, { useState, useRef, useEffect } from 'react';
import { ChevronDown, ArrowUp, AlertTriangle } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import './PushButton.scss';

export interface PushButtonProps {
  /** Push callback (force = true for force-push) */
  onPush: (force: boolean) => void | Promise<void>;
  /** Disabled state */
  disabled?: boolean;
  /** Loading state */
  loading?: boolean;
  /** Button size */
  size?: 'small' | 'medium' | 'large';
  /** Button variant */
  variant?: 'primary' | 'accent' | 'secondary' | 'ghost';
  /** Custom class name */
  className?: string;
  /** Render as icon-only buttons */
  iconOnly?: boolean;
}

function buttonSize(size: PushButtonProps['size']): React.ComponentProps<typeof Button>['size'] {
  if (size === 'small') return 'sm';
  if (size === 'large') return 'lg';
  return 'md';
}

function buttonVariant(variant: PushButtonProps['variant']): React.ComponentProps<typeof Button>['variant'] {
  if (variant === 'secondary' || variant === 'ghost') return 'outline';
  return 'fill';
}

function iconButtonSize(size: PushButtonProps['size']): React.ComponentProps<typeof IconButton>['size'] {
  return size === 'small' ? 'md' : 'lg';
}

function iconButtonTone(variant: PushButtonProps['variant']): React.ComponentProps<typeof IconButton>['tone'] {
  return variant === 'primary' || variant === 'accent' ? 'primary' : 'neutral';
}

export const PushButton: React.FC<PushButtonProps> = ({
  onPush,
  disabled = false,
  loading = false,
  size = 'small',
  variant = 'accent',
  className = '',
  iconOnly = false
}) => {
  const [showDropdown, setShowDropdown] = useState(false);
  const [menuPosition, setMenuPosition] = useState<{ top: number; left: number; width: number }>({ top: 0, left: 0, width: 0 });
  const dropdownRef = useRef<HTMLDivElement>(null);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const { t } = useI18n('panels/git');


  useEffect(() => {
    if (showDropdown && wrapperRef.current) {
      const rect = wrapperRef.current.getBoundingClientRect();
      setMenuPosition({
        top: rect.bottom + 6,
        left: rect.left,
        width: 0
      });
    }
  }, [showDropdown]);


  useEffect(() => {
    if (!showDropdown) return;

    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setShowDropdown(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [showDropdown]);


  const handlePush = async (force: boolean = false) => {
    setShowDropdown(false);
    await onPush(force);
  };


  const handleToggleDropdown = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!disabled && !loading) {
      setShowDropdown(!showDropdown);
    }
  };

  return (
    <div className={`bitfun-push-button ${className}`} ref={dropdownRef} data-bf-component="git-tool" data-bf-part="pushButton">
      <div className="bitfun-push-button__wrapper" ref={wrapperRef}>
        {iconOnly ? (
          <IconButton
            size={iconButtonSize(size)}
            tone={iconButtonTone(variant)}
            onClick={() => handlePush(false)}
            disabled={disabled || loading}
            loading={loading}
            aria-label={t('actions.push')}
            title={t('actions.push')}
          >
            <ArrowUp size={14} />
          </IconButton>
        ) : (
          <Button
            variant={buttonVariant(variant)}
            size={buttonSize(size)}
            onClick={() => handlePush(false)}
            disabled={disabled || loading}
            loading={loading}
            leadingIcon={<ArrowUp />}
          >
            {t('actions.push')}
          </Button>
        )}

        <IconButton
          size={iconButtonSize(size)}
          tone={iconButtonTone(variant)}
          onClick={handleToggleDropdown}
          disabled={disabled || loading}
          aria-label={`${t('actions.push')} / ${t('actions.forcePush')}`}
          title={`${t('actions.push')} / ${t('actions.forcePush')}`}
        >
          <ChevronDown
            size={14}
            className={`bitfun-push-button__arrow ${showDropdown ? 'bitfun-push-button__arrow--open' : ''}`}
          />
        </IconButton>
      </div>

      {showDropdown && (
        <div 
          className="bitfun-push-button__menu"
          style={{
            top: `${menuPosition.top}px`,
            left: `${menuPosition.left}px`
          }}
        >
          <button
            className="bitfun-push-button__menu-item"
            onClick={() => handlePush(false)}
          >
            <ArrowUp size={14} />
            <span className="bitfun-push-button__menu-item-title">{t('actions.push')}</span>
          </button>

          <div className="bitfun-push-button__menu-divider" />

          <button
            className="bitfun-push-button__menu-item bitfun-push-button__menu-item--danger"
            onClick={() => handlePush(true)}
          >
            <AlertTriangle size={14} />
            <span className="bitfun-push-button__menu-item-title">{t('actions.forcePush')}</span>
          </button>
        </div>
      )}
    </div>
  );
};

