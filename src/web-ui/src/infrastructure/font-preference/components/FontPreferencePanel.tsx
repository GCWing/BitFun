import React, { useMemo, useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Select, type SelectOption, Switch } from '@/component-library';
import { ConfigPageRow, ConfigPageSection } from '@/infrastructure/config/components/common';
import { useFontPreference } from '../hooks/useFontPreference';
import { FontSizeLevel, PRESET_UI_BASE_PX, UI_FONT_SIZE_PRESETS } from '../types';
import './FontPreferencePanel.scss';

const UI_LEVELS: Array<Exclude<FontSizeLevel, 'custom'>> = ['compact', 'small', 'default', 'medium', 'large'];
const FLOW_CHAT_PX_OPTIONS = [12, 13, 14, 15, 16, 17, 18, 19, 20];

export function FontPreferencePanel() {
  const { t } = useTranslation('settings/appearance');
  const { preference, setUiSize, setFlowChatFont, reset } = useFontPreference();

  const { level, customPx } = preference.uiSize;
  const [customInput, setCustomInput] = useState<string>(String(customPx ?? 14));
  const [fcBaseInput, setFcBaseInput] = useState<string>(String(preference.flowChat.basePx ?? 14));
  const [customError, setCustomError] = useState<string | null>(null);

  useEffect(() => {
    if (preference.flowChat.mode === 'independent') {
      setFcBaseInput(String(preference.flowChat.basePx ?? 14));
    }
  }, [preference.flowChat.mode, preference.flowChat.basePx]);

  /** Legacy "sync" mode removed from UI: normalize to lift (UI +1). */
  useEffect(() => {
    if (preference.flowChat.mode === 'sync') {
      void setFlowChatFont('lift');
    }
  }, [preference.flowChat.mode, setFlowChatFont]);

  /** Baseline px currently applied in the UI (preset level or custom). */
  const getEffectiveUiBasePx = useCallback((): number => {
    if (level === 'custom') {
      const n = parseInt(customInput, 10);
      if (!isNaN(n) && n >= 12 && n <= 20) return n;
      return customPx ?? 14;
    }
    return PRESET_UI_BASE_PX[level];
  }, [level, customInput, customPx]);

  const handleLevelClick = useCallback(async (l: FontSizeLevel) => {
    if (l === 'custom') {
      const px = getEffectiveUiBasePx();
      setCustomInput(String(px));
      await setUiSize('custom', px);
    } else {
      await setUiSize(l);
    }
    setCustomError(null);
  }, [getEffectiveUiBasePx, setUiSize]);

  const handleCustomInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const raw = e.target.value;
    setCustomInput(raw);
    const px = parseInt(raw, 10);
    if (isNaN(px) || px < 12 || px > 20) {
      setCustomError(t('appearance.fontSize.customPxOutOfRange'));
    } else {
      setCustomError(null);
      void setUiSize('custom', px);
    }
  };

  const handleCustomStep = (delta: number) => {
    const current = parseInt(customInput, 10);
    const next = Math.max(12, Math.min(20, (isNaN(current) ? 14 : current) + delta));
    setCustomInput(String(next));
    setCustomError(null);
    void setUiSize('custom', next);
  };

  const handleReset = async () => {
    await reset();
    setCustomInput('14');
    setFcBaseInput('14');
    setCustomError(null);
  };

  const previewBasePx = level === 'custom'
    ? (parseInt(customInput, 10) || 14)
    : parseInt(UI_FONT_SIZE_PRESETS[level].base, 10);

  const customLevelLabelPx = (() => {
    if (level !== 'custom') return 14;
    const n = parseInt(customInput, 10);
    return !isNaN(n) && n >= 12 && n <= 20 ? n : 14;
  })();

  const fcIndependent = preference.flowChat.mode === 'independent';
  const flowChatPxValue = (() => {
    const n = parseInt(fcBaseInput, 10);
    return n >= 12 && n <= 20 ? n : 14;
  })();

  const flowChatPxOptions = useMemo<SelectOption[]>(
    () =>
      FLOW_CHAT_PX_OPTIONS.map((n) => ({
        value: n,
        label: t('appearance.fontSize.flowChatPxOption', { n }),
      })),
    [t]
  );

  const handleFlowChatCustomToggle = (enabled: boolean) => {
    if (enabled) {
      const px = parseInt(fcBaseInput, 10);
      const v = isNaN(px) || px < 12 || px > 20 ? 14 : px;
      setFcBaseInput(String(v));
      void setFlowChatFont('independent', v);
    } else {
      void setFlowChatFont('lift');
    }
  };

  const handleFlowChatPxChange = useCallback(
    (v: string | number | (string | number)[]) => {
      if (Array.isArray(v)) return;
      const n = typeof v === 'number' ? v : parseInt(String(v), 10);
      if (Number.isNaN(n)) return;
      setFcBaseInput(String(n));
      void setFlowChatFont('independent', n);
    },
    [setFlowChatFont]
  );

  return (
    <ConfigPageSection
      title={t('appearance.fontSize.title')}
      description={t('appearance.fontSize.hint')}
    >
      {/* UI Font Size */}
      <ConfigPageRow
        className="font-pref-panel__row--ui"
        label={t('appearance.fontSize.uiSizeLabel')}
        description={t('appearance.fontSize.uiSizeHint')}
        align="start"
        multiline
      >
        <div className="font-pref-panel__ui-size">
          <div className="font-pref-panel__ui-segment-block">
            <div className="font-pref-panel__level-buttons" role="group" aria-label={t('appearance.fontSize.uiSizeLabel')}>
              {UI_LEVELS.map((l) => (
                <button
                  key={l}
                  type="button"
                  className={[
                    'font-pref-panel__level-btn',
                    level === l ? 'font-pref-panel__level-btn--active' : '',
                  ].join(' ').trim()}
                  onClick={() => void handleLevelClick(l)}
                  aria-pressed={level === l}
                >
                  <span
                    className="font-pref-panel__level-label"
                    style={{ fontSize: UI_FONT_SIZE_PRESETS[l].base }}
                  >
                    {t(`appearance.fontSize.levels.${l}`)}
                  </span>
                </button>
              ))}
              <div className="font-pref-panel__custom-segment-inline">
                <button
                  type="button"
                  className={[
                    'font-pref-panel__level-btn',
                    level === 'custom' ? 'font-pref-panel__level-btn--active' : '',
                  ].join(' ').trim()}
                  onClick={() => void handleLevelClick('custom')}
                  aria-pressed={level === 'custom'}
                >
                  <span
                    className="font-pref-panel__level-label"
                    style={{ fontSize: `${customLevelLabelPx}px` }}
                  >
                    {t('appearance.fontSize.levels.custom')}
                  </span>
                </button>
                {level === 'custom' && (
                  <div
                    className="font-pref-panel__custom-controls"
                    role="group"
                    aria-label={t('appearance.fontSize.customPxLabel')}
                  >
                    <div className="font-pref-panel__stepper">
                      <button
                        type="button"
                        className="font-pref-panel__step-btn"
                        onClick={() => handleCustomStep(-1)}
                        aria-label="-1"
                      >−</button>
                      <input
                        type="number"
                        className={[
                          'font-pref-panel__number-input',
                          customError ? 'font-pref-panel__number-input--error' : '',
                        ].join(' ').trim()}
                        value={customInput}
                        min={12}
                        max={20}
                        step={1}
                        placeholder={t('appearance.fontSize.customPxPlaceholder')}
                        onChange={handleCustomInputChange}
                        onFocus={() => void handleLevelClick('custom')}
                        aria-invalid={!!customError}
                      />
                      <button
                        type="button"
                        className="font-pref-panel__step-btn"
                        onClick={() => handleCustomStep(1)}
                        aria-label="+1"
                      >+</button>
                    </div>
                    <span className="font-pref-panel__custom-unit">px</span>
                  </div>
                )}
              </div>
            </div>
          </div>
          {customError && (
            <span className="font-pref-panel__error">{customError}</span>
          )}

          {/* Live preview */}
          <div
            className="font-pref-panel__preview"
            style={{ fontSize: `${previewBasePx}px` }}
            aria-label="Font size preview"
          >
            {t('appearance.fontSize.previewText')}
          </div>
        </div>
      </ConfigPageRow>

      {/* Flow chat font scale */}
      <ConfigPageRow
        className="font-pref-panel__row--flow-chat"
        label={t('appearance.fontSize.flowChatLabel')}
        description={t('appearance.fontSize.flowChatHint')}
        align="start"
      >
        <div className="font-pref-panel__flow-chat">
          <div className="font-pref-panel__flow-chat-line">
            <Switch
              size="small"
              checked={fcIndependent}
              onChange={(e) => handleFlowChatCustomToggle(e.target.checked)}
              label={t('appearance.fontSize.flowChatCustomToggle')}
            />
          </div>
          {fcIndependent && (
            <div className="font-pref-panel__flow-chat-controls">
              <Select
                size="small"
                value={flowChatPxValue}
                options={flowChatPxOptions}
                onChange={handleFlowChatPxChange}
                placement="bottom"
              />
            </div>
          )}
        </div>
      </ConfigPageRow>

      {/* Reset */}
      <ConfigPageRow label="" align="center">
        <button
          type="button"
          className="font-pref-panel__reset-btn"
          onClick={() => void handleReset()}
        >
          {t('appearance.fontSize.resetButton')}
        </button>
      </ConfigPageRow>
    </ConfigPageSection>
  );
}
