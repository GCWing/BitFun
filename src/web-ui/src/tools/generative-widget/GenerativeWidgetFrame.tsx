import React, { useEffect, useMemo, useRef, useState } from 'react';
import morphdomRuntime from 'morphdom/dist/morphdom-umd.js?raw';
import { widgetAppearanceAdapter } from '@/infrastructure/appearance/adapters/WidgetAppearanceAdapter';
import { fontPreferenceService } from '@/infrastructure/font-preference';
import {
  createWidgetAppearanceFallbackCss,
  readWidgetAppearancePayload,
  type WidgetAppearancePayload,
} from './appearancePayload';
import './GenerativeWidgetFrame.scss';

export type WidgetMessage =
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
    }
  | {
      source: 'bitfun-widget';
      type: 'bitfun-widget:clear-selection';
      widgetId?: string;
    }
  | {
      source: 'bitfun-widget';
      type: 'bitfun-widget:selection-cleared';
      widgetId?: string;
    }
  | {
      source: 'bitfun-widget';
      type: 'bitfun-widget:context-menu';
      widgetId?: string;
      clientX?: number;
      clientY?: number;
      viewportX?: number;
      viewportY?: number;
      elementSummary?: string;
      sectionSummary?: string;
      filePath?: string;
      line?: number;
    };

export type WidgetContextMenuMessage = Extract<
  WidgetMessage,
  { type: 'bitfun-widget:context-menu' }
>;

export interface GenerativeWidgetFrameProps {
  widgetId: string;
  title?: string;
  widgetCode: string;
  preferredWidth?: number;
  executeScripts?: boolean;
  className?: string;
  onWidgetEvent?: (event: WidgetMessage) => void;
  onHeightChange?: (height: number) => void;
  selectionRevision?: number;
}

