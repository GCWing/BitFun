/**
 * SelfControlService — lets BitFun agent operate its own GUI.
 *
 * Inspired by page-agent's design but implemented with native DOM APIs
 * and BitFun-specific semantic shortcuts (scene switching, settings tabs, model selection).
 */

import { useSceneStore } from '@/app/stores/sceneStore';
import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import { configManager } from '@/infrastructure/config';
import { getModelDisplayName } from '@/infrastructure/config/services/modelConfigs';
import { matchProviderCatalogItemByBaseUrl } from '@/infrastructure/config/services/providerCatalog';
import { createLogger } from '@/shared/utils/logger';

const logger = createLogger('SelfControlService');

export interface SimplifiedElement {
  tag: string;
  id?: string;
  class?: string;
  text: string;
  ariaLabel?: string;
  role?: string;
  placeholder?: string;
  title?: string;
  dataTestid?: string;
  dataSelfControlTarget?: string;
  interactive: boolean;
  rect: { x: number; y: number; width: number; height: number };
}

export interface PageState {
  title: string;
  activeScene: string;
  elements: SimplifiedElement[];
}

export type SelfControlAction =
  | { type: 'click'; selector: string }
  | { type: 'click_by_text'; text: string; tag?: string }
  | { type: 'input'; selector: string; value: string }
  | { type: 'scroll'; selector?: string; direction: 'up' | 'down' | 'top' | 'bottom' }
  | { type: 'open_scene'; sceneId: string }
  | { type: 'open_settings_tab'; tabId: string }
  | { type: 'set_config'; key: string; value: unknown }
  | { type: 'get_config'; key: string }
  | { type: 'list_models' }
  | { type: 'set_default_model'; modelQuery: string; slot?: 'primary' | 'fast' }
  | { type: 'select_option'; selector: string; optionText: string }
  | { type: 'get_page_state' };

interface ModelInfo {
  id: string;
  name: string;
  displayName: string;
  provider: string;
  modelName: string;
  enabled: boolean;
}

export class SelfControlService {
  private highlightOverlay: HTMLDivElement | null = null;

  getPageState(): PageState {
    const activeScene = useSceneStore.getState().activeTabId;
    const elements = this.collectInteractiveElements();
    return {
      title: document.title,
      activeScene,
      elements,
    };
  }

  async executeAction(rawAction: SelfControlAction): Promise<string> {
    const action = this.normalizeAction(rawAction);
    logger.info('Executing self-control action', { type: action.type });

    switch (action.type) {
      case 'get_page_state':
        return JSON.stringify(this.getPageState(), null, 2);

      case 'open_scene':
        useSceneStore.getState().openScene(action.sceneId as any);
        return `Opened scene: ${action.sceneId}`;

      case 'open_settings_tab':
        useSceneStore.getState().openScene('settings');
        useSettingsStore.getState().setActiveTab(action.tabId as any);
        return `Opened settings tab: ${action.tabId}`;

      case 'set_config':
        await configManager.setConfig(action.key, action.value);
        return `Set config ${action.key} = ${JSON.stringify(action.value)}`;

      case 'get_config': {
        const value = await configManager.getConfig(action.key);
        return value === undefined ? 'null' : JSON.stringify(value);
      }

      case 'list_models':
        return this.listModels();

      case 'set_default_model':
        return this.setDefaultModel(action.modelQuery, action.slot || 'primary');

      case 'select_option':
        return this.selectOption(action.selector, action.optionText);

      case 'click':
        return this.clickElement(action.selector);

      case 'click_by_text':
        return this.clickElementByText(action.text, action.tag);

      case 'input':
        return this.inputText(action.selector, action.value);

      case 'scroll':
        return this.scroll(action.selector, action.direction);

      default:
        return `Unknown action type: ${(action as any).type}`;
    }
  }

