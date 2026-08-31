import { Button, Textarea } from '@bitfun/ui';
import React from 'react';
import { Code2, RotateCcw, Save } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import './GenerativeWidgetPanel.scss';

export interface GenerativeWidgetPanelProps {
  title?: string;
  widgetId?: string;
  widgetCode?: string;
  onWidgetCodePersist?: (widgetCode: string) => Promise<void> | void;
}

const AUTO_SAVE_DELAY_MS = 600;

export const GenerativeWidgetPanel: React.FC<GenerativeWidgetPanelProps> = ({
  widgetCode,
  onWidgetCodePersist,
}) => {
  const { t } = useTranslation('flow-chat');
  const [draftCode, setDraftCode] = React.useState(widgetCode ?? '');
  const [saveState, setSaveState] = React.useState<'saved' | 'saving' | 'unsaved' | 'error'>('saved');
  const [saveError, setSaveError] = React.useState<string | null>(null);
  const savedCodeRef = React.useRef(widgetCode ?? '');

  React.useEffect(() => {
    const externalCode = widgetCode ?? '';
    if (externalCode === savedCodeRef.current || draftCode !== savedCodeRef.current) {
      return;
    }

    savedCodeRef.current = externalCode;
    setDraftCode(externalCode);
    setSaveState('saved');
    setSaveError(null);
  }, [draftCode, widgetCode]);

  const persistWidgetCode = React.useCallback(async (nextCode: string) => {
    if (nextCode === savedCodeRef.current) {
      setSaveState('saved');
      setSaveError(null);
      return;
    }

    setSaveState('saving');
    setSaveError(null);

    try {
      await onWidgetCodePersist?.(nextCode);
      savedCodeRef.current = nextCode;
      setSaveState('saved');
    } catch (error) {
      setSaveState('error');
      setSaveError(error instanceof Error ? error.message : t('toolCards.generativeUI.saveError'));
    }
  }, [onWidgetCodePersist, t]);

  React.useEffect(() => {
    if (draftCode === savedCodeRef.current) {
      if (saveState !== 'saving' && saveState !== 'error') {
        setSaveState('saved');
      }
      return;
    }

    setSaveState(current => (current === 'saving' ? current : 'unsaved'));
    const timer = window.setTimeout(() => {
      void persistWidgetCode(draftCode);
    }, AUTO_SAVE_DELAY_MS);

    return () => window.clearTimeout(timer);
  }, [draftCode, persistWidgetCode, saveState]);

  const handleReset = React.useCallback(() => {
    setDraftCode(savedCodeRef.current);
    setSaveState('saved');
    setSaveError(null);
  }, []);

  const handleSaveNow = React.useCallback(() => {
    void persistWidgetCode(draftCode);
  }, [draftCode, persistWidgetCode]);

  const saveLabel = React.useMemo(() => {
    switch (saveState) {
      case 'saving':
        return t('toolCards.generativeUI.saving');
      case 'unsaved':
        return t('toolCards.generativeUI.unsaved');
      case 'error':
        return t('toolCards.generativeUI.saveFailed');
      case 'saved':
      default:
        return t('toolCards.generativeUI.savedToSession');
    }
  }, [saveState, t]);

  if (!draftCode) {
    return (
      <div className="bitfun-generative-widget-panel bitfun-generative-widget-panel--empty" data-bf-component="generative-widget" data-bf-part="empty" data-bf-state="empty">
        <div className="bitfun-generative-widget-panel__empty-copy">
          {t('toolCards.generativeUI.empty')}
        </div>
      </div>
    );
  }

  return (
    <div className="bitfun-generative-widget-panel" data-bf-component="generative-widget" data-bf-part="root">
      <div className="bitfun-generative-widget-panel__toolbar" data-bf-component="generative-widget" data-bf-part="toolbar">
        <div className="bitfun-generative-widget-panel__toolbar-meta" data-bf-component="generative-widget" data-bf-part="toolbarMeta">
          <span className={`bitfun-generative-widget-panel__save-state bitfun-generative-widget-panel__save-state--${saveState}`}>
            {saveLabel}
          </span>
        </div>
        <div className="bitfun-generative-widget-panel__toolbar-actions" data-bf-component="generative-widget" data-bf-part="toolbarActions">
          <Button
            variant="outline"
            size="sm"
            leadingIcon={<RotateCcw size={14} />}
            onClick={handleReset}
            disabled={draftCode === savedCodeRef.current}
          >
            {t('toolCards.generativeUI.reset')}
          </Button>
          <Button
            variant="fill"
            size="sm"
            leadingIcon={<Save size={14} />}
            loading={saveState === 'saving'}
            onClick={handleSaveNow}
            disabled={draftCode === savedCodeRef.current}
          >
            {t('toolCards.generativeUI.saveNow')}
          </Button>
        </div>
      </div>

      <div className="bitfun-generative-widget-panel__workspace" data-bf-component="generative-widget" data-bf-part="workspace">
        <section className="bitfun-generative-widget-panel__pane bitfun-generative-widget-panel__pane--editor" data-bf-component="generative-widget" data-bf-part="editorPane">
          <div className="bitfun-generative-widget-panel__pane-header" data-bf-component="generative-widget" data-bf-part="paneHeader">
            <span className="bitfun-generative-widget-panel__pane-title">
              <Code2 size={14} />
              <span>{t('toolCards.generativeUI.source')}</span>
            </span>
          </div>
          <Textarea
            className="bitfun-generative-widget-panel__editor-field"
            font="mono"
            layout="fill"
            resize="none"
            data-bf-component="generative-widget"
            data-bf-part="editor"
            value={draftCode}
            onValueChange={(value) => {
              setDraftCode(value);
              if (saveState === 'error') {
                setSaveState('unsaved');
                setSaveError(null);
              }
            }}
            spellCheck={false}
            aria-label={t('toolCards.generativeUI.source')}
          />
        </section>
      </div>

      {saveError && (
        <div className="bitfun-generative-widget-panel__error" data-bf-component="generative-widget" data-bf-part="error">
          {saveError}
        </div>
      )}
    </div>
  );
};

export default GenerativeWidgetPanel;
