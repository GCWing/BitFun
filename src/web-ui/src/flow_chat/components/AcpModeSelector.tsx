/**
 * The ACP session mode picker.
 *
 * ACP's `mode` category is "the picker that is not the model picker": dsh-acp
 * publishes its agent presets there, claude-code and codex their permission
 * modes. It used to live as a second section inside the model dropdown, which
 * made one button stand for two unrelated choices. It is its own trigger now,
 * sitting beside the model picker and showing the mode in force.
 */

import React, { useCallback, useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Check, ChevronDown } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import { PresenceBoundary } from '@/component-library/components/PresenceBoundary';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import type { AcpModeState } from '../utils/acpSessionConfig';
import { getModelSelectorDropdownLayout } from './modelSelectorDropdownPosition';
import './AcpModeSelector.scss';

interface AcpModeSelectorProps {
  mode: AcpModeState;
  /** Which agent published the mode; shown as the dropdown's right-hand hint. */
  clientId?: string;
  disabled?: boolean;
  loading?: boolean;
  dropdownPlacement?: 'top' | 'bottom';
  /**
   * Replaces the trigger tooltip. The caller uses this when this picker is the
   * only one on screen and therefore also carries the context-usage readout.
   */
  tooltip?: React.ReactNode;
  /** Extra trigger content before the chevron — the context-usage badge. */
  trailing?: React.ReactNode;
  onSelect: (value: string) => void | Promise<void>;
}