  /**
   * Normalize snake_case fields coming from Rust backend into camelCase
   * expected by the TypeScript union type.
   */
  private normalizeAction(raw: SelfControlAction): SelfControlAction {
    const r = raw as any;
    const base = { ...r };

    if (r.scene_id !== undefined && base.sceneId === undefined) base.sceneId = r.scene_id;
    if (r.tab_id !== undefined && base.tabId === undefined) base.tabId = r.tab_id;
    if (r.model_query !== undefined && base.modelQuery === undefined) base.modelQuery = r.model_query;
    if (r.option_text !== undefined && base.optionText === undefined) base.optionText = r.option_text;
    if (r.config_value !== undefined && base.value === undefined) base.value = r.config_value;

    return base as SelfControlAction;
  }

  /**
   * Fetch and normalize all enabled models from ai.models config.
   */
  private async fetchEnabledModels(): Promise<ModelInfo[]> {
    const models = (await configManager.getConfig<any[]>('ai.models')) || [];
    logger.debug('Fetched ai.models', { count: models.length });

    return models
      .filter((m) => m && m.enabled !== false)
      .map((m) => {
        const providerItem = matchProviderCatalogItemByBaseUrl(m.base_url || '');
        const inferredProvider = providerItem?.id || m.provider || m.name || 'Unknown';
        const displayName = getModelDisplayName({
          name: m.name || inferredProvider,
          model_name: m.model_name || '',
          base_url: m.base_url || '',
        });

        return {
          id: String(m.id || ''),
          name: String(m.name || ''),
          displayName,
          provider: inferredProvider,
          modelName: String(m.model_name || ''),
          enabled: m.enabled !== false,
        };
      })
      .filter((m) => m.enabled && m.id);
  }

  private async listModels(): Promise<string> {
    const enabledModels = await this.fetchEnabledModels();
    if (enabledModels.length === 0) {
      return 'No enabled models found.';
    }

    const lines = enabledModels.map((m) => {
      const parts = [`ID: ${m.id}`, `Display: ${m.displayName}`];
      if (m.modelName) parts.push(`Model: ${m.modelName}`);
      if (m.provider) parts.push(`Provider: ${m.provider}`);
      return `- ${parts.join(' | ')}`;
    });

    return `Available enabled models (${enabledModels.length}):\n${lines.join('\n')}`;
  }

  private async setDefaultModel(modelQuery: string, slot: 'primary' | 'fast'): Promise<string> {
    const enabledModels = await this.fetchEnabledModels();

    if (enabledModels.length === 0) {
      return 'No enabled models found. Please configure models first.';
    }

    const query = modelQuery.toLowerCase().trim();

    // Scoring: exact match > startsWith > includes
    let bestMatch: ModelInfo | null = null;
    let bestScore = -1;

    for (const m of enabledModels) {
      const searchTargets = [
        m.displayName.toLowerCase(),
        m.modelName.toLowerCase(),
        m.name.toLowerCase(),
        m.provider.toLowerCase(),
        m.id.toLowerCase(),
      ];

      for (const target of searchTargets) {
        if (target === query) {
          return this.applyDefaultModel(slot, m);
        }
        if (target.startsWith(query) && bestScore < 2) {
          bestScore = 2;
          bestMatch = m;
        } else if (target.includes(query) && bestScore < 1) {
          bestScore = 1;
          bestMatch = m;
        }
      }
    }

    if (bestMatch) {
      return this.applyDefaultModel(slot, bestMatch);
    }

    const available = enabledModels.map((m) => `"${m.displayName}" (ID: ${m.id})`).join(', ');
    return (
      `Model "${modelQuery}" not found. Available enabled models: ${available}\n\n` +
      `Tip: use "list_models" to see exact names, or open the model settings tab to select manually.`
    );
  }

  private async applyDefaultModel(slot: 'primary' | 'fast', model: ModelInfo): Promise<string> {
    const currentConfig = (await configManager.getConfig<any>('ai.default_models')) || {};
    await configManager.setConfig('ai.default_models', {
      ...currentConfig,
      [slot]: model.id,
    });
    return `Set ${slot === 'primary' ? 'primary' : 'fast'} model to "${model.displayName}" (ID: ${model.id})`;
  }

