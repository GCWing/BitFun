import {
  Button,
  NumberInput,
  SegmentedControl,
  type SegmentedControlOption,
} from '@bitfun/ui';
import { useMemo, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { ConfigPageRow, ConfigPageSection } from '@/infrastructure/config/components/common';
import { useFontPreference } from '../hooks/useFontPreference';
import { FontSizeLevel, PRESET_UI_BASE_PX, UI_FONT_SIZE_PRESETS } from '../types';
import './FontPreferencePanel.scss';

const UI_LEVELS: Array<Exclude<FontSizeLevel, 'custom'>> = ['compact', 'small', 'default', 'medium', 'large'];

export function FontPreferencePanel() {
  const { t } = useTranslation('settings/application');
  const { preference, setUiSize, reset } = useFontPreference();

  const { level, customPx } = preference.uiSize;
  const [customInput, setCustomInput] = useState<string>(String(customPx ?? 14));

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
  }, [getEffectiveUiBasePx, setUiSize]);

  const handleCustomValueChange = (px: number) => {
    setCustomInput(String(px));
    void setUiSize('custom', px);
  };

  const handleReset = async () => {
    await reset();
    setCustomInput('14');
  };

  const previewBasePx = level === 'custom'
    ? (parseInt(customInput, 10) || 14)
    : parseInt(UI_FONT_SIZE_PRESETS[level].base, 10);

  const customLevelLabelPx = (() => {
    if (level !== 'custom') return 14;
    const n = parseInt(customInput, 10);
    return !isNaN(n) && n >= 12 && n <= 20 ? n : 14;
  })();

  const levelOptions = useMemo<SegmentedControlOption[]>(
    () => [
      ...UI_LEVELS.map((l) => ({
        value: l,
        label: (
          <span
            className="font-pref-panel__level-label"
            style={{ fontSize: UI_FONT_SIZE_PRESETS[l].base }}
          >
            {t(`appearance.fontSize.levels.${l}`)}
          </span>
        ),
      })),
      {
        value: 'custom',
        label: (
          <span
            className="font-pref-panel__level-label"
            style={{ fontSize: `${customLevelLabelPx}px` }}
          >
            {t('appearance.fontSize.levels.custom')}
          </span>
        ),
      },
    ],
    [t, customLevelLabelPx]
  );

  return (
    <div
      data-testid="appearance-font-section"
      data-bf-component="font-preference"
      data-bf-part="root"
    >
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
            <div
              className="font-pref-panel__level-buttons"
              data-testid="appearance-ui-font-level-group"
            >
              <SegmentedControl
                aria-label={t('appearance.fontSize.uiSizeLabel')}
                options={levelOptions}
                value={level}
                onValueChange={(value) => void handleLevelClick(value as FontSizeLevel)}
              />
              {level === 'custom' && (
                <div
                  className="font-pref-panel__custom-controls"
                  role="group"
                  aria-label={t('appearance.fontSize.customPxLabel')}
                  data-testid="appearance-ui-font-custom-controls"
                  data-bf-component="font-preference"
                  data-bf-part="customControls"
                >
                  <NumberInput
                    className="font-pref-panel__custom-number-input"
                    value={parseInt(customInput, 10) || 14}
                    min={12}
                    max={20}
                    step={1}
                    unit="px"
                    variant="stepper"
                    size="sm"
                    decrementLabel={`${t('appearance.fontSize.customPxLabel')} −1`}
                    incrementLabel={`${t('appearance.fontSize.customPxLabel')} +1`}
                    onValueChange={handleCustomValueChange}
                    aria-label={t('appearance.fontSize.customPxLabel')}
                    onFocus={() => void handleLevelClick('custom')}
                  />
                </div>
              )}
            </div>
          </div>

          {/* Live preview */}
          <div
            className="font-pref-panel__preview"
            style={{ fontSize: `${previewBasePx}px` }}
            aria-label="Font size preview"
            data-testid="appearance-ui-font-preview"
            data-bf-component="font-preference"
            data-bf-part="preview"
          >
            {t('appearance.fontSize.previewText')}
          </div>
        </div>
      </ConfigPageRow>

      {/* Reset */}
      <ConfigPageRow label="" align="center">
        <Button
          variant="outline"
          size="sm"
          className="font-pref-panel__reset-btn"
          onClick={() => void handleReset()}
          data-testid="appearance-font-reset-btn"
        >
          {t('appearance.fontSize.resetButton')}
        </Button>
      </ConfigPageRow>
      </ConfigPageSection>
    </div>
  );
}