export const GENERATIVE_WIDGET_SHELL_HTML = `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <style>
    * { box-sizing: border-box; }
    :root {
${createWidgetAppearanceFallbackCss()}
    }
    html, body {
      margin: 0;
      padding: 0;
      width: 100%;
      min-height: 0;
      background: transparent;
      color: var(--bf-color-content-primary);
      font-family: var(--bf-font-family-sans);
      overflow-x: hidden;
      overflow-y: hidden;
    }
    body { min-height: 0; }
    #root {
      width: 100%;
      max-width: 100%;
      min-width: 0;
      overflow-x: hidden;
    }
    #root > * {
      max-width: 100%;
    }
    img, svg, canvas, video {
      max-width: 100%;
      height: auto;
    }
    table {
      width: 100%;
      max-width: 100%;
      table-layout: fixed;
    }
    pre, code {
      white-space: pre-wrap;
      word-break: break-word;
    }
    body {
      font-size: var(--bf-font-size-sm);
      line-height: var(--bf-line-height-base);
    }
    body, button, input, textarea, select {
      font-family: var(--bf-font-family-sans);
    }
    button, input, textarea, select {
      font: inherit;
    }
    a {
      color: var(--bf-color-accent-default);
      text-decoration: none;
    }
    a:hover {
      color: var(--bf-color-accent-hover);
    }
    [data-file-path],
    [data-bitfun-open-file] {
      cursor: pointer;
    }
    .bf-root,
    .bf-stack,
    .bf-section,
    .bf-card,
    .bf-panel,
    .bf-empty,
    .bf-list,
    .bf-table-wrap {
      min-width: 0;
    }
    .bf-root {
      width: 100%;
      max-width: 100%;
      display: flex;
      flex-direction: column;
      gap: var(--bf-space-4);
      color: var(--bf-color-content-primary);
    }
    .bf-stack {
      display: flex;
      flex-direction: column;
      gap: var(--bf-space-3);
    }
    .bf-row {
      display: flex;
      align-items: center;
      gap: var(--bf-space-3);
      min-width: 0;
    }
    .bf-row-wrap {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--bf-space-3);
      min-width: 0;
    }
    .bf-toolbar {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      justify-content: space-between;
      gap: var(--bf-space-3);
      padding: var(--bf-space-3) var(--bf-space-4);
      border-radius: var(--bf-radius-lg);
      background: color-mix(in srgb, var(--bf-color-surface-panel) 82%, transparent);
      border: 1px solid var(--bf-color-border-subtle);
      box-shadow: var(--bf-shadow-xs);
    }
    .bf-section {
      display: flex;
      flex-direction: column;
      gap: var(--bf-space-3);
    }
    .bf-section-header {
      display: flex;
      flex-wrap: wrap;
      align-items: flex-start;
      justify-content: space-between;
      gap: var(--bf-space-3);
    }
    .bf-title {
      margin: 0;
      font-size: var(--bf-font-size-lg);
      font-weight: var(--bf-font-weight-semibold);
      line-height: var(--bf-line-height-tight);
      color: var(--bf-color-content-primary);
      letter-spacing: var(--bf-letter-spacing-snug);
    }
    .bf-subtitle {
      margin: 0;
      font-size: var(--bf-font-size-xs);
      color: var(--bf-color-content-muted);
      line-height: var(--bf-line-height-base);
    }
    .bf-eyebrow {
      margin: 0;
      font-size: var(--bf-font-size-meta);
      font-weight: var(--bf-font-weight-medium);
      letter-spacing: var(--bf-letter-spacing-widest);
      text-transform: uppercase;
      color: var(--bf-color-content-muted);
    }
    .bf-card,
    .bf-panel {
      position: relative;
      display: flex;
      flex-direction: column;
      gap: var(--bf-space-3);
      width: 100%;
      padding: var(--bf-space-4);
      border-radius: var(--bf-radius-lg);
      background: var(--bf-color-surface-panel);
      border: 1px solid var(--bf-color-border-subtle);
      box-shadow: var(--bf-shadow-sm);
      overflow: hidden;
    }
    .bf-panel {
      background: color-mix(in srgb, var(--bf-color-surface-panel) 74%, var(--bf-color-surface-subtle));
    }
    [data-bitfun-prompt-selected="true"],
    [data-bitfun-context-selected="true"] {
      position: relative;
      outline: 2px solid var(--bf-color-accent-default);
      outline-offset: 2px;
      box-shadow:
        0 0 0 4px color-mix(in srgb, var(--bf-color-accent-default) 18%, transparent),
        0 10px 24px color-mix(in srgb, var(--bf-color-accent-default) 14%, transparent);
      border-radius: min(var(--bf-radius-base), 12px);
      transition: outline-color 120ms ease, box-shadow 120ms ease, transform 120ms ease;
      transform: translateY(-1px);
    }
    .bf-card-accent {
      background: color-mix(in srgb, var(--bf-color-accent-default) 10%, var(--bf-color-surface-panel));
      border-color: color-mix(in srgb, var(--bf-color-accent-default) 30%, var(--bf-color-border-subtle));
    }
    .bf-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(min(180px, 100%), 1fr));
      gap: var(--bf-space-3);
      width: 100%;
      min-width: 0;
    }
    .bf-kpi {
      display: flex;
      flex-direction: column;
      gap: 6px;
      min-width: 0;
      padding: var(--bf-space-3);
      border-radius: var(--bf-radius-base);
      background: var(--bf-color-action-neutral-surface);
      border: 1px solid var(--bf-color-border-subtle);
    }
    .bf-kpi-label {
      font-size: var(--bf-font-size-meta);
      font-weight: var(--bf-font-weight-medium);
      text-transform: uppercase;
      letter-spacing: var(--bf-letter-spacing-widest);
      color: var(--bf-color-content-muted);
    }
    .bf-kpi-value {
      font-size: var(--bf-font-size-2xl);
      font-weight: var(--bf-font-weight-semibold);
      line-height: var(--bf-line-height-display);
      color: var(--bf-color-content-primary);
    }
    .bf-kpi-meta {
      font-size: var(--bf-font-size-xs);
      color: var(--bf-color-content-secondary);
    }
    .bf-badge {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 6px;
      min-height: 24px;
      padding: 0 10px;
      border-radius: 999px;
      background: var(--bf-color-action-neutral-surface);
      border: 1px solid var(--bf-color-border-subtle);
      font-size: var(--bf-font-size-xs);
      font-weight: var(--bf-font-weight-medium);
      color: var(--bf-color-content-secondary);
      white-space: nowrap;
    }
    .bf-badge-accent {
      background: color-mix(in srgb, var(--bf-color-accent-default) 14%, transparent);
      border-color: color-mix(in srgb, var(--bf-color-accent-default) 28%, var(--bf-color-border-subtle));
      color: var(--bf-color-accent-default);
    }
    .bf-badge-success {
      background: color-mix(in srgb, var(--bf-color-status-success-content) 14%, transparent);
      border-color: color-mix(in srgb, var(--bf-color-status-success-content) 28%, var(--bf-color-border-subtle));
      color: var(--bf-color-status-success-content);
    }
    .bf-badge-warning {
      background: color-mix(in srgb, var(--bf-color-status-warning-content) 14%, transparent);
      border-color: color-mix(in srgb, var(--bf-color-status-warning-content) 28%, var(--bf-color-border-subtle));
      color: var(--bf-color-status-warning-content);
    }
    .bf-badge-error {
      background: color-mix(in srgb, var(--bf-color-status-danger-content) 14%, transparent);
      border-color: color-mix(in srgb, var(--bf-color-status-danger-content) 28%, var(--bf-color-border-subtle));
      color: var(--bf-color-status-danger-content);
    }
    .bf-button {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 8px;
      min-height: 32px;
      max-width: 100%;
      padding: 0 12px;
      border: 1px solid var(--bf-color-border-default);
      border-radius: var(--bf-radius-sm);
      background: var(--bf-color-action-neutral-surface);
      color: var(--bf-color-content-secondary);
      text-decoration: none;
      white-space: nowrap;
      transition:
        transform 120ms cubic-bezier(0.23, 1, 0.32, 1),
        background-color var(--bf-motion-duration-fast) var(--bf-motion-easing-standard),
        border-color var(--bf-motion-duration-fast) var(--bf-motion-easing-standard),
        color var(--bf-motion-duration-fast) var(--bf-motion-easing-standard),
        box-shadow var(--bf-motion-duration-fast) var(--bf-motion-easing-standard);
    }
    .bf-button:hover {
      background: var(--bf-color-action-neutral-surface-hover);
      color: var(--bf-color-content-primary);
      border-color: var(--bf-color-field-border-hover);
    }
    .bf-button-primary {
      background: var(--bf-color-accent-default);
      color: var(--bf-color-content-on-dark);
      border-color: transparent;
      box-shadow: var(--bf-shadow-xs);
    }
    .bf-button-primary:hover {
      background: var(--bf-color-accent-hover);
      color: var(--bf-color-content-on-dark);
      border-color: transparent;
    }
    .bf-input,
    .bf-textarea,
    .bf-select {
      width: 100%;
      max-width: 100%;
      min-width: 0;
      padding: 0 12px;
      border-radius: var(--bf-radius-sm);
      border: 1px solid var(--bf-color-border-default);
      background: var(--bf-color-surface-subtle);
      color: var(--bf-color-content-primary);
      transition:
        background-color var(--bf-motion-duration-fast) var(--bf-motion-easing-standard),
        border-color var(--bf-motion-duration-fast) var(--bf-motion-easing-standard),
        color var(--bf-motion-duration-fast) var(--bf-motion-easing-standard),
        box-shadow var(--bf-motion-duration-fast) var(--bf-motion-easing-standard);
    }
    .bf-input,
    .bf-select {
      min-height: 34px;
    }
    .bf-textarea {
      min-height: 96px;
      padding-top: 10px;
      padding-bottom: 10px;
      resize: vertical;
    }
    .bf-input::placeholder,
    .bf-textarea::placeholder {
      color: color-mix(in srgb, var(--bf-color-content-muted) 55%, transparent);
    }
    .bf-input:focus,
    .bf-textarea:focus,
    .bf-select:focus {
      outline: none;
      border-color: var(--bf-color-accent-default);
      background: var(--bf-color-action-quiet-hover);
    }
    .bf-list {
      display: flex;
      flex-direction: column;
      gap: 8px;
      width: 100%;
    }
    .bf-list-item {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: var(--bf-space-3);
      padding: var(--bf-space-3);
      border-radius: var(--bf-radius-base);
      background: var(--bf-color-surface-subtle);
      border: 1px solid transparent;
    }
    .bf-list-item[data-file-path]:hover,
    .bf-list-item[data-bitfun-open-file]:hover,
    .bf-card[data-file-path]:hover,
    .bf-panel[data-file-path]:hover {
      border-color: color-mix(in srgb, var(--bf-color-accent-default) 35%, var(--bf-color-border-subtle));
      background: color-mix(in srgb, var(--bf-color-action-neutral-surface) 76%, var(--bf-color-accent-default));
    }
    .bf-table-wrap {
      width: 100%;
      overflow-x: auto;
      border: 1px solid var(--bf-color-border-subtle);
      border-radius: var(--bf-radius-base);
      background: var(--bf-color-surface-panel);
    }
    .bf-table {
      width: 100%;
      border-collapse: collapse;
      table-layout: fixed;
    }
    .bf-table th,
    .bf-table td {
      padding: 10px 12px;
      text-align: left;
      vertical-align: top;
      border-bottom: 1px solid var(--bf-color-border-subtle);
      color: var(--bf-color-content-secondary);
      font-size: var(--bf-font-size-sm);
      word-break: break-word;
    }
    .bf-table th {
      font-size: var(--bf-font-size-xs);
      font-weight: var(--bf-font-weight-medium);
      color: var(--bf-color-content-muted);
      text-transform: uppercase;
      letter-spacing: var(--bf-letter-spacing-wider);
    }
    .bf-empty {
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      gap: 8px;
      min-height: 140px;
      padding: var(--bf-space-5);
      border-radius: var(--bf-radius-lg);
      border: 1px dashed var(--bf-color-border-default);
      background: color-mix(in srgb, var(--bf-color-surface-subtle) 80%, transparent);
      color: var(--bf-color-content-muted);
      text-align: center;
    }
    .bf-divider {
      width: 100%;
      height: 1px;
      background: var(--bf-color-border-subtle);
      border: 0;
      margin: 0;
    }
    .bf-code {
      padding: 2px 6px;
      border-radius: 6px;
      background: var(--bf-color-action-neutral-surface);
      color: var(--bf-color-content-primary);
      font-family: var(--bf-font-family-mono);
      font-size: var(--bf-font-size-xs);
    }
    .bf-mono {
      font-family: var(--bf-font-family-mono);
    }
    @media (max-width: 560px) {
      .bf-card,
      .bf-panel,
      .bf-toolbar {
        padding: var(--bf-space-3);
      }
      .bf-grid {
        grid-template-columns: 1fr;
      }
      .bf-title {
        font-size: var(--bf-font-size-base);
      }
    }
    @media (prefers-reduced-motion: reduce) {
      .bf-button,
      .bf-input,
      .bf-textarea,
      .bf-select,
      [data-bitfun-prompt-selected="true"],
      [data-bitfun-context-selected="true"] {
        transition: none;
      }
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
      var selectedPromptTarget = null;

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

      function normalizeSpace(value) {
        return String(value || '').replace(/\\s+/g, ' ').trim();
      }

      function truncateText(value, maxLength) {
        var text = normalizeSpace(value);
        if (!text) return '';
        if (text.length <= maxLength) return text;
        return text.slice(0, Math.max(0, maxLength - 3)).trimEnd() + '...';
      }

      function clearPromptTargetSelection() {
        if (!selectedPromptTarget) return;
        selectedPromptTarget.removeAttribute('data-bitfun-prompt-selected');
        selectedPromptTarget = null;
      }

      function setPromptTargetSelection(element) {
        if (!element || !element.setAttribute) {
          clearPromptTargetSelection();
          return;
        }
        if (selectedPromptTarget === element) return;
        clearPromptTargetSelection();
        selectedPromptTarget = element;
        selectedPromptTarget.setAttribute('data-bitfun-prompt-selected', 'true');
      }

      function findPromptTarget(target) {
        var node = target && target.nodeType === 1 ? target : target && target.parentElement;
        while (node && node !== document.body) {
          if (
            node.hasAttribute('data-file-path') ||
            node.hasAttribute('data-bitfun-open-file') ||
            node.hasAttribute('data-prompt-target') ||
            node.hasAttribute('data-section-title')
          ) {
            return node;
          }
          if (/^(button|a|summary)$/i.test(node.tagName)) {
            return node;
          }
          node = node.parentElement;
        }
        return target && target.nodeType === 1 ? target : null;
      }

      function summarizeElement(element) {
        if (!element || !element.getAttribute) return '';

        var label = normalizeSpace(
          element.getAttribute('data-prompt-target') ||
          element.getAttribute('data-label') ||
          element.getAttribute('aria-label') ||
          element.getAttribute('title')
        );
        if (label) {
          return truncateText(label, 96);
        }

        var text = truncateText(element.textContent || '', 96);
        if (text) {
          return text;
        }

        var tag = (element.tagName || '').toLowerCase();
        if (!tag) return '';

        var parts = [tag];
        var id = normalizeSpace(element.getAttribute('id'));
        if (id) {
          parts.push('#' + id);
        }
        var className = normalizeSpace(element.getAttribute('class'));
        if (className) {
          parts.push('.' + className.split(/\\s+/).slice(0, 2).join('.'));
        }
        return truncateText(parts.join(' '), 96);
      }

      function summarizeSection(element) {
        var node = element;
        while (node && node !== document.body) {
          var explicit = normalizeSpace(node.getAttribute && node.getAttribute('data-section-title'));
          if (explicit) {
            return truncateText(explicit, 96);
          }

          var tag = (node.tagName || '').toLowerCase();
          var role = normalizeSpace(node.getAttribute('role'));
          if (
            tag === 'section' ||
            tag === 'article' ||
            role === 'region' ||
            role === 'group' ||
            node.classList.contains('bf-card') ||
            node.classList.contains('bf-panel')
          ) {
            var heading = node.querySelector('h1, h2, h3, h4, h5, h6, [data-section-title]');
            var headingText = truncateText(
              heading && heading.getAttribute
                ? heading.getAttribute('data-section-title') || heading.textContent
                : '',
              96
            );
            if (headingText) {
              return headingText;
            }
          }

          node = node.parentElement;
        }

        return '';
      }

      function measureHeight() {
        var root = document.getElementById('root');
        return Math.max(
          root ? root.scrollHeight : 0,
          root ? root.offsetHeight : 0,
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

      function applyAppearance(appearance) {
        if (!appearance) return;
        var root = document.documentElement;
        if (!root) return;
        if (appearance.id) root.setAttribute('data-bf-appearance', String(appearance.id));
        if (appearance.mode) root.setAttribute('data-bf-appearance-mode', String(appearance.mode));
        var vars = appearance.vars || {};
        Object.keys(vars).forEach(function (name) {
          root.style.setProperty(name, String(vars[name]));
        });
        var body = document.body;
        if (body) {
          body.style.background = vars['--bf-color-surface-canvas'] || 'transparent';
          body.style.color =
            vars['--bf-color-content-primary'] ||
            getComputedStyle(root).getPropertyValue('--bf-color-content-primary') ||
            body.style.color;
          body.style.fontFamily =
            vars['--bf-font-family-sans'] ||
            getComputedStyle(root).getPropertyValue('--bf-font-family-sans') ||
            body.style.fontFamily;
        }
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

      document.addEventListener('pointerdown', function (event) {
        var target = event.target;
        if (!selectedPromptTarget) return;
        if (target === selectedPromptTarget) return;
        if (selectedPromptTarget.contains && selectedPromptTarget.contains(target)) return;
        clearPromptTargetSelection();
        sendMessage({
          source: 'bitfun-widget',
          type: 'bitfun-widget:selection-cleared',
          widgetId: currentWidgetId
        });
      }, true);

      document.addEventListener('contextmenu', function (event) {
        var target = event.target;
        var promptTarget = findPromptTarget(target);
        var elementSummary = summarizeElement(promptTarget);
        if (!elementSummary) {
          clearPromptTargetSelection();
          return;
        }
        setPromptTargetSelection(promptTarget);

        var filePath = normalizeSpace(
          promptTarget && promptTarget.getAttribute
            ? promptTarget.getAttribute('data-file-path') || promptTarget.getAttribute('data-bitfun-open-file')
            : ''
        );
        var lineValue = Number(
          promptTarget && promptTarget.getAttribute ? promptTarget.getAttribute('data-line') || '' : ''
        );

        event.preventDefault();
        event.stopPropagation();
        sendMessage({
          source: 'bitfun-widget',
          type: 'bitfun-widget:context-menu',
          widgetId: currentWidgetId,
          clientX: Number(event.clientX) || 0,
          clientY: Number(event.clientY) || 0,
          elementSummary: elementSummary,
          sectionSummary: summarizeSection(promptTarget),
          filePath: filePath || undefined,
          line: Number.isFinite(lineValue) && lineValue > 0 ? lineValue : undefined
        });
      }, true);

      window.addEventListener('message', function (event) {
        var data = event.data;
        if (!data) return;
        if (data.type === 'bitfun-widget:clear-selection') {
          if (!data.widgetId || data.widgetId === currentWidgetId) {
            clearPromptTargetSelection();
          }
          return;
        }
        if (data.type !== 'bitfun-widget:update') return;
        currentWidgetId = data.widgetId || currentWidgetId || '';
        applyAppearance(data.appearance);
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
  selectionRevision = 0,
}) => {
  const iframeRef = useRef<HTMLIFrameElement | null>(null);
  const [isLoaded, setIsLoaded] = useState(false);
  const [frameHeight, setFrameHeight] = useState(160);
  const lastExecutedHtmlRef = useRef('');
  const [appearancePayload, setAppearancePayload] = useState<WidgetAppearancePayload | null>(() =>
    readWidgetAppearancePayload(),
  );

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

      if (data.type === 'bitfun-widget:context-menu') {
        const iframeRect = iframeRef.current?.getBoundingClientRect();
        onWidgetEvent?.({
          ...data,
          viewportX: iframeRect ? iframeRect.left + (Number(data.clientX) || 0) : data.viewportX,
          viewportY: iframeRect ? iframeRect.top + (Number(data.clientY) || 0) : data.viewportY,
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
    const iframe = iframeRef.current;
    if (!iframe) return undefined;

    let cancelled = false;
    setIsLoaded(false);

    const writeShellHtml = () => {
      const doc = iframe.contentDocument;
      if (!doc) return false;
      doc.open();
      doc.write(GENERATIVE_WIDGET_SHELL_HTML);
      doc.close();
      if (!cancelled) {
        setIsLoaded(true);
      }
      return true;
    };

    if (writeShellHtml()) {
      return undefined;
    }

    const handleLoad = () => {
      writeShellHtml();
    };

    iframe.addEventListener('load', handleLoad);
    if (iframe.src !== 'about:blank') {
      iframe.src = 'about:blank';
    }

    return () => {
      cancelled = true;
      iframe.removeEventListener('load', handleLoad);
    };
  }, []);

  useEffect(() => {
    const updateAppearance = () => {
      setAppearancePayload(readWidgetAppearancePayload());
    };

    updateAppearance();
    const unsubscribeAppearance = widgetAppearanceAdapter.subscribe(updateAppearance);
    const unsubscribeFont = fontPreferenceService.on('font:after-change', updateAppearance);
    return () => {
      unsubscribeAppearance?.();
      unsubscribeFont();
    };
  }, []);

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
        appearance: appearancePayload,
        runScripts: shouldRunScripts,
      },
      '*',
    );

    if (shouldRunScripts) {
      lastExecutedHtmlRef.current = normalizedCode;
    }
  }, [appearancePayload, executeScripts, isLoaded, normalizedCode, title, widgetId]);

  useEffect(() => {
    if (!isLoaded || !iframeRef.current?.contentWindow) {
      return;
    }

    iframeRef.current.contentWindow.postMessage(
      {
        type: 'bitfun-widget:clear-selection',
        widgetId,
      },
      '*',
    );
  }, [isLoaded, selectionRevision, widgetId]);

  return (
    <div
      className={`bitfun-generative-widget-frame ${className}`.trim()}
      data-bf-component="generative-widget"
      data-bf-part="frame"
      style={{ height: `${frameHeight}px` }}
    >
      <iframe
        ref={iframeRef}
        title={title || 'Generative widget'}
        className={`bitfun-generative-widget-frame__iframe${isLoaded ? ' bitfun-generative-widget-frame__iframe--loaded' : ''}`}
        data-bf-component="generative-widget"
        data-bf-part="iframe"
        style={{ width: '100%', minWidth: '100%' }}
        sandbox="allow-scripts allow-same-origin allow-forms allow-modals allow-popups"
        src="about:blank"
      />
    </div>
  );
};

export default GenerativeWidgetFrame;
