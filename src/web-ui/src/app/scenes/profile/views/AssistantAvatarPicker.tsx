import React, { useEffect, useId, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Bot, Check, Pencil, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, IconButton, Input } from '@/component-library';
import type { IdentitySaveStatus } from '@/app/scenes/my-agent/useAgentIdentityDocument';
import { ASSISTANT_AVATAR_PRESETS, firstAvatarGrapheme } from './assistantAvatar';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';

interface AssistantAvatarPickerProps {
  value: string;
  saveStatus: IdentitySaveStatus;
  saveError?: string | null;
  onChange: (value: string) => void;
}

const AssistantAvatarPicker: React.FC<AssistantAvatarPickerProps> = ({
  value,
  saveStatus,
  saveError,
  onChange,
}) => {
  const { t } = useTranslation('scenes/profile');
  const pickerId = useId();
  const titleId = `${pickerId}-title`;
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const [isOpen, setIsOpen] = useState(false);
  const displayedValue = useMemo(() => firstAvatarGrapheme(value), [value]);
  const [customValue, setCustomValue] = useState(displayedValue);
  const popoverLayout = useAnchoredPopoverPosition({
    open: isOpen,
    anchorRef: triggerRef,
    popoverRef,
    preferredPlacement: 'bottom',
    alignment: 'start',
    gap: 8,
  });

  useEffect(() => {
    setCustomValue(displayedValue);
  }, [displayedValue]);

  useEffect(() => {
    if (!isOpen) return;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!rootRef.current?.contains(target) && !popoverRef.current?.contains(target)) {
        setIsOpen(false);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      setIsOpen(false);
      triggerRef.current?.focus();
    };

    document.addEventListener('pointerdown', handlePointerDown);
    window.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [isOpen]);

  const chooseAvatar = (nextValue: string) => {
    setCustomValue(nextValue);
    onChange(nextValue);
  };

  const handleCustomSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const nextValue = firstAvatarGrapheme(customValue);
    if (!nextValue) return;
    chooseAvatar(nextValue);
  };

  const normalizedCustomValue = firstAvatarGrapheme(customValue);
  const statusContent = saveStatus === 'saving'
    ? t('identity.avatarSaving')
    : saveStatus === 'saved'
      ? t('identity.avatarSaved')
      : saveStatus === 'error'
        ? t('identity.avatarSaveFailed')
        : t('identity.avatarAutosave');

  return (
    <div ref={rootRef} className="acp-avatar-picker">
      <button
        ref={triggerRef}
        type="button"
        className={`acp-avatar-picker__trigger${isOpen ? ' is-open' : ''}`}
        aria-label={t('identity.avatarChange')}
        aria-expanded={isOpen}
        aria-controls={pickerId}
        onClick={() => setIsOpen((open) => !open)}
      >
        <span className="acp-avatar-picker__value" aria-hidden="true">
          {displayedValue || <Bot size={26} strokeWidth={1.6} />}
        </span>
        <span className="acp-avatar-picker__edit-cue" aria-hidden="true">
          <Pencil size={11} strokeWidth={1.9} />
        </span>
      </button>

      {isOpen ? createPortal(
        <div
          ref={popoverRef}
          id={pickerId}
          className="acp-avatar-picker__popover"
          role="region"
          aria-labelledby={titleId}
          data-bf-placement={popoverLayout?.placement ?? 'bottom'}
          style={{
            top: `${popoverLayout?.top ?? 0}px`,
            left: `${popoverLayout?.left ?? 0}px`,
            visibility: popoverLayout ? 'visible' : 'hidden',
          }}
        >
          <div className="acp-avatar-picker__header">
            <div className="acp-avatar-picker__heading">
              <strong id={titleId}>{t('identity.avatarPickerTitle')}</strong>
              <span>{t('identity.avatarPickerHint')}</span>
            </div>
            <IconButton
              type="button"
              size="xs"
              variant="ghost"
              aria-label={t('identity.avatarPickerClose')}
              onClick={() => {
                setIsOpen(false);
                triggerRef.current?.focus();
              }}
            >
              <X size={14} />
            </IconButton>
          </div>

          <div className="acp-avatar-picker__grid" role="group" aria-label={t('identity.avatarPresets')}>
            {ASSISTANT_AVATAR_PRESETS.map((preset) => (
              <button
                key={preset}
                type="button"
                className={`acp-avatar-picker__option${displayedValue === preset ? ' is-selected' : ''}`}
                aria-label={t('identity.avatarUsePreset', { emoji: preset })}
                aria-pressed={displayedValue === preset}
                onClick={() => chooseAvatar(preset)}
              >
                <span aria-hidden="true">{preset}</span>
              </button>
            ))}
          </div>

          <form className="acp-avatar-picker__custom" onSubmit={handleCustomSubmit}>
            <Input
              value={customValue}
              size="small"
              maxLength={16}
              autoComplete="off"
              aria-label={t('identity.avatarCustom')}
              placeholder={t('identity.avatarCustomPlaceholder')}
              className="acp-avatar-picker__custom-input"
              onChange={(event) => setCustomValue(event.target.value)}
            />
            <Button
              type="submit"
              variant="secondary"
              size="small"
              disabled={!normalizedCustomValue || normalizedCustomValue === displayedValue}
            >
              {t('identity.avatarApply')}
            </Button>
          </form>

          <div className="acp-avatar-picker__footer">
            <span
              className={[
                'acp-avatar-picker__status',
                saveStatus === 'error' && 'is-error',
                saveStatus === 'saved' && 'is-saved',
              ].filter(Boolean).join(' ')}
              title={saveStatus === 'error' && saveError ? saveError : undefined}
              aria-live="polite"
            >
              {saveStatus === 'saved' ? <Check size={12} strokeWidth={2} aria-hidden="true" /> : null}
              {statusContent}
            </span>
          </div>
        </div>,
        getAppearanceOverlayHost(),
      ) : null}
    </div>
  );
};

export default AssistantAvatarPicker;