  private collectInteractiveElements(): SimplifiedElement[] {
    const candidates = document.querySelectorAll(
      [
        'button',
        'a',
        'input',
        'textarea',
        'select',
        'label',
        '[role="button"]',
        '[role="link"]',
        '[role="tab"]',
        '[role="menuitem"]',
        '[role="combobox"]',
        '[role="option"]',
        '[role="radio"]',
        '[role="checkbox"]',
        '[role="switch"]',
        '[tabindex="0"]',
        '[contenteditable="true"]',
        '[data-testid]',
        '[data-self-control-target]',
        '.select__trigger',
        '.select__option',
        '.switch',
      ].join(',')
    );

    const elements: SimplifiedElement[] = [];
    const seen = new Set<Element>();
    const viewportW = window.innerWidth;
    const viewportH = window.innerHeight;

    candidates.forEach((el) => {
      if (seen.has(el)) return;
      seen.add(el);

      const htmlEl = el as HTMLElement;
      const rect = htmlEl.getBoundingClientRect();

      if (rect.width < 2 || rect.height < 2) return;
      if (rect.right < 0 || rect.bottom < 0 || rect.left > viewportW || rect.top > viewportH) return;

      const style = window.getComputedStyle(htmlEl);
      if (style.display === 'none' || style.visibility === 'hidden' || parseFloat(style.opacity) < 0.01) {
        return;
      }

      const text = this.extractText(el).slice(0, 120);
      const ariaLabel = el.getAttribute('aria-label') || undefined;
      const placeholder = (el as HTMLInputElement).placeholder || undefined;
      const title = el.getAttribute('title') || undefined;
      const dataTestid = el.getAttribute('data-testid') || undefined;
      const dataSelfControlTarget = el.getAttribute('data-self-control-target') || undefined;

      const hasIdentity = !!(text || el.id || dataTestid || dataSelfControlTarget || ariaLabel || placeholder || title);
      const isInteractive = this.isInteractive(el);
      if (!hasIdentity && !isInteractive) return;

      elements.push({
        tag: el.tagName.toLowerCase(),
        id: el.id || undefined,
        class: el.className || undefined,
        text,
        ariaLabel,
        role: el.getAttribute('role') || undefined,
        placeholder,
        title,
        dataTestid,
        dataSelfControlTarget,
        interactive: isInteractive,
        rect: { x: Math.round(rect.x), y: Math.round(rect.y), width: Math.round(rect.width), height: Math.round(rect.height) },
      });
    });

    return elements;
  }

  private extractText(el: Element): string {
    const walk = (node: Node): string => {
      if (node.nodeType === Node.TEXT_NODE) {
        return node.textContent || '';
      }
      if (node.nodeType !== Node.ELEMENT_NODE) {
        return '';
      }
      const elNode = node as HTMLElement;
      const style = window.getComputedStyle(elNode);
      if (style.display === 'none' || style.visibility === 'hidden') {
        return '';
      }
      return Array.from(elNode.childNodes)
        .map(walk)
        .join('')
        .replace(/\s+/g, ' ')
        .trim();
    };

    const directText = el.getAttribute('aria-label') || '';
    const childText = walk(el);
    return (directText || childText || (el as HTMLElement).title || '').trim();
  }

  private isInteractive(el: Element): boolean {
    const tag = el.tagName.toLowerCase();
    const role = el.getAttribute('role');
    if (['button', 'a', 'input', 'textarea', 'select', 'label'].includes(tag)) return true;
    if (['button', 'link', 'tab', 'menuitem', 'combobox', 'option', 'radio', 'checkbox', 'switch'].includes(role || '')) return true;
    if ((el as HTMLElement).onclick != null) return true;
    if (el.getAttribute('tabindex') === '0') return true;
    if (el.classList.contains('select__trigger') || el.classList.contains('select__option')) return true;
    if (el.getAttribute('contenteditable') === 'true') return true;
    return false;
  }

  private async selectOption(selector: string, optionText: string): Promise<string> {
    const trigger = document.querySelector(selector) as HTMLElement | null;
    if (!trigger) return `Select trigger not found: ${selector}`;

    this.flashHighlight(trigger);
    trigger.click();

    await new Promise((r) => setTimeout(r, 150));

    const options = Array.from(document.querySelectorAll('.select__option'));
    const target = options.find((el) => {
      const text = this.extractText(el).toLowerCase();
      return text.includes(optionText.toLowerCase());
    }) as HTMLElement | undefined;

    if (!target) {
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      return `Option "${optionText}" not found in dropdown`;
    }

    this.flashHighlight(target);
    target.click();
    return `Selected option "${optionText}" in ${selector}`;
  }

