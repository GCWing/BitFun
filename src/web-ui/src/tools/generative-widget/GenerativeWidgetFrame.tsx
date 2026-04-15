import React, { useEffect, useMemo, useRef, useState } from 'react';
import morphdomRuntime from 'morphdom/dist/morphdom-umd.js?raw';
import './GenerativeWidgetFrame.scss';

type WidgetMessage =
  | {
      source: 'bitfun-widget';
      type: 'bitfun-widget:event';
      widgetId?: string;
      payload?: unknown;
    }
  | {
      source: 'bitfun-widget';
      type: 'bitfun-widget:prompt';
      widgetId?: string;
      text?: string;
    }
  | {
      source: 'bitfun-widget';
      type: 'bitfun-widget:ready';
      widgetId?: string;
    }
  | {
      source: 'bitfun-widget';
      type: 'bitfun-widget:open-file';
      widgetId?: string;
      filePath?: string;
      line?: number;
      column?: number;
      lineEnd?: number;
      nodeType?: string;
    }
  | {
      source: 'bitfun-widget';
      type: 'bitfun-widget:resize';
      widgetId?: string;
      height?: number;
    };

export interface GenerativeWidgetFrameProps {
  widgetId: string;
  title?: string;
  widgetCode: string;
  executeScripts?: boolean;
  className?: string;
  onWidgetEvent?: (event: WidgetMessage) => void;
  onHeightChange?: (height: number) => void;
}

const SHELL_HTML = `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <style>
    * { box-sizing: border-box; }
    html, body {
      margin: 0;
      padding: 0;
      width: 100%;
      min-height: 100%;
      background: transparent;
      color: #e8e8e8;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      overflow: hidden;
    }
    body { min-height: 100%; }
    #root { min-height: 100%; width: 100%; }
    @keyframes bitfunWidgetFadeIn {
      from { opacity: 0; transform: translateY(4px); }
      to { opacity: 1; transform: translateY(0); }
    }
  </style>
  <script>${morphdomRuntime}</script>
</head>
<body>
  <div id="root"></div>
  <script>
    (function () {
      var currentWidgetId = '';
      var lastExecutedHtml = '';
      var resizeFrame = null;
      var resizeObserver = null;

      function send(type, payload) {
        parent.postMessage({
          source: 'bitfun-widget',
          type: type,
          widgetId: currentWidgetId,
          payload: payload
        }, '*');
      }

      function sendMessage(message) {
        parent.postMessage(message, '*');
      }

      function measureHeight() {
        var root = document.getElementById('root');
        var rootRectHeight = root ? root.getBoundingClientRect().height : 0;
        return Math.max(
          Math.ceil(rootRectHeight),
          root ? root.scrollHeight : 0,
          document.body ? document.body.scrollHeight : 0,
          document.documentElement ? document.documentElement.scrollHeight : 0,
          120
        );
      }

      function scheduleResize() {
        if (resizeFrame !== null) return;
        resizeFrame = window.requestAnimationFrame(function () {
          resizeFrame = null;
          sendMessage({
            source: 'bitfun-widget',
            type: 'bitfun-widget:resize',
            widgetId: currentWidgetId,
            height: measureHeight()
          });
        });
      }

      function runScripts(root) {
        var scripts = root.querySelectorAll('script');
        scripts.forEach(function (oldScript) {
          var nextScript = document.createElement('script');
          for (var i = 0; i < oldScript.attributes.length; i += 1) {
            var attr = oldScript.attributes[i];
            nextScript.setAttribute(attr.name, attr.value);
          }
          if (oldScript.src) {
            nextScript.src = oldScript.src;
          } else {
            nextScript.textContent = oldScript.textContent;
          }
          oldScript.parentNode.replaceChild(nextScript, oldScript);
        });
      }

      function setContent(html, shouldRunScripts) {
        var root = document.getElementById('root');
        if (!root) return;
        var nextHtml = String(html || '');

        if (window.morphdom) {
          var target = document.createElement('div');
          target.id = 'root';
          target.innerHTML = nextHtml;

          window.morphdom(root, target, {
            onBeforeElUpdated: function (fromEl, toEl) {
              if (fromEl.isEqualNode && fromEl.isEqualNode(toEl)) {
                return false;
              }
              return true;
            },
            onNodeAdded: function (node) {
              if (
                node &&
                node.nodeType === 1 &&
                node.tagName !== 'SCRIPT' &&
                node.tagName !== 'STYLE'
              ) {
                node.style.animation = 'bitfunWidgetFadeIn 0.18s ease both';
              }
              return node;
            }
          });
        } else {
          root.innerHTML = nextHtml;
        }

        if (shouldRunScripts && html !== lastExecutedHtml) {
          lastExecutedHtml = html || '';
          runScripts(root);
        }

        scheduleResize();
      }

      var bridge = {
        send: function (data) {
          send('bitfun-widget:event', data);
        }
      };

      window.bitfunWidget = bridge;
      window.glimpse = bridge;
      window.sendPrompt = function (text) {
        parent.postMessage({
          source: 'bitfun-widget',
          type: 'bitfun-widget:prompt',
          widgetId: currentWidgetId,
          text: String(text || '')
        }, '*');
      };

      document.addEventListener('click', function (event) {
        var target = event.target;
        var fileTarget = target && target.closest ? target.closest('[data-file-path], [data-bitfun-open-file]') : null;
        if (fileTarget) {
          var filePath = fileTarget.getAttribute('data-file-path') || fileTarget.getAttribute('data-bitfun-open-file') || '';
          if (filePath) {
            var lineValue = Number(fileTarget.getAttribute('data-line') || '');
            var columnValue = Number(fileTarget.getAttribute('data-column') || '');
            var lineEndValue = Number(fileTarget.getAttribute('data-line-end') || '');
            event.preventDefault();
            event.stopPropagation();
            sendMessage({
              source: 'bitfun-widget',
              type: 'bitfun-widget:open-file',
              widgetId: currentWidgetId,
              filePath: filePath,
              line: Number.isFinite(lineValue) && lineValue > 0 ? lineValue : undefined,
              column: Number.isFinite(columnValue) && columnValue > 0 ? columnValue : undefined,
              lineEnd: Number.isFinite(lineEndValue) && lineEndValue > 0 ? lineEndValue : undefined,
              nodeType: fileTarget.getAttribute('data-node-type') || undefined
            });
            return;
          }
        }

        var anchor = event.target && event.target.closest ? event.target.closest('a[href]') : null;
        if (!anchor) return;
        var href = anchor.getAttribute('href');
        if (!href || href.charAt(0) === '#') return;
        anchor.setAttribute('target', '_blank');
        anchor.setAttribute('rel', 'noreferrer noopener');
      }, true);

      window.addEventListener('message', function (event) {
        var data = event.data;
        if (!data || data.type !== 'bitfun-widget:update') return;
        currentWidgetId = data.widgetId || currentWidgetId || '';
        setContent(String(data.html || ''), Boolean(data.runScripts));
      });

      window.addEventListener('load', scheduleResize);
      if (window.ResizeObserver) {
        resizeObserver = new ResizeObserver(scheduleResize);
        resizeObserver.observe(document.documentElement);
        var root = document.getElementById('root');
        if (root) {
          resizeObserver.observe(root);
        }
      }

      sendMessage({
        source: 'bitfun-widget',
        type: 'bitfun-widget:ready',
        widgetId: currentWidgetId
      });
      scheduleResize();
    })();
  </script>
</body>
</html>`;

