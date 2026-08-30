/** Push button with optional force-push dropdown. */

import { Button, Icon, IconButton, Menu, MenuItem, MenuSeparator, Tooltip } from '@bitfun/ui';
import React, { useState, useRef, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { AlertTriangle } from 'lucide-react';

import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useI18n } from '@/infrastructure/i18n';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
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

function iconButtonVariant(variant: PushButtonProps['variant']): React.ComponentProps<typeof IconButton>['variant'] {
  if (variant === 'primary' || variant === 'accent') return 'primary';
  return 'quiet';
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
  const menuRef = useRef<HTMLDivElement>(null);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const { t } = useI18n('panels/git');
  const menuLayout = useAnchoredPopoverPosition({
    open: showDropdown,
    anchorRef: wrapperRef,
    popoverRef: menuRef,
    preferredPlacement: 'bottom',
    gap: 6,
  });


  useEffect(() => {
    if (!showDropdown) return;

    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (!wrapperRef.current?.contains(target) && !menuRef.current?.contains(target)) {
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
    <div className={`bitfun-push-button ${className}`} data-bf-component="git-tool" data-bf-part="pushButton">
      <div className="bitfun-push-button__wrapper" ref={wrapperRef}>
        {iconOnly ? (
          <Tooltip content={t('actions.push')}>
            <IconButton
              size={buttonSize(size)}
              variant={iconButtonVariant(variant)}
              onClick={() => handlePush(false)}
              disabled={disabled || loading}
              loading={loading}
              aria-label={t('actions.push')}
              icon={<Icon name="arrow-up" size="sm" />}
            />
          </Tooltip>
        ) : (
          <Button
            variant={buttonVariant(variant)}
            size={buttonSize(size)}
            onClick={() => handlePush(false)}
            disabled={disabled || loading}
            loading={loading}
            leadingIcon={<Icon name="arrow-up" size="lg" />}
          >
            {t('actions.push')}
          </Button>
        )}

        <Tooltip content={`${t('actions.push')} / ${t('actions.forcePush')}`}>
          <IconButton
            size={buttonSize(size)}
            variant={iconButtonVariant(variant)}
            onClick={handleToggleDropdown}
            disabled={disabled || loading}
            aria-label={`${t('actions.push')} / ${t('actions.forcePush')}`}
            icon={<Icon name="chevron-down" size="sm" className={`bitfun-push-button__arrow ${showDropdown ? 'bitfun-push-button__arrow--open' : ''}`} />}
          />
        </Tooltip>
      </div>

      {showDropdown && createPortal(
        <Menu
          ref={menuRef}
          className="bitfun-push-button__menu"
          style={{
            top: `${menuLayout?.top ?? 0}px`,
            left: `${menuLayout?.left ?? 0}px`,
            visibility: menuLayout ? 'visible' : 'hidden',
          }}
          aria-label={`${t('actions.push')} / ${t('actions.forcePush')}`}
          autoFocusFirstItem
        >
          <MenuItem
            leading={<Icon name="arrow-up" size="sm" />}
            onClick={() => handlePush(false)}
          >
            {t('actions.push')}
          </MenuItem>

          <MenuSeparator />

          <MenuItem
            leading={<AlertTriangle size={14} aria-hidden />}
            tone="danger"
            onClick={() => handlePush(true)}
          >
            {t('actions.forcePush')}
          </MenuItem>
        </Menu>,
        getAppearanceOverlayHost(),
      )}
    </div>
  );
};

