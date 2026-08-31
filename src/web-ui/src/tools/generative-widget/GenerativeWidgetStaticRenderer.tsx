import React, { useEffect, useMemo, useRef } from 'react';
import { GENERATIVE_WIDGET_SHELL_HTML } from './GenerativeWidgetFrame';
import { readWidgetAppearancePayload } from './appearancePayload';

export interface GenerativeWidgetStaticRendererProps {
  widgetCode: string;
  className?: string;
}

function extractShellCss(html: string): string {
  const match = html.match(/<style>([\s\S]*?)<\/style>/);
  const css = match?.[1] ?? '';
  return css.replace(
    /^(\s*):root\s*\{/m,
    '$1.bitfun-generative-widget-static-renderer {',
  );
}

function runScripts(root: HTMLElement): void {
  const scripts = root.querySelectorAll('script');
  scripts.forEach((oldScript) => {
    const nextScript = document.createElement('script');
    for (let i = 0; i < oldScript.attributes.length; i += 1) {
      const attr = oldScript.attributes[i];
      nextScript.setAttribute(attr.name, attr.value);
    }
    if (oldScript.src) {
      nextScript.src = oldScript.src;
    } else {
      nextScript.textContent = oldScript.textContent;
    }
    oldScript.parentNode?.replaceChild(nextScript, oldScript);
  });
}

export const GenerativeWidgetStaticRenderer: React.FC<GenerativeWidgetStaticRendererProps> = ({
  widgetCode,
  className = '',
}) => {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const shellCss = useMemo(
    () => extractShellCss(GENERATIVE_WIDGET_SHELL_HTML),
    [],
  );
  const appearancePayload = useMemo(() => readWidgetAppearancePayload(), []);

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;

    const globalWindow = window as Window & {
      bitfunWidget?: { send: (data: unknown) => void };
      glimpse?: { send: (data: unknown) => void };
      sendPrompt?: (text: string) => void;
    };

    const prevBridge = globalWindow.bitfunWidget;
    const prevGlimpse = globalWindow.glimpse;
    const prevSendPrompt = globalWindow.sendPrompt;

    const noopBridge = { send: (_data: unknown) => {} };
    globalWindow.bitfunWidget = noopBridge;
    globalWindow.glimpse = noopBridge;
    globalWindow.sendPrompt = (_text: string) => {};

    root.innerHTML = String(widgetCode || '');
    runScripts(root);

    return () => {
      root.innerHTML = '';
      globalWindow.bitfunWidget = prevBridge;
      globalWindow.glimpse = prevGlimpse;
      globalWindow.sendPrompt = prevSendPrompt;
    };
  }, [widgetCode]);

  const appearanceStyle = useMemo(() => {
    const style: React.CSSProperties & Record<string, string> = {
      background: 'transparent',
      color: 'var(--bf-color-content-primary)',
      width: '100%',
    };

    Object.entries(appearancePayload?.vars ?? {}).forEach(([key, value]) => {
      style[key] = value;
    });

    return style;
  }, [appearancePayload]);

  return (
    <div
      className={`bitfun-generative-widget-static-renderer ${className}`.trim()}
      style={appearanceStyle}
      data-bf-appearance={appearancePayload?.id ?? 'unknown'}
      data-bf-appearance-mode={appearancePayload?.mode ?? 'dark'}
    >
      <style>{shellCss}</style>
      <div ref={rootRef} className="bitfun-generative-widget-static-renderer__root" />
    </div>
  );
};

export default GenerativeWidgetStaticRenderer;