  private clickElement(selector: string): string {
    const el = document.querySelector(selector) as HTMLElement | null;
    if (!el) return `Element not found: ${selector}`;
    this.flashHighlight(el);
    el.click();
    return `Clicked element: ${selector}`;
  }

  private clickElementByText(text: string, tag?: string): string {
    const selector = tag || '*';
    const elements = Array.from(document.querySelectorAll(selector));
    const query = text.toLowerCase().trim();

    const target = elements.find((el) => {
      const candidates = [
        this.extractText(el).toLowerCase(),
        (el.getAttribute('aria-label') || '').toLowerCase(),
        (el.getAttribute('title') || '').toLowerCase(),
        ((el as HTMLInputElement).placeholder || '').toLowerCase(),
      ];
      return candidates.some((c) => c.includes(query));
    }) as HTMLElement | undefined;

    if (!target) return `Element with text "${text}" not found`;
    this.flashHighlight(target);
    target.click();
    return `Clicked element with text: ${text}`;
  }

  private inputText(selector: string, value: string): string {
    const el = document.querySelector(selector) as HTMLInputElement | HTMLTextAreaElement | null;
    if (!el) return `Input element not found: ${selector}`;

    this.flashHighlight(el);

    if (el.tagName.toLowerCase() === 'input' || el.tagName.toLowerCase() === 'textarea') {
      el.focus();
      el.value = value;
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new Event('change', { bubbles: true }));
      return `Set input ${selector} to "${value}"`;
    }

    if (el.isContentEditable) {
      el.textContent = value;
      el.dispatchEvent(new Event('input', { bubbles: true }));
      return `Set contenteditable ${selector} to "${value}"`;
    }

    return `Element ${selector} is not an input`;
  }

  private scroll(selector: string | undefined, direction: 'up' | 'down' | 'top' | 'bottom'): string {
    const el = selector
      ? (document.querySelector(selector) as HTMLElement | null)
      : (document.scrollingElement as HTMLElement | null);

    if (!el) return `Scroll target not found: ${selector || 'document'}`;

    const scrollAmount = 500;
    switch (direction) {
      case 'up':
        el.scrollBy({ top: -scrollAmount, behavior: 'smooth' });
        return `Scrolled up ${selector || 'document'}`;
      case 'down':
        el.scrollBy({ top: scrollAmount, behavior: 'smooth' });
        return `Scrolled down ${selector || 'document'}`;
      case 'top':
        el.scrollTo({ top: 0, behavior: 'smooth' });
        return `Scrolled to top ${selector || 'document'}`;
      case 'bottom':
        el.scrollTo({ top: el.scrollHeight, behavior: 'smooth' });
        return `Scrolled to bottom ${selector || 'document'}`;
    }
  }

  private flashHighlight(el: HTMLElement): void {
    const rect = el.getBoundingClientRect();
    if (!this.highlightOverlay) {
      this.highlightOverlay = document.createElement('div');
      this.highlightOverlay.style.position = 'fixed';
      this.highlightOverlay.style.pointerEvents = 'none';
      this.highlightOverlay.style.zIndex = '999999';
      this.highlightOverlay.style.border = '2px solid #f59e0b';
      this.highlightOverlay.style.backgroundColor = 'rgba(245, 158, 11, 0.15)';
      this.highlightOverlay.style.borderRadius = '4px';
      this.highlightOverlay.style.transition = 'opacity 0.2s ease';
      document.body.appendChild(this.highlightOverlay);
    }

    this.highlightOverlay.style.left = `${rect.left + window.scrollX}px`;
    this.highlightOverlay.style.top = `${rect.top + window.scrollY}px`;
    this.highlightOverlay.style.width = `${rect.width}px`;
    this.highlightOverlay.style.height = `${rect.height}px`;
    this.highlightOverlay.style.opacity = '1';

    setTimeout(() => {
      if (this.highlightOverlay) {
        this.highlightOverlay.style.opacity = '0';
      }
    }, 800);
  }
}

export const selfControlService = new SelfControlService();
