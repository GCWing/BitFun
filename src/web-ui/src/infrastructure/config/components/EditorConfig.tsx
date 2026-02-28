 

import React, { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { NumberInput, Select, Button, Switch, ConfigPageLoading, ConfigPageMessage } from '@/component-library';
import { RotateCcw } from 'lucide-react';
import { configManager } from '../services/ConfigManager';
import { globalEventBus } from '@/infrastructure/event-bus';
import { DEFAULT_EDITOR_CONFIG, type EditorConfig as EditorConfigType, type EditorConfigPartial } from '@/tools/editor/config';
import {
  ConfigPageLayout,
  ConfigPageHeader,
  ConfigPageContent,
  ConfigPageSection,
  ConfigPageRow,
} from './common';
import { createLogger } from '@/shared/utils/logger';
import './EditorConfig.scss';

const log = createLogger('EditorConfig');


const AUTO_SAVE_DELAY = 500;

export interface EditorConfigProps {}


const fontFamilyOptions = [
  { label: 'Fira Code', value: 'Fira Code' },
  { label: 'Noto Sans SC', value: 'Noto Sans SC' },
  { label: 'Consolas', value: 'Consolas' },
  { label: 'Courier New', value: 'Courier New' },
];




const cursorStyleOptions = [
  { label: 'line', value: 'line', labelKey: 'appearance.cursorStyles.line' },
  { label: 'line-thin', value: 'line-thin', labelKey: 'appearance.cursorStyles.lineThin' },
  { label: 'block', value: 'block', labelKey: 'appearance.cursorStyles.block' },
  { label: 'block-outline', value: 'block-outline', labelKey: 'appearance.cursorStyles.blockOutline' },
  { label: 'underline', value: 'underline', labelKey: 'appearance.cursorStyles.underline' },
  { label: 'underline-thin', value: 'underline-thin', labelKey: 'appearance.cursorStyles.underlineThin' },
];


const cursorBlinkingOptions = [
  { label: 'blink', value: 'blink', labelKey: 'appearance.cursorBlinkings.blink' },
  { label: 'smooth', value: 'smooth', labelKey: 'appearance.cursorBlinkings.smooth' },
  { label: 'phase', value: 'phase', labelKey: 'appearance.cursorBlinkings.phase' },
  { label: 'expand', value: 'expand', labelKey: 'appearance.cursorBlinkings.expand' },
  { label: 'solid', value: 'solid', labelKey: 'appearance.cursorBlinkings.solid' },
];


const wordWrapOptions = [
  { label: 'off', value: 'off', labelKey: 'behavior.wordWrapOptions.off' },
  { label: 'on', value: 'on', labelKey: 'behavior.wordWrapOptions.on' },
  { label: 'wordWrapColumn', value: 'wordWrapColumn', labelKey: 'behavior.wordWrapOptions.wordWrapColumn' },
  { label: 'bounded', value: 'bounded', labelKey: 'behavior.wordWrapOptions.bounded' },
];


const lineNumbersOptions = [
  { label: 'on', value: 'on', labelKey: 'behavior.lineNumberOptions.on' },
  { label: 'off', value: 'off', labelKey: 'behavior.lineNumberOptions.off' },
  { label: 'relative', value: 'relative', labelKey: 'behavior.lineNumberOptions.relative' },
  { label: 'interval', value: 'interval', labelKey: 'behavior.lineNumberOptions.interval' },
];


const minimapSideOptions = [
  { label: 'left', value: 'left', labelKey: 'display.minimapPositionLeft' },
  { label: 'right', value: 'right', labelKey: 'display.minimapPositionRight' },
];


const minimapSizeOptions = [
  { label: 'proportional', value: 'proportional', labelKey: 'display.minimapSizeAuto' },
  { label: 'fill', value: 'fill', labelKey: 'display.minimapSizeFill' },
  { label: 'fit', value: 'fit', labelKey: 'display.minimapSizeFit' },
];


const renderWhitespaceOptions = [
  { label: 'none', value: 'none', labelKey: 'display.whitespaceOptions.none' },
  { label: 'boundary', value: 'boundary', labelKey: 'display.whitespaceOptions.boundary' },
  { label: 'selection', value: 'selection', labelKey: 'display.whitespaceOptions.selection' },
  { label: 'trailing', value: 'trailing', labelKey: 'display.whitespaceOptions.trailing' },
  { label: 'all', value: 'all', labelKey: 'display.whitespaceOptions.all' },
];


const renderLineHighlightOptions = [
  { label: 'none', value: 'none', labelKey: 'display.lineHighlightOptions.none' },
  { label: 'gutter', value: 'gutter', labelKey: 'display.lineHighlightOptions.gutter' },
  { label: 'line', value: 'line', labelKey: 'display.lineHighlightOptions.line' },
  { label: 'all', value: 'all', labelKey: 'display.lineHighlightOptions.all' },
];

 
function getPrimaryFont(fontFamily: string): string {
  
  const fonts = fontFamily.split(',').map(f => f.trim().replace(/^['"]|['"]$/g, ''));
  
  const primary = fonts[0] || 'Fira Code';
  return primary;
}

 
function buildFontFamily(primaryFont: string): string {
  
  const fallbackFonts = ['Consolas', 'Monaco', 'Menlo', "'Courier New'", 'monospace'];
  const fonts = [primaryFont, ...fallbackFonts.filter(f => f !== primaryFont && f !== `'${primaryFont}'`)];
  return fonts.map(f => f.includes(' ') && !f.startsWith("'") ? `'${f}'` : f).join(', ');
}

 
function convertToSnakeCase(config: EditorConfigPartial): Record<string, any> {
  const result: Record<string, any> = {};
  
  if (config.fontSize !== undefined) result.font_size = config.fontSize;
  if (config.fontFamily !== undefined) result.font_family = config.fontFamily;
  if (config.fontWeight !== undefined) result.font_weight = config.fontWeight;
  if (config.lineHeight !== undefined) result.line_height = config.lineHeight;
  if (config.tabSize !== undefined) result.tab_size = config.tabSize;
  if (config.insertSpaces !== undefined) result.insert_spaces = config.insertSpaces;
  if (config.wordWrap !== undefined) result.word_wrap = config.wordWrap;
  if (config.lineNumbers !== undefined) result.line_numbers = config.lineNumbers;
  if (config.theme !== undefined) result.theme = config.theme;
  if (config.autoSave !== undefined) result.auto_save = config.autoSave;
  if (config.autoSaveDelay !== undefined) result.auto_save_delay = config.autoSaveDelay;
  if (config.formatOnSave !== undefined) result.format_on_save = config.formatOnSave;
  if (config.formatOnPaste !== undefined) result.format_on_paste = config.formatOnPaste;
  if (config.trimAutoWhitespace !== undefined) result.trim_auto_whitespace = config.trimAutoWhitespace;
  if (config.renderWhitespace !== undefined) result.render_whitespace = config.renderWhitespace;
  if (config.renderLineHighlight !== undefined) result.render_line_highlight = config.renderLineHighlight;
  if (config.cursorStyle !== undefined) result.cursor_style = config.cursorStyle;
  if (config.cursorBlinking !== undefined) result.cursor_blinking = config.cursorBlinking;
  if (config.scrollBeyondLastLine !== undefined) result.scroll_beyond_last_line = config.scrollBeyondLastLine;
  if (config.smoothScrolling !== undefined) result.smooth_scrolling = config.smoothScrolling;
  if (config.semanticHighlighting !== undefined) result.semantic_highlighting = config.semanticHighlighting;
  if (config.bracketPairColorization !== undefined) result.bracket_pair_colorization = config.bracketPairColorization;
  
  if (config.minimap) {
    result.minimap = {
      enabled: config.minimap.enabled,
      side: config.minimap.side,
      size: config.minimap.size,
    };
  }
  
  return result;
}

 
function convertToCamelCase(config: Record<string, any>): EditorConfigPartial {
  const result: EditorConfigPartial = {};
  
  if (config.font_size !== undefined) result.fontSize = config.font_size;
  if (config.font_family !== undefined) result.fontFamily = config.font_family;
  if (config.font_weight !== undefined) result.fontWeight = config.font_weight;
  if (config.line_height !== undefined) result.lineHeight = config.line_height;
  if (config.tab_size !== undefined) result.tabSize = config.tab_size;
  if (config.insert_spaces !== undefined) result.insertSpaces = config.insert_spaces;
  if (config.word_wrap !== undefined) result.wordWrap = config.word_wrap;
  if (config.line_numbers !== undefined) result.lineNumbers = config.line_numbers;
  if (config.theme !== undefined) result.theme = config.theme;
  if (config.auto_save !== undefined) result.autoSave = config.auto_save;
  if (config.auto_save_delay !== undefined) result.autoSaveDelay = config.auto_save_delay;
  if (config.format_on_save !== undefined) result.formatOnSave = config.format_on_save;
  if (config.format_on_paste !== undefined) result.formatOnPaste = config.format_on_paste;
  if (config.trim_auto_whitespace !== undefined) result.trimAutoWhitespace = config.trim_auto_whitespace;
  if (config.render_whitespace !== undefined) result.renderWhitespace = config.render_whitespace;
  if (config.render_line_highlight !== undefined) result.renderLineHighlight = config.render_line_highlight;
  if (config.cursor_style !== undefined) result.cursorStyle = config.cursor_style;
  if (config.cursor_blinking !== undefined) result.cursorBlinking = config.cursor_blinking;
  if (config.scroll_beyond_last_line !== undefined) result.scrollBeyondLastLine = config.scroll_beyond_last_line;
  if (config.smooth_scrolling !== undefined) result.smoothScrolling = config.smooth_scrolling;
  if (config.semantic_highlighting !== undefined) result.semanticHighlighting = config.semantic_highlighting;
  if (config.bracket_pair_colorization !== undefined) result.bracketPairColorization = config.bracket_pair_colorization;
  
  if (config.minimap) {
    result.minimap = {
      enabled: config.minimap.enabled,
      side: config.minimap.side,
      size: config.minimap.size,
    };
  }
  
  return result;
}

const EditorConfig: React.FC<EditorConfigProps> = () => {
  const { t } = useTranslation('settings/editor');
  
  
  const fontWeightOptionsTranslated = [
    { label: t('appearance.fontWeightNormal'), value: 'normal' },
    { label: t('appearance.fontWeightBold'), value: 'bold' },
  ];
  
  
  const cursorStyleOptionsTranslated = cursorStyleOptions.map(o => ({ ...o, label: t(o.labelKey) }));
  const cursorBlinkingOptionsTranslated = cursorBlinkingOptions.map(o => ({ ...o, label: t(o.labelKey) }));
  const wordWrapOptionsTranslated = wordWrapOptions.map(o => ({ ...o, label: t(o.labelKey) }));
  const lineNumbersOptionsTranslated = lineNumbersOptions.map(o => ({ ...o, label: t(o.labelKey) }));
  const minimapSideOptionsTranslated = minimapSideOptions.map(o => ({ ...o, label: t(o.labelKey) }));
  const minimapSizeOptionsTranslated = minimapSizeOptions.map(o => ({ ...o, label: t(o.labelKey) }));
  const renderWhitespaceOptionsTranslated = renderWhitespaceOptions.map(o => ({ ...o, label: t(o.labelKey) }));
  const renderLineHighlightOptionsTranslated = renderLineHighlightOptions.map(o => ({ ...o, label: t(o.labelKey) }));
  
  
  const [config, setConfig] = useState<EditorConfigType>({ ...DEFAULT_EDITOR_CONFIG });
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [statusMessage, setStatusMessage] = useState<{ type: 'success' | 'error' | 'warning'; text: string } | null>(null);
  
  
  const isInitialLoadRef = useRef(true);
  const autoSaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const configRef = useRef<EditorConfigType>(config);
  
  
  useEffect(() => {
    configRef.current = config;
  }, [config]);

  
  useEffect(() => {
    loadConfig();
  }, []);

  const loadConfig = async () => {
    try {
      setIsLoading(true);
      isInitialLoadRef.current = true;
      const backendConfig = await configManager.getConfig<Record<string, any>>('editor');
      if (backendConfig) {
        const camelCaseConfig = convertToCamelCase(backendConfig);
        setConfig({ ...DEFAULT_EDITOR_CONFIG, ...camelCaseConfig });
      }
      
      setTimeout(() => {
        isInitialLoadRef.current = false;
      }, 100);
    } catch (error) {
      log.error('Failed to load config', error);
      setStatusMessage({ 
        type: 'error', 
        text: t('messages.loadFailed') 
      });
    } finally {
      setIsLoading(false);
    }
  };

  
  const doSave = useCallback(async (configToSave: EditorConfigType) => {
    try {
      setIsSaving(true);
      setStatusMessage(null);

      
      const snakeCaseConfig = convertToSnakeCase(configToSave);
      await configManager.setConfig('editor', snakeCaseConfig);

      
      globalEventBus.emit('editor:config:changed', snakeCaseConfig);

      setStatusMessage({ 
        type: 'success', 
        text: t('messages.saveSuccess') 
      });

      
      setTimeout(() => setStatusMessage(null), 3000);
    } catch (error) {
      log.error('Failed to save config', error);
      setStatusMessage({ 
        type: 'error', 
        text: `${t('messages.saveFailed')}: ` + (error instanceof Error ? error.message : String(error))
      });
    } finally {
      setIsSaving(false);
    }
  }, [t]);

  
  useEffect(() => {
    
    if (isInitialLoadRef.current || isLoading) {
      return;
    }

    
    if (autoSaveTimerRef.current) {
      clearTimeout(autoSaveTimerRef.current);
    }

    
    autoSaveTimerRef.current = setTimeout(() => {
      doSave(configRef.current);
    }, AUTO_SAVE_DELAY);

    
    return () => {
      if (autoSaveTimerRef.current) {
        clearTimeout(autoSaveTimerRef.current);
      }
    };
  }, [config, isLoading, doSave]);

  const resetConfig = useCallback(async () => {
    if (await window.confirm(t('messages.confirmReset'))) {
      setConfig({ ...DEFAULT_EDITOR_CONFIG });
      setStatusMessage({ 
        type: 'warning', 
        text: t('messages.resetDone') 
      });
    }
  }, [t]);

  const updateConfig = useCallback(<K extends keyof EditorConfigType>(
    key: K,
    value: EditorConfigType[K]
  ) => {
    setConfig(prev => ({ ...prev, [key]: value }));
    if (statusMessage?.type === 'success') {
      setStatusMessage(null);
    }
  }, [statusMessage]);

  const updateMinimapConfig = useCallback((key: keyof EditorConfigType['minimap'], value: any) => {
    setConfig(prev => ({
      ...prev,
      minimap: {
        ...prev.minimap,
        [key]: value
      }
    }));
  }, []);

  if (isLoading) {
    return (
      <ConfigPageLayout className="bitfun-editor-config">
        <ConfigPageHeader
          title={t('title')}
          subtitle={t('subtitle')}
        />
        <ConfigPageContent>
          <ConfigPageLoading text={t('messages.loading')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout className="bitfun-editor-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />

      <ConfigPageContent className="bitfun-editor-config__content">
        <ConfigPageSection
          title={t('sections.appearance.title')}
          description={t('sections.appearance.description')}
        >
          <ConfigPageRow label={t('appearance.font')} description={t('appearance.fontHint')} align="center">
            <Select
              options={fontFamilyOptions}
              value={getPrimaryFont(config.fontFamily)}
              onChange={(v) => updateConfig('fontFamily', buildFontFamily(v as string))}
              placeholder={t('appearance.font')}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('appearance.fontWeight')} align="center">
            <Select
              options={fontWeightOptionsTranslated}
              value={config.fontWeight}
              onChange={(v) => updateConfig('fontWeight', v as typeof config.fontWeight)}
              placeholder={t('appearance.fontWeight')}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('appearance.fontSize')} align="center">
            <NumberInput
              value={config.fontSize}
              onChange={(v) => updateConfig('fontSize', v)}
              min={10}
              max={32}
              step={1}
              unit="px"
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('appearance.lineHeight')} align="center">
            <NumberInput
              value={config.lineHeight}
              onChange={(v) => updateConfig('lineHeight', v)}
              min={1.0}
              max={3.0}
              step={0.1}
              precision={1}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('appearance.cursorStyle')} align="center">
            <Select
              options={cursorStyleOptionsTranslated}
              value={config.cursorStyle}
              onChange={(v) => updateConfig('cursorStyle', v as typeof config.cursorStyle)}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('appearance.cursorBlinking')} align="center">
            <Select
              options={cursorBlinkingOptionsTranslated}
              value={config.cursorBlinking}
              onChange={(v) => updateConfig('cursorBlinking', v as typeof config.cursorBlinking)}
              size="small"
            />
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection
          title={t('sections.behavior.title')}
          description={t('sections.behavior.description')}
        >
          <ConfigPageRow label={t('behavior.tabSize')} align="center">
            <NumberInput
              value={config.tabSize}
              onChange={(v) => updateConfig('tabSize', v)}
              min={1}
              max={8}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('behavior.insertSpaces')} description={t('behavior.insertSpacesDesc')} align="center">
            <Switch
              checked={config.insertSpaces}
              onChange={(e) => updateConfig('insertSpaces', e.target.checked)}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('behavior.wordWrap')} align="center">
            <Select
              options={wordWrapOptionsTranslated}
              value={config.wordWrap}
              onChange={(v) => updateConfig('wordWrap', v as typeof config.wordWrap)}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('behavior.lineNumbers')} align="center">
            <Select
              options={lineNumbersOptionsTranslated}
              value={config.lineNumbers}
              onChange={(v) => updateConfig('lineNumbers', v as typeof config.lineNumbers)}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('behavior.smoothScrolling')} description={t('behavior.smoothScrollingDesc')} align="center">
            <Switch
              checked={config.smoothScrolling}
              onChange={(e) => updateConfig('smoothScrolling', e.target.checked)}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('behavior.scrollBeyondLastLine')} description={t('behavior.scrollBeyondLastLineDesc')} align="center">
            <Switch
              checked={config.scrollBeyondLastLine}
              onChange={(e) => updateConfig('scrollBeyondLastLine', e.target.checked)}
              size="small"
            />
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection
          title={t('sections.display.title')}
          description={t('sections.display.description')}
        >
          <ConfigPageRow label={t('display.minimap')} description={t('display.minimapDesc')} align="center">
            <Switch
              checked={config.minimap.enabled}
              onChange={(e) => updateMinimapConfig('enabled', e.target.checked)}
              size="small"
            />
          </ConfigPageRow>
          {config.minimap.enabled && (
            <>
              <ConfigPageRow label={t('display.minimapPosition')} align="center">
                <Select
                  options={minimapSideOptionsTranslated}
                  value={config.minimap.side}
                  onChange={(v) => updateMinimapConfig('side', v as string)}
                  size="small"
                />
              </ConfigPageRow>
              <ConfigPageRow label={t('display.minimapSize')} align="center">
                <Select
                  options={minimapSizeOptionsTranslated}
                  value={config.minimap.size}
                  onChange={(v) => updateMinimapConfig('size', v as string)}
                  size="small"
                />
              </ConfigPageRow>
            </>
          )}
          <ConfigPageRow label={t('display.whitespace')} align="center">
            <Select
              options={renderWhitespaceOptionsTranslated}
              value={config.renderWhitespace}
              onChange={(v) => updateConfig('renderWhitespace', v as typeof config.renderWhitespace)}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('display.lineHighlight')} align="center">
            <Select
              options={renderLineHighlightOptionsTranslated}
              value={config.renderLineHighlight}
              onChange={(v) => updateConfig('renderLineHighlight', v as typeof config.renderLineHighlight)}
              size="small"
            />
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection
          title={t('sections.advanced.title')}
          description={t('sections.advanced.description')}
        >
          <ConfigPageRow label={t('advanced.semanticHighlighting')} description={t('advanced.semanticHighlightingDesc')} align="center">
            <Switch
              checked={config.semanticHighlighting}
              onChange={(e) => updateConfig('semanticHighlighting', e.target.checked)}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('advanced.bracketPairColorization')} description={t('advanced.bracketPairColorizationDesc')} align="center">
            <Switch
              checked={config.bracketPairColorization}
              onChange={(e) => updateConfig('bracketPairColorization', e.target.checked)}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('advanced.formatOnSave')} description={t('advanced.formatOnSaveDesc')} align="center">
            <Switch
              checked={config.formatOnSave}
              onChange={(e) => updateConfig('formatOnSave', e.target.checked)}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('advanced.formatOnPaste')} description={t('advanced.formatOnPasteDesc')} align="center">
            <Switch
              checked={config.formatOnPaste}
              onChange={(e) => updateConfig('formatOnPaste', e.target.checked)}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('advanced.trimAutoWhitespace')} description={t('advanced.trimAutoWhitespaceDesc')} align="center">
            <Switch
              checked={config.trimAutoWhitespace}
              onChange={(e) => updateConfig('trimAutoWhitespace', e.target.checked)}
              size="small"
            />
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection
          title={t('actions.save')}
          description={t('messages.saving')}
        >
          <ConfigPageRow label={t('actions.reset')} description={t('messages.confirmReset')} align="center">
            <div className="bitfun-editor-config__actions">
              <Button
                variant="secondary"
                size="small"
                onClick={resetConfig}
                disabled={isSaving}
              >
                <RotateCcw size={14} />
                {t('actions.reset')}
              </Button>
              {isSaving && (
                <span className="bitfun-editor-config__saving">{t('messages.saving')}</span>
              )}
            </div>
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageMessage message={statusMessage} />
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default EditorConfig;