export const AcpModeSelector: React.FC<AcpModeSelectorProps> = ({
  mode,
  clientId,
  disabled = false,
  loading = false,
  dropdownPlacement = 'top',
  tooltip,
  trailing,
  onSelect,
}) => {
  const { t } = useTranslation('flow-chat');
  const [open, setOpen] = useState(false);
  const [keyboardOpen, setKeyboardOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuId = useId();
  const [menuStyle, setMenuStyle] = useState<React.CSSProperties>({
    position: 'fixed',
    visibility: 'hidden',
  });
  const [resolvedPlacement, setResolvedPlacement] = useState(dropdownPlacement);

  const candidates = mode.option.options;

  useEffect(() => {
    if (candidates.length === 0) setOpen(false);
  }, [candidates.length]);

  useEffect(() => {
    if (!open) return;
    const handlePointerDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (!rootRef.current?.contains(target) && !menuRef.current?.contains(target)) {
        setOpen(false);
        setKeyboardOpen(false);
      }
    };
    document.addEventListener('mousedown', handlePointerDown);
    return () => document.removeEventListener('mousedown', handlePointerDown);
  }, [open]);

  useEffect(() => {
    if (!open || !rootRef.current) return;
    const updatePosition = () => {
      if (!rootRef.current || !menuRef.current) return;
      const layout = getModelSelectorDropdownLayout(
        rootRef.current.getBoundingClientRect(),
        menuRef.current.getBoundingClientRect(),
        dropdownPlacement,
        { width: window.innerWidth, height: window.innerHeight },
      );
      setMenuStyle(layout.style);
      setResolvedPlacement(layout.placement);
    };
    updatePosition();
    const observer = new ResizeObserver(updatePosition);
    if (menuRef.current) observer.observe(menuRef.current);
    window.addEventListener('scroll', updatePosition, true);
    window.addEventListener('resize', updatePosition);
    return () => {
      observer.disconnect();
      window.removeEventListener('scroll', updatePosition, true);
      window.removeEventListener('resize', updatePosition);
    };
  }, [dropdownPlacement, open]);

  useEffect(() => {
    if (!open || !keyboardOpen) return;
    const frame = window.requestAnimationFrame(() => {
      const checked = menuRef.current?.querySelector<HTMLButtonElement>(
        'button[role="menuitemradio"][aria-checked="true"]:not(:disabled)',
      );
      const first = menuRef.current?.querySelector<HTMLButtonElement>(
        'button[role="menuitemradio"]:not(:disabled)',
      );
      (checked ?? first)?.focus();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [keyboardOpen, open]);

  const select = useCallback((value: string) => {
    if (menuRef.current?.contains(document.activeElement)) {
      triggerRef.current?.focus();
    }
    setOpen(false);
    void onSelect(value);
  }, [onSelect]);

  const handleMenuKeyDown = useCallback((event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      triggerRef.current?.focus();
      setOpen(false);
      return;
    }
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
    const items = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>(
      'button[role="menuitemradio"]:not(:disabled)',
    ));
    if (items.length === 0) return;
    event.preventDefault();
    const activeIndex = items.indexOf(document.activeElement as HTMLButtonElement);
    let nextIndex = activeIndex;
    if (event.key === 'Home') nextIndex = 0;
    if (event.key === 'End') nextIndex = items.length - 1;
    if (event.key === 'ArrowDown') nextIndex = activeIndex < 0 ? 0 : (activeIndex + 1) % items.length;
    if (event.key === 'ArrowUp') nextIndex = activeIndex < 0 ? items.length - 1 : (activeIndex - 1 + items.length) % items.length;
    items[nextIndex]?.focus();
  }, []);

  if (candidates.length === 0) return null;

  const currentLabel = candidates.find(candidate => candidate.value === mode.currentValue)?.name
    ?? mode.currentValue;
  const triggerTooltip = tooltip
    ?? mode.option.description
    ?? `${mode.option.name}: ${currentLabel}`;

  return (
    <div
      ref={rootRef}
      className="bitfun-acp-mode-selector"
      data-bf-component="acp-mode-selector"
      data-bf-part="root"
      data-bf-state={open ? 'open' : undefined}
    >
      <Tooltip content={triggerTooltip} disabled={open}>
        <button
          ref={triggerRef}
          type="button"
          className={`bitfun-acp-mode-selector__trigger${open ? ' bitfun-acp-mode-selector__trigger--open' : ''}`}
          data-bf-component="acp-mode-selector"
          data-bf-part="trigger"
          data-bf-state={open ? 'open' : undefined}
          data-testid="chat-acp-mode-selector-btn"
          aria-haspopup="menu"
          aria-expanded={open}
          aria-controls={open ? menuId : undefined}
          disabled={disabled || loading}
          onClick={(event) => {
            const nextOpen = !open;
            if (nextOpen) {
              setKeyboardOpen(event.detail === 0);
            } else if (event.detail !== 0) {
              setKeyboardOpen(false);
            }
            setOpen(nextOpen);
          }}
          onKeyDown={(event) => {
            if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
              event.preventDefault();
              setKeyboardOpen(true);
              setOpen(true);
            } else if (event.key === 'Escape' && open) {
              event.preventDefault();
              setOpen(false);
            }
          }}
        >
          <span
            className="bitfun-acp-mode-selector__label"
            data-bf-component="acp-mode-selector"
            data-bf-part="label"
          >
            {currentLabel}
          </span>
          {trailing}
          <ChevronDown size={10} aria-hidden="true" />
        </button>
      </Tooltip>

      <PresenceBoundary active={open}>
        {createPortal(
          <div
            id={menuId}
            ref={menuRef}
            className="bitfun-acp-mode-selector__menu"
            data-bf-component="acp-mode-selector"
            data-bf-part="menu"
            data-placement={resolvedPlacement}
            data-open={open ? 'true' : 'false'}
            data-keyboard-open={keyboardOpen ? 'true' : 'false'}
            style={menuStyle}
            role="menu"
            aria-hidden={!open}
            {...(!open ? { inert: '' } : {})}
            aria-label={t('modelSelector.acpMode')}
            data-testid="chat-acp-mode-selector-menu"
            onKeyDown={handleMenuKeyDown}
          >
            <div
              className="bitfun-acp-mode-selector__header"
              data-bf-component="acp-mode-selector"
              data-bf-part="header"
            >
              <span>{t('modelSelector.acpMode')}</span>
              {clientId && (
                <span className="bitfun-acp-mode-selector__header-hint">{clientId}</span>
              )}
            </div>
            {candidates.map((candidate) => {
              const isSelected = mode.currentValue === candidate.value;
              // The row stays one line; what a mode does — or why it can no
              // longer change — is hover-only.
              const hint = mode.locked
                ? (mode.option.description ?? t('modelSelector.acpModeLocked'))
                : (candidate.description ?? candidate.name);

              return (
                <Tooltip key={candidate.value} content={hint} placement="right">
                  <button
                    type="button"
                    role="menuitemradio"
                    aria-checked={isSelected}
                    data-testid="chat-acp-mode-option"
                    data-mode-value={candidate.value}
                    data-selected={isSelected ? 'true' : 'false'}
                    disabled={mode.locked || loading}
                    className="bitfun-acp-mode-selector__option"
                    data-bf-component="acp-mode-selector"
                    data-bf-part="option"
                    data-bf-state={isSelected ? 'selected' : undefined}
                    onClick={() => select(candidate.value)}
                  >
                    <span>
                      <strong>{candidate.name}</strong>
                    </span>
                    {isSelected && <Check size={14} aria-hidden="true" />}
                  </button>
                </Tooltip>
              );
            })}
          </div>,
          getAppearanceOverlayHost(),
        )}
      </PresenceBoundary>
    </div>
  );
};

export default AcpModeSelector;
