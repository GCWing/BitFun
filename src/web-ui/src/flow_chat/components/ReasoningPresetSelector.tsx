import React, { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import {
  Check,
  Circle,
  CircleOff,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import { PresenceBoundary } from '@/component-library/components/PresenceBoundary';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import type {
  ReasoningCatalogProjection,
  ReasoningPresetDescriptor,
} from '@/infrastructure/config/types';
import { getModelSelectorDropdownLayout } from './modelSelectorDropdownPosition';
import './ReasoningPresetSelector.scss';

interface ReasoningPresetSelectorProps {
  projection?: ReasoningCatalogProjection | null;
  selectedPreset?: string | null;
  disabled?: boolean;
  loading?: boolean;
  dropdownPlacement?: 'top' | 'bottom';
  onSelect: (presetId: string | null) => void | Promise<void>;
}

function presetLabel(
  preset: ReasoningPresetDescriptor,
  t: ReturnType<typeof useTranslation>['t'],
): string {
  return t(`reasoningEffort.${preset.id}`, { defaultValue: preset.label || preset.id });
}

function presetSourceLabel(
  source: ReasoningPresetDescriptor['source'],
  t: ReturnType<typeof useTranslation>['t'],
): string {
  switch (source) {
    case 'models_dev':
      return t('reasoningSelector.source.models_dev');
    case 'adapter_fallback':
      return t('reasoningSelector.source.adapter_fallback');
    case 'model_config':
      return t('reasoningSelector.source.model_config');
  }
}

function presetSourceTooltip(
  source: ReasoningPresetDescriptor['source'],
  t: ReturnType<typeof useTranslation>['t'],
): string {
  switch (source) {
    case 'models_dev':
      return t('reasoningSelector.sourceTooltip.models_dev');
    case 'adapter_fallback':
      return t('reasoningSelector.sourceTooltip.adapter_fallback');
    case 'model_config':
      return t('reasoningSelector.sourceTooltip.model_config');
  }
}

function presetDisplayLabel(
  preset: ReasoningPresetDescriptor,
  orderedPresets: ReasoningPresetDescriptor[],
  t: ReturnType<typeof useTranslation>['t'],
): string {
  const fallback = presetLabel(preset, t);
  const semanticKey = presetVisualSemanticKey(preset, orderedPresets);
  return semanticKey
    ? t(`reasoningSelector.levels.${semanticKey}`, { defaultValue: fallback })
    : fallback;
}

function presetModeLabel(
  preset: ReasoningPresetDescriptor,
  orderedPresets: ReasoningPresetDescriptor[],
  t: ReturnType<typeof useTranslation>['t'],
): string {
  const semanticKey = presetVisualSemanticKey(preset, orderedPresets);
  return semanticKey
    ? t(`reasoningSelector.modes.${semanticKey}`, {
        defaultValue: t('reasoningSelector.modes.custom'),
      })
    : t('reasoningSelector.modes.custom');
}

type ReasoningIntensityLevel = 0 | 1 | 2 | 3 | 4;

type ReasoningPresetSemanticKey =
  | 'off'
  | 'on'
  | 'minimal'
  | 'low'
  | 'medium'
  | 'high'
  | 'xhigh'
  | 'max';

const REASONING_PRESET_SEMANTIC_KEYS = new Set<ReasoningPresetSemanticKey>([
  'off',
  'on',
  'minimal',
  'low',
  'medium',
  'high',
  'xhigh',
  'max',
]);

function asReasoningPresetSemanticKey(value: string): ReasoningPresetSemanticKey | undefined {
  const normalized = value.trim().toLowerCase();
  if (normalized === 'none') return 'off';
  return REASONING_PRESET_SEMANTIC_KEYS.has(normalized as ReasoningPresetSemanticKey)
    ? normalized as ReasoningPresetSemanticKey
    : undefined;
}

function presetSemanticKey(
  preset: ReasoningPresetDescriptor,
): ReasoningPresetSemanticKey | undefined {
  const idKey = asReasoningPresetSemanticKey(preset.id);
  if (idKey) return idKey;

  // Generated presets can prefix or replace their semantic id (for example
  // effort-off and budget-max). Their action is the stable meaning to localize.
  // Custom presets keep their authored label instead of being renamed by an
  // implementation detail in their request actions.
  if (preset.source === 'model_config') return undefined;

  for (const action of preset.actions) {
    if (action.type === 'toggle') return action.enabled ? 'on' : 'off';
    if (action.type === 'effort') {
      const effortKey = asReasoningPresetSemanticKey(action.value);
      if (effortKey) return effortKey;
    }
    if (action.type === 'budget_tokens') {
      if (preset.id.toLowerCase().includes('max')) return 'max';
      if (preset.id.toLowerCase().includes('high')) return 'high';
    }
  }

  return undefined;
}

function presetDisablesReasoning(preset: ReasoningPresetDescriptor): boolean {
  if (presetSemanticKey(preset) === 'off') return true;
  return preset.actions.some(action => (
    action.type === 'toggle' && !action.enabled
  ));
}

function reasoningIntensityLevel(
  preset: ReasoningPresetDescriptor | undefined,
  orderedPresets: ReasoningPresetDescriptor[],
): ReasoningIntensityLevel {
  if (!preset) return 0;
  if (presetDisablesReasoning(preset)) return 0;

  const activePresets = orderedPresets.filter(item => !presetDisablesReasoning(item));
  const activeIndex = activePresets.findIndex(item => item.id === preset.id);
  if (activeIndex < 0) return 0;
  if (activePresets.length === 1) return 1;

  // The catalog may merge toggle, effort and token-budget presets. Ranking by
  // the catalog order keeps the visual series monotonic even when ids come from
  // different action families (off, on, low, high, max).
  return Math.min(
    4,
    Math.max(1, Math.round((activeIndex / (activePresets.length - 1)) * 3) + 1),
  ) as 1 | 2 | 3 | 4;
}

function presetVisualSemanticKey(
  preset: ReasoningPresetDescriptor,
  orderedPresets: ReasoningPresetDescriptor[],
): ReasoningPresetSemanticKey | undefined {
  if (presetDisablesReasoning(preset)) return 'off';

  const includesUnscopedOnPreset = orderedPresets.some(item => (
    !presetDisablesReasoning(item) && presetSemanticKey(item) === 'on'
  ));
  if (!includesUnscopedOnPreset) return presetSemanticKey(preset);

  switch (reasoningIntensityLevel(preset, orderedPresets)) {
    case 1:
      return 'low';
    case 2:
      return 'medium';
    case 3:
      return 'high';
    case 4:
      return 'max';
    case 0:
      return 'off';
  }
}

interface ReasoningIntensityMarkProps {
  level: ReasoningIntensityLevel;
  compact?: boolean;
}

const ReasoningIntensityMark: React.FC<ReasoningIntensityMarkProps> = ({
  level,
  compact = false,
}) => {
  const ringSizes = compact ? [14, 9, 4.5] : [24, 14, 6.5];
  const ringCount = level === 0 ? 0 : Math.min(level, 3);

  return (
    <span
      className="bitfun-reasoning-preset-selector__status-meter"
      data-intensity={level}
      data-size={compact ? 'compact' : 'option'}
      aria-hidden="true"
    >
      {level === 0 ? (
        <CircleOff
          className="bitfun-reasoning-preset-selector__status-off"
          size={compact ? 14 : 22}
          strokeWidth={compact ? 1.5 : 1.2}
        />
      ) : (
        <>
          {ringSizes.slice(0, ringCount).map((size, index) => (
            <Circle
              key={size}
              className="bitfun-reasoning-preset-selector__status-ring"
              data-ring={index + 1}
              size={size}
              strokeWidth={index === 0
                ? (compact ? 1.45 : 1.1)
                : (compact ? 1.7 : 1.35)}
            />
          ))}
          {level === 4 && (
            <Circle
              className="bitfun-reasoning-preset-selector__status-peak"
              size={compact ? 3.5 : 5.5}
              strokeWidth={0}
              fill="currentColor"
            />
          )}
        </>
      )}
    </span>
  );
};

export const ReasoningPresetSelector: React.FC<ReasoningPresetSelectorProps> = ({
  projection,
  selectedPreset,
  disabled = false,
  loading = false,
  dropdownPlacement = 'top',
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

  const presets = useMemo(
    () => (projection?.status === 'known' ? projection.presets ?? [] : []),
    [projection],
  );
  const selected = presets.find(preset => preset.id === selectedPreset);
  const defaultPreset = presets.find(preset => preset.id === projection?.default_preset);

  useEffect(() => {
    if (presets.length === 0) setOpen(false);
  }, [presets.length]);

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
        'button[role="menuitemradio"][aria-checked="true"]',
      );
      const first = menuRef.current?.querySelector<HTMLButtonElement>(
        'button[role="menuitemradio"]',
      );
      (checked ?? first)?.focus();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [keyboardOpen, open]);

  const select = useCallback((presetId: string | null) => {
    if (menuRef.current?.contains(document.activeElement)) {
      triggerRef.current?.focus();
    }
    setOpen(false);
    void onSelect(presetId);
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
    if (event.key === 'ArrowDown') {
      nextIndex = activeIndex < 0 ? 0 : (activeIndex + 1) % items.length;
    }
    if (event.key === 'ArrowUp') {
      nextIndex = activeIndex < 0 ? items.length - 1 : (activeIndex - 1 + items.length) % items.length;
    }
    items[nextIndex]?.focus();
  }, []);

  if (presets.length === 0) return null;

  const orderedPresets = [...presets].sort((left, right) => left.order - right.order);
  const presetLabels = orderedPresets.map(preset => (
    presetDisplayLabel(preset, orderedPresets, t)
  ));
  const labelCounts = new Map<string, number>();
  presetLabels.forEach((label) => {
    const normalizedLabel = label.trim().toLowerCase();
    labelCounts.set(normalizedLabel, (labelCounts.get(normalizedLabel) ?? 0) + 1);
  });

  const currentLabel = selected
    ? presetLabel(selected, t)
    : t('reasoningSelector.auto');
  const effectivePreset = selected ?? defaultPreset;
  const intensityLevel = reasoningIntensityLevel(effectivePreset, orderedPresets);
  const statusLabel = effectivePreset
    ? presetDisplayLabel(effectivePreset, orderedPresets, t)
    : currentLabel;
  // The trigger is the meter and nothing else, so this string is both the hover
  // text and the control's accessible name. It names the level the meter draws
  // rather than the preset's raw id, because that is what the shape shows.
  const tooltip = selected
    ? t('reasoningSelector.current', { preset: statusLabel })
    : t('reasoningSelector.currentAuto', {
        preset: effectivePreset ? statusLabel : t('reasoningSelector.modelDefault'),
      });

  return (
    <div
      ref={rootRef}
      className="bitfun-reasoning-preset-selector"
      data-bf-component="reasoning-preset-selector"
      data-bf-part="root"
      data-bf-state={open ? 'open' : undefined}
    >
      <Tooltip content={tooltip} disabled={open}>
        <button
          ref={triggerRef}
          type="button"
          className={[
            'bitfun-reasoning-preset-selector__trigger',
            open && 'bitfun-reasoning-preset-selector__trigger--open',
          ].filter(Boolean).join(' ')}
          data-bf-component="reasoning-preset-selector"
          data-bf-part="trigger"
          data-bf-state={open ? 'open' : undefined}
          data-testid="chat-reasoning-preset-selector-btn"
          aria-label={tooltip}
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
          <ReasoningIntensityMark level={intensityLevel} compact />
        </button>
      </Tooltip>

      <PresenceBoundary active={open}>
        {createPortal(
          <div
          id={menuId}
          ref={menuRef}
          className="bitfun-reasoning-preset-selector__menu"
          data-bf-component="reasoning-preset-selector"
          data-bf-part="menu"
          data-placement={resolvedPlacement}
          data-open={open ? 'true' : 'false'}
          data-keyboard-open={keyboardOpen ? 'true' : 'false'}
          style={menuStyle}
          role="menu"
          aria-hidden={!open}
          {...(!open ? { inert: '' } : {})}
          aria-label={t('reasoningSelector.title')}
          data-testid="chat-reasoning-preset-selector-menu"
          onKeyDown={handleMenuKeyDown}
        >
          <div
            className="bitfun-reasoning-preset-selector__header"
            data-bf-component="reasoning-preset-selector"
            data-bf-part="header"
          >
            <span className="bitfun-reasoning-preset-selector__title">
              {t('reasoningSelector.title')}
            </span>
            <button
              type="button"
              role="menuitemradio"
              aria-checked={!selected}
              className="bitfun-reasoning-preset-selector__auto"
              data-bf-component="reasoning-preset-selector"
              data-bf-part="auto"
              data-bf-state={!selected ? 'selected' : undefined}
              onClick={() => select(null)}
            >
              <span>{t('reasoningSelector.auto')}</span>
              {defaultPreset && (
                <small>{presetDisplayLabel(defaultPreset, orderedPresets, t)}</small>
              )}
              {!selected && (
                <Check
                  className="bitfun-reasoning-preset-selector__auto-check"
                  size={12}
                  aria-hidden="true"
                />
              )}
            </button>
          </div>
          <div
            className="bitfun-reasoning-preset-selector__options"
            data-bf-component="reasoning-preset-selector"
            data-bf-part="options"
          >
            {orderedPresets.map((preset, index) => {
              const isSelected = selected?.id === preset.id;
              const label = presetLabels[index] ?? presetLabel(preset, t);
              const hasDuplicateLabel = (labelCounts.get(label.trim().toLowerCase()) ?? 0) > 1;
              const optionIntensity = reasoningIntensityLevel(preset, orderedPresets);
              return (
                <Tooltip
                  key={preset.id}
                  content={presetSourceTooltip(preset.source, t)}
                  placement="right"
                >
                  <button
                    type="button"
                    role="menuitemradio"
                    aria-checked={isSelected}
                    data-preset-id={preset.id}
                    className="bitfun-reasoning-preset-selector__option"
                    data-bf-component="reasoning-preset-selector"
                    data-bf-part="option"
                    data-bf-state={isSelected ? 'selected' : undefined}
                    onClick={() => select(preset.id)}
                  >
                    <ReasoningIntensityMark level={optionIntensity} />
                    <span className="bitfun-reasoning-preset-selector__option-copy">
                      <strong>{label}</strong>
                      <small>
                        {hasDuplicateLabel
                          ? presetSourceLabel(preset.source, t)
                          : presetModeLabel(preset, orderedPresets, t)}
                      </small>
                    </span>
                    {isSelected && (
                      <Check
                        className="bitfun-reasoning-preset-selector__option-check"
                        size={12}
                        aria-hidden="true"
                      />
                    )}
                  </button>
                </Tooltip>
              );
            })}
          </div>
          </div>,
          getAppearanceOverlayHost(),
        )}
      </PresenceBoundary>
    </div>
  );
};

export default ReasoningPresetSelector;