export const GenerativeWidgetFrame: React.FC<GenerativeWidgetFrameProps> = ({
  widgetId,
  title,
  widgetCode,
  executeScripts = false,
  className = '',
  onWidgetEvent,
  onHeightChange,
}) => {
  const iframeRef = useRef<HTMLIFrameElement | null>(null);
  const [isLoaded, setIsLoaded] = useState(false);
  const [frameHeight, setFrameHeight] = useState(160);
  const lastExecutedHtmlRef = useRef('');

  const normalizedCode = useMemo(() => widgetCode || '', [widgetCode]);

  useEffect(() => {
    const handleMessage = (event: MessageEvent<WidgetMessage>) => {
      const data = event.data;
      if (event.source !== iframeRef.current?.contentWindow) return;
      if (!data || data.source !== 'bitfun-widget') return;
      if (data.widgetId && data.widgetId !== widgetId) return;

      if (data.type === 'bitfun-widget:resize') {
        const nextHeight = Math.max(120, Math.ceil(Number(data.height) || 0));
        setFrameHeight((prev) => {
          if (Math.abs(prev - nextHeight) <= 1) return prev;
          onHeightChange?.(nextHeight);
          return nextHeight;
        });
        return;
      }

      onWidgetEvent?.(data);
    };

    window.addEventListener('message', handleMessage);
    return () => {
      window.removeEventListener('message', handleMessage);
    };
  }, [onHeightChange, onWidgetEvent, widgetId]);

  useEffect(() => {
    if (!isLoaded || !iframeRef.current?.contentWindow) return;

    const shouldRunScripts =
      Boolean(executeScripts) && lastExecutedHtmlRef.current !== normalizedCode;

    iframeRef.current.contentWindow.postMessage(
      {
        type: 'bitfun-widget:update',
        widgetId,
        title,
        html: normalizedCode,
        runScripts: shouldRunScripts,
      },
      '*',
    );

    if (shouldRunScripts) {
      lastExecutedHtmlRef.current = normalizedCode;
    }
  }, [executeScripts, isLoaded, normalizedCode, title, widgetId]);

  return (
    <div
      className={`bitfun-generative-widget-frame ${className}`.trim()}
      style={{ height: `${frameHeight}px` }}
    >
      <iframe
        ref={iframeRef}
        title={title || 'Generative widget'}
        className="bitfun-generative-widget-frame__iframe"
        sandbox="allow-scripts allow-forms allow-modals allow-popups"
        srcDoc={SHELL_HTML}
        onLoad={() => setIsLoaded(true)}
      />
    </div>
  );
};

export default GenerativeWidgetFrame;
