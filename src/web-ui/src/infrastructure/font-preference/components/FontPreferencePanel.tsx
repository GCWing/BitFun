import {
  FieldGroup,
  Input,
  NumberInput,
  SegmentedControl,
  type SegmentedControlOption,
} from '@bitfun/ui';
import { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ConfigPageRow, ConfigPageSection } from '@/infrastructure/config/components/common';
import { useFontPreference } from '../hooks/useFontPreference';
import { FontSizeLevel, PRESET_UI_BASE_PX, UI_FONT_SIZE_PRESETS } from '../types';
import './FontPreferencePanel.scss';

const UI_LEVELS: Array<Exclude<FontSizeLevel, 'custom'>> = ['compact', 'small', 'default', 'medium', 'large'];

export function FontPreferencePanel() {
  const { t } = useTranslation('settings/application');
  const { preference, setUiSize } = useFontPreference();

  const { level, customPx } = preference.uiSize;
  const [customInput, setCustomInput] = useState<string>(String(customPx ?? 14));
  const [previewText, setPreviewText] = useState('');

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

  const previewBasePx = level === 'custom'
    ? (parseInt(customInput, 10) || 14)
    : parseInt(UI_FONT_SIZE_PRESETS[level].base, 10);

  const levelOptions = useMemo<SegmentedControlOption[]>(
    () => [
      ...UI_LEVELS.map((l) => ({
        value: l,
        label: t(`appearance.fontSize.levels.${l}`),
      })),
      {
        value: 'custom',
        label: t('appearance.fontSize.levels.custom'),
      },
    ],
    [t]
  );

  return (
    <div
      data-testid="appearance-font-section"
      data-bf-component="font-preference"
      data-bf-part="root"
    >
      <ConfigPageSection
        bodySurface={false}
        className="font-pref-panel__section"
        title={t('appearance.fontSize.title')}
        description={t('appearance.fontSize.hint')}
      >
        <FieldGroup
          appearance="subtle"
          className="font-pref-panel__surface"
          dividers={false}
          fieldSurface="default"
        >
          <ConfigPageRow
            className="font-pref-panel__row--ui"
            label={t('appearance.fontSize.uiSizeLabel')}
            description={t('appearance.fontSize.uiSizeHint')}
            align="start"
            multiline
          >
            <div className="font-pref-panel__ui-size">
              <div
                className="font-pref-panel__level-buttons"
                data-testid="appearance-ui-font-level-group"
              >
                <SegmentedControl
                  className="font-pref-panel__level-control"
                  aria-label={t('appearance.fontSize.uiSizeLabel')}
                  options={levelOptions}
                  size="md"
                  tone="neutral"
                  value={level}
                  variant="pills"
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
                      size="md"
                      decrementLabel={`${t('appearance.fontSize.customPxLabel')} −1`}
                      incrementLabel={`${t('appearance.fontSize.customPxLabel')} +1`}
                      onValueChange={handleCustomValueChange}
                      aria-label={t('appearance.fontSize.customPxLabel')}
                      onFocus={() => void handleLevelClick('custom')}
                    />
                  </div>
                )}
              </div>

              {/* Editable live preview */}
              <div
                className="font-pref-panel__preview"
                data-bf-component="font-preference"
                data-bf-part="preview"
              >
                <Input
                  className="font-pref-panel__preview-input"
                  aria-label={t('appearance.fontSize.previewLabel')}
                  autoComplete="off"
                  data-testid="appearance-ui-font-preview-input"
                  onValueChange={setPreviewText}
                  placeholder={t('appearance.fontSize.previewPlaceholder')}
                  size="md"
                  spellCheck={false}
                  style={{ fontSize: `${previewBasePx}px` }}
                  value={previewText}
                />
              </div>
            </div>
          </ConfigPageRow>
        </FieldGroup>
      </ConfigPageSection>
    </div>
  );
}
