/**
 * SelfControlService — lets BitFun agent operate its own GUI.
 *
 * Architecture: four responsibility regions inside one class.
 *
 * Region 1 – DOM Primitives   : click / input / scroll / pressKey / readText / wait
 * Region 2 – App State        : openScene / openSettingsTab / getPageState
 * Region 3 – Config & Models  : setConfig / getConfig / listModels / setDefaultModel / deleteModel
 * Region 4 – Task Orchestration: executeTask — composes Regions 1-3
 *
 * The backend forwards the LLM's camelCase payload directly without any
 * field remapping, so all action types here use camelCase field names that
 * match the Rust input_schema exactly.
 */

import { useSceneStore } from '@/app/stores/sceneStore';
import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import { configManager } from '@/infrastructure/config';
import { getModelDisplayName } from '@/infrastructure/config/services/modelConfigs';
import { matchProviderCatalogItemByBaseUrl } from '@/infrastructure/config/services/providerCatalog';
import { createLogger } from '@/shared/utils/logger';

const logger = createLogger('SelfControlService');

// Option selectors tried in order when looking for dropdown items
const DROPDOWN_OPTION_SELECTORS = [
  '.select__option',
  '[role="option"]',
  '.dropdown__item',
  '.menu__item',
  'li',
] as const;

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
  activeSettingsTab?: string;
  elements: SimplifiedElement[];
  targets: Record<string, string>;
  semanticHints: string[];
}

/** Internal normalized action shape used by the dispatcher. */
export type SelfControlAction =
  | { type: 'execute_task'; task: string; params?: Record<string, string> }
  | { type: 'click'; selector: string }
  | { type: 'click_by_text'; text: string; tag?: string }
  | { type: 'input'; selector: string; value: string }
  | { type: 'scroll'; selector?: string; direction: 'up' | 'down' | 'top' | 'bottom' }
  | { type: 'open_scene'; sceneId: string }
  | { type: 'open_settings_tab'; tabId: string }
  | { type: 'set_config'; key: string; configValue: unknown }
  | { type: 'get_config'; key: string }
  | { type: 'list_models'; includeDisabled?: boolean }
  | { type: 'set_default_model'; modelQuery: string; slot?: 'primary' | 'fast' }
  | { type: 'select_option'; selector: string; optionText: string }
  | { type: 'get_page_state' }
  | { type: 'wait'; durationMs: number }
  | { type: 'press_key'; key: string }
  | { type: 'read_text'; selector: string }
  | { type: 'delete_model'; modelQuery: string };

/**
 * Raw action payload received from Rust.
 * The tool schema uses `action` as the discriminator; we normalize it to `type`
 * before dispatch so the frontend can accept both the new direct passthrough
 * payload and older internal callers that already send `type`.
 */
export type SelfControlIncomingAction = Partial<SelfControlAction> & {
  action?: SelfControlAction['type'];
  type?: SelfControlAction['type'];
  sceneId?: string;
  scene_id?: string;
  tabId?: string;
  tab_id?: string;
  configValue?: unknown;
  config_value?: unknown;
  modelQuery?: string;
  model_query?: string;
  optionText?: string;
  option_text?: string;
  durationMs?: number;
  duration_ms?: number;
  includeDisabled?: boolean;
  include_disabled?: boolean;
};

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

  // ── Region 2: App State ──────────────────────────────────────────────────

  async getPageState(): Promise<PageState> {
    const activeScene = useSceneStore.getState().activeTabId;
    const activeSettingsTab =
      activeScene === 'settings' ? useSettingsStore.getState().activeTab : undefined;
    const elements = this.collectInteractiveElements();
    const targets = this.buildTargetIndex(elements);
    const semanticHints = this.buildSemanticHints(activeScene, activeSettingsTab, elements, targets);

    if (activeScene === 'settings' && activeSettingsTab === 'models') {
      await this.maybeAppendModelSummary(semanticHints);
    }

    return {
      title: document.title,
      activeScene,
      activeSettingsTab,
      elements: elements.slice(0, 60),
      targets,
      semanticHints,
    };
  }

  // ── Dispatcher ───────────────────────────────────────────────────────────

  async executeAction(rawAction: SelfControlIncomingAction | SelfControlAction): Promise<string> {
    const action = this.normalizeAction(rawAction);
    logger.info('Executing self-control action', { type: action.type });

    switch (action.type) {
      case 'execute_task':
        return this.executeTask(action.task, action.params);

      case 'get_page_state':
        return JSON.stringify(await this.getPageState(), null, 2);

      // Region 2: App State
      case 'open_scene':
        return this.openScene(action.sceneId);
      case 'open_settings_tab':
        return this.openSettingsTab(action.tabId);

      // Region 3: Config & Models
      case 'set_config':
        return this.setConfig(action.key, action.configValue);
      case 'get_config':
        return this.getConfig(action.key);
      case 'list_models':
        return this.listModels(action.includeDisabled);
      case 'set_default_model':
        return this.setDefaultModel(action.modelQuery, action.slot ?? 'primary');
      case 'delete_model':
        return this.deleteModel(action.modelQuery);

      // Region 1: DOM Primitives
      case 'click':
        return this.clickElement(action.selector);
      case 'click_by_text':
        return this.clickElementByText(action.text, action.tag);
      case 'input':
        return this.inputText(action.selector, action.value);
      case 'scroll':
        return this.scroll(action.selector, action.direction);
      case 'select_option':
        return this.selectOption(action.selector, action.optionText);
      case 'wait':
        return this.wait(action.durationMs);
      case 'press_key':
        return this.pressKey(action.key);
      case 'read_text':
        return this.readText(action.selector);

      default:
        return `Unknown action type: ${(action as { type: string }).type}`;
    }
  }

  // ── Region 4: Task Orchestration ─────────────────────────────────────────

  private async executeTask(task: string, params?: Record<string, string>): Promise<string> {
    logger.info('Executing task', { task, params });

    switch (task) {
      case 'set_primary_model':
      case 'set_fast_model': {
        const slot = task === 'set_primary_model' ? 'primary' : 'fast';
        const modelQuery = params?.modelQuery ?? params?.model ?? '';
        if (!modelQuery) return `Missing modelQuery for ${task}`;

        const configResult = await this.setDefaultModel(modelQuery, slot);
        if (!configResult.toLowerCase().includes('not found')) {
          return configResult;
        }
        return this.setDefaultModelViaUI(modelQuery, slot);
      }

      case 'open_model_settings': {
        return this.openSettingsTab('models');
      }

      case 'return_to_session': {
        return this.openScene('session');
      }

      case 'delete_model': {
        const modelQuery = params?.modelQuery ?? params?.model ?? '';
        if (!modelQuery) return 'Missing modelQuery for delete_model';
        return this.deleteModel(modelQuery);
      }

      default:
        return `Unknown task: ${task}. Available tasks: set_primary_model, set_fast_model, open_model_settings, return_to_session, delete_model.`;
    }
  }

  // ── Region 2: App State ──────────────────────────────────────────────────

  private normalizeAction(rawAction: SelfControlIncomingAction | SelfControlAction): SelfControlAction {
    const raw = rawAction as SelfControlIncomingAction;
    const type = raw.type ?? raw.action;
    if (!type) {
      throw new Error('Missing self-control action type');
    }

    return {
      ...raw,
      type,
      sceneId: raw.sceneId ?? raw.scene_id,
      tabId: raw.tabId ?? raw.tab_id,
      configValue: raw.configValue ?? raw.config_value,
      modelQuery: raw.modelQuery ?? raw.model_query,
      optionText: raw.optionText ?? raw.option_text,
      durationMs: raw.durationMs ?? raw.duration_ms,
      includeDisabled: raw.includeDisabled ?? raw.include_disabled,
    } as SelfControlAction;
  }

  private openScene(sceneId: string): string {
    useSceneStore.getState().openScene(sceneId as any);
    return `Opened scene: ${sceneId}`;
  }

  private openSettingsTab(tabId: string): string {
    useSceneStore.getState().openScene('settings' as any);
    useSettingsStore.getState().setActiveTab(tabId as any);
    return `Opened settings tab: ${tabId}`;
  }

  // ── Region 3: Config & Model Operations ──────────────────────────────────

  private async setConfig(key: string, value: unknown): Promise<string> {
    await configManager.setConfig(key, value);
    return `Set config ${key} = ${JSON.stringify(value)}`;
  }

  private async getConfig(key: string): Promise<string> {
    const value = await configManager.getConfig(key);
    return value === undefined ? 'null' : JSON.stringify(value);
  }

  private async fetchModels(includeDisabled = false): Promise<ModelInfo[]> {
    const models = (await configManager.getConfig<any[]>('ai.models')) ?? [];
    logger.debug('Fetched ai.models', { count: models.length, includeDisabled });

    const mapped = models
      .filter((m) => m && (includeDisabled || m.enabled !== false))
      .map((m) => {
        const providerItem = matchProviderCatalogItemByBaseUrl(m.base_url ?? '');
        const inferredProvider = providerItem?.id ?? m.provider ?? m.name ?? 'Unknown';
        const displayName = getModelDisplayName({
          name: m.name ?? inferredProvider,
          model_name: m.model_name ?? '',
          base_url: m.base_url ?? '',
        });

        return {
          id: String(m.id ?? ''),
          name: String(m.name ?? ''),
          displayName,
          provider: inferredProvider,
          modelName: String(m.model_name ?? ''),
          enabled: m.enabled !== false,
        };
      });

    return includeDisabled ? mapped.filter((m) => m.id) : mapped.filter((m) => m.enabled && m.id);
  }

  private async listModels(includeDisabled = false): Promise<string> {
    const models = await this.fetchModels(includeDisabled);
    if (models.length === 0) {
      return includeDisabled ? 'No models configured.' : 'No enabled models found.';
    }

    const lines = models.map((m) => {
      const status = m.enabled ? '[enabled]' : '[disabled]';
      const parts = [`${status} ID: ${m.id}`, `Display: ${m.displayName}`];
      if (m.modelName) parts.push(`Model: ${m.modelName}`);
      if (m.provider) parts.push(`Provider: ${m.provider}`);
      return `- ${parts.join(' | ')}`;
    });

    const label = includeDisabled ? 'All configured models' : 'Enabled models';
    return `${label} (${models.length}):\n${lines.join('\n')}`;
  }

  private async setDefaultModel(modelQuery: string, slot: 'primary' | 'fast'): Promise<string> {
    const enabledModels = await this.fetchModels();

    if (enabledModels.length === 0) {
      return 'No enabled models found. Please configure models first.';
    }

    const query = modelQuery.toLowerCase().trim();
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
      `Tip: use "list_models" to see exact names, or use task "set_primary_model" to let me try the UI dropdown automatically.`
    );
  }

  private async setDefaultModelViaUI(modelQuery: string, slot: 'primary' | 'fast'): Promise<string> {
    this.openSettingsTab('models');
    await this.wait(300);

    const targetAttr = slot === 'primary' ? 'primary-model-select' : 'fast-model-select';
    const selector = `[data-self-control-target="${targetAttr}"] .select__trigger`;
    const trigger = document.querySelector(selector) as HTMLElement | null;

    if (!trigger) {
      return `Could not find ${slot} model selector in the UI. The model setting page may not be fully loaded.`;
    }

    this.flashHighlight(trigger);
    trigger.click();
    await this.wait(200);

    const options = this.findDropdownOptions();
    const query = modelQuery.toLowerCase();
    const target = options.find((el) => this.extractText(el).toLowerCase().includes(query)) as
      | HTMLElement
      | undefined;

    if (!target) {
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      const optionTexts = options.map((el) => `"${this.extractText(el)}"`).join(', ');
      return `Model "${modelQuery}" not found in the ${slot} dropdown. Available options: ${optionTexts}`;
    }

    this.flashHighlight(target);
    target.click();
    return `Set ${slot} model to "${modelQuery}" via the UI dropdown`;
  }

  private async applyDefaultModel(slot: 'primary' | 'fast', model: ModelInfo): Promise<string> {
    const currentConfig = (await configManager.getConfig<any>('ai.default_models')) ?? {};
    await configManager.setConfig('ai.default_models', {
      ...currentConfig,
      [slot]: model.id,
    });
    return `Set ${slot === 'primary' ? 'primary' : 'fast'} model to "${model.displayName}" (ID: ${model.id})`;
  }

  private async deleteModel(modelQuery: string): Promise<string> {
    const allModels = (await configManager.getConfig<any[]>('ai.models')) ?? [];
    if (allModels.length === 0) {
      return 'No models configured.';
    }

    const query = modelQuery.toLowerCase().trim();
    const matches = allModels.filter((m) => {
      const haystack = [
        String(m.id ?? '').toLowerCase(),
        String(m.name ?? '').toLowerCase(),
        String(m.model_name ?? '').toLowerCase(),
        String(m.provider ?? '').toLowerCase(),
        String(m.base_url ?? '').toLowerCase(),
      ].join(' ');
      return haystack.includes(query);
    });

    if (matches.length === 0) {
      const available = allModels
        .map((m) => `"${m.name ?? 'Unknown'}/${m.model_name ?? 'unknown'}" (ID: ${m.id})`)
        .join(', ');
      return `Model matching "${modelQuery}" not found. Available models: ${available}`;
    }

    const deletedIds = new Set(matches.map((m) => String(m.id ?? '')));
    const updatedModels = allModels.filter((m) => !deletedIds.has(String(m.id ?? '')));
    await configManager.setConfig('ai.models', updatedModels);

    const currentDefaults =
      (await configManager.getConfig<Record<string, string>>('ai.default_models')) ?? {};
    const remainingEnabledModels = updatedModels.filter((m) => m && m.enabled !== false && m.id);
    const nextDefaults: Record<string, string> = { ...currentDefaults };
    const notes: string[] = [];

    if (currentDefaults.primary && deletedIds.has(currentDefaults.primary)) {
      const replacementPrimary = String(remainingEnabledModels[0]?.id ?? '');
      if (replacementPrimary) {
        nextDefaults.primary = replacementPrimary;
        notes.push(`primary fallback -> ${replacementPrimary}`);
      } else {
        delete nextDefaults.primary;
        notes.push('primary cleared');
      }
    }

    if (currentDefaults.fast && deletedIds.has(currentDefaults.fast)) {
      const fallbackFast = nextDefaults.primary;
      if (fallbackFast) {
        nextDefaults.fast = fallbackFast;
        notes.push(`fast fallback -> ${fallbackFast}`);
      } else {
        delete nextDefaults.fast;
        notes.push('fast cleared');
      }
    }

    if (notes.length > 0) {
      await configManager.setConfig('ai.default_models', nextDefaults);
    }

    const deletedNames = matches
      .map((m) => `"${m.name ?? 'Unknown'}/${m.model_name ?? 'unknown'}" (ID: ${m.id})`)
      .join(', ');
    const suffix = notes.length > 0 ? ` Default model updates: ${notes.join('; ')}.` : '';
    return `Deleted ${matches.length} model(s): ${deletedNames}.${suffix}`;
  }

  // ── Region 1: DOM Primitives ─────────────────────────────────────────────

  private clickElement(selector: string): string {
    const el = document.querySelector(selector) as HTMLElement | null;
    if (!el) return `Element not found: ${selector}`;
    this.flashHighlight(el);
    this.dispatchClick(el);
    return `Clicked element: ${selector}`;
  }

  private clickElementByText(text: string, tag?: string): string {
    const selector = tag ?? '*';
    const elements = Array.from(document.querySelectorAll(selector));
    const query = text.toLowerCase().trim();

    const target = elements.find((el) => {
      const candidates = [
        this.extractText(el).toLowerCase(),
        (el.getAttribute('aria-label') ?? '').toLowerCase(),
        (el.getAttribute('title') ?? '').toLowerCase(),
        ((el as HTMLInputElement).placeholder ?? '').toLowerCase(),
      ];
      return candidates.some((c) => c.includes(query));
    }) as HTMLElement | undefined;

    if (!target) return `Element with text "${text}" not found`;
    this.flashHighlight(target);
    this.dispatchClick(target);
    return `Clicked element with text: ${text}`;
  }

  private inputText(selector: string, value: string): string {
    const el = document.querySelector(selector) as HTMLInputElement | HTMLTextAreaElement | null;
    if (!el) return `Input element not found: ${selector}`;

    this.flashHighlight(el);

    if (el.tagName.toLowerCase() === 'input' || el.tagName.toLowerCase() === 'textarea') {
      el.focus();
      el.dispatchEvent(new FocusEvent('focus', { bubbles: true }));

      // Use native value setter to bypass React controlled-component guards
      const prototype = Object.getPrototypeOf(el);
      const nativeSetter = Object.getOwnPropertyDescriptor(prototype, 'value')?.set;
      if (nativeSetter) {
        nativeSetter.call(el, value);
      } else {
        el.value = value;
      }

      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new InputEvent('input', { bubbles: true, data: value, inputType: 'insertText' }));
      el.dispatchEvent(new Event('change', { bubbles: true }));
      el.dispatchEvent(new FocusEvent('blur', { bubbles: true }));

      return `Set input ${selector} to "${value}"`;
    }

    if (el.isContentEditable) {
      el.focus();
      el.textContent = value;
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(
        new InputEvent('input', { bubbles: true, data: value, inputType: 'insertText' }),
      );
      el.dispatchEvent(new FocusEvent('blur', { bubbles: true }));
      return `Set contenteditable ${selector} to "${value}"`;
    }

    return `Element ${selector} is not an input`;
  }

  private scroll(
    selector: string | undefined,
    direction: 'up' | 'down' | 'top' | 'bottom',
  ): string {
    const el = selector
      ? (document.querySelector(selector) as HTMLElement | null)
      : (document.scrollingElement as HTMLElement | null);

    if (!el) return `Scroll target not found: ${selector ?? 'document'}`;

    const scrollAmount = 500;
    switch (direction) {
      case 'up':
        el.scrollBy({ top: -scrollAmount, behavior: 'smooth' });
        return `Scrolled up ${selector ?? 'document'}`;
      case 'down':
        el.scrollBy({ top: scrollAmount, behavior: 'smooth' });
        return `Scrolled down ${selector ?? 'document'}`;
      case 'top':
        el.scrollTo({ top: 0, behavior: 'smooth' });
        return `Scrolled to top ${selector ?? 'document'}`;
      case 'bottom':
        el.scrollTo({ top: el.scrollHeight, behavior: 'smooth' });
        return `Scrolled to bottom ${selector ?? 'document'}`;
    }
  }

  private async selectOption(selector: string, optionText: string): Promise<string> {
    const trigger = document.querySelector(selector) as HTMLElement | null;
    if (!trigger) return `Select trigger not found: ${selector}`;

    this.flashHighlight(trigger);
    trigger.click();
    await this.wait(200);

    const options = this.findDropdownOptions();
    const query = optionText.toLowerCase();
    const target = options.find((el) => this.extractText(el).toLowerCase().includes(query)) as
      | HTMLElement
      | undefined;

    if (!target) {
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      const optionTexts = options.slice(0, 20).map((el) => `"${this.extractText(el)}"`).join(', ');
      return `Option "${optionText}" not found in dropdown. Available options: ${optionTexts}`;
    }

    this.flashHighlight(target);
    target.click();
    return `Selected option "${optionText}" in ${selector}`;
  }

  private async wait(durationMs: number): Promise<string> {
    const ms = Math.max(0, Math.min(durationMs, 30000));
    await new Promise((r) => setTimeout(r, ms));
    return `Waited ${ms}ms`;
  }

  private pressKey(key: string): string {
    const normalized = key.trim();
    if (!normalized) return 'No key specified';
    document.dispatchEvent(
      new KeyboardEvent('keydown', { key: normalized, bubbles: true, cancelable: true }),
    );
    document.dispatchEvent(
      new KeyboardEvent('keyup', { key: normalized, bubbles: true, cancelable: true }),
    );
    return `Pressed key: ${normalized}`;
  }

  private readText(selector: string): string {
    const el = document.querySelector(selector);
    if (!el) return `Element not found: ${selector}`;
    const text = this.extractText(el).slice(0, 2000);
    return text || '(empty text)';
  }

  // ── DOM Utilities ─────────────────────────────────────────────────────────

  /** Find dropdown option elements using the prioritised selector list. */
  private findDropdownOptions(): Element[] {
    for (const sel of DROPDOWN_OPTION_SELECTORS) {
      const options = Array.from(document.querySelectorAll(sel));
      if (options.length > 0) return options;
    }
    return [];
  }

  /** Dispatch a realistic pointer+mouse+click event sequence on an element. */
  private dispatchClick(el: HTMLElement): void {
    const rect = el.getBoundingClientRect();
    const x = rect.left + rect.width / 2;
    const y = rect.top + rect.height / 2;
    const common = { bubbles: true, cancelable: true, clientX: x, clientY: y };
    el.dispatchEvent(new PointerEvent('pointerdown', { ...common, pointerType: 'mouse' }));
    el.dispatchEvent(new MouseEvent('mousedown', common));
    el.dispatchEvent(new PointerEvent('pointerup', { ...common, pointerType: 'mouse' }));
    el.dispatchEvent(new MouseEvent('mouseup', common));
    el.dispatchEvent(new MouseEvent('click', common));
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
      ].join(','),
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
      if (rect.right < 0 || rect.bottom < 0 || rect.left > viewportW || rect.top > viewportH)
        return;

      const style = window.getComputedStyle(htmlEl);
      if (
        style.display === 'none' ||
        style.visibility === 'hidden' ||
        parseFloat(style.opacity) < 0.01
      ) {
        return;
      }

      const tag = el.tagName.toLowerCase();
      const isLayoutContainer = [
        'body',
        'html',
        'main',
        'div',
        'section',
        'article',
        'nav',
        'aside',
      ].includes(tag);
      const dataTestid = el.getAttribute('data-testid') ?? undefined;
      const dataSelfControlTarget = el.getAttribute('data-self-control-target') ?? undefined;

      if (isLayoutContainer && !dataTestid && !dataSelfControlTarget && !el.id) {
        const isSmall = rect.width < 400 && rect.height < 200;
        const role = el.getAttribute('role');
        if (!isSmall || !role) return;
      }

      const text = this.extractText(el).slice(0, 120);
      const ariaLabel = el.getAttribute('aria-label') ?? undefined;
      const placeholder = (el as HTMLInputElement).placeholder ?? undefined;
      const title = el.getAttribute('title') ?? undefined;

      const hasIdentity = !!(
        text ||
        el.id ||
        dataTestid ||
        dataSelfControlTarget ||
        ariaLabel ||
        placeholder ||
        title
      );
      const isInteractive = this.isInteractive(el);
      if (!hasIdentity && !isInteractive) return;

      elements.push({
        tag,
        id: el.id || undefined,
        class: el.className || undefined,
        text,
        ariaLabel,
        role: el.getAttribute('role') ?? undefined,
        placeholder,
        title,
        dataTestid,
        dataSelfControlTarget,
        interactive: isInteractive,
        rect: {
          x: Math.round(rect.x),
          y: Math.round(rect.y),
          width: Math.round(rect.width),
          height: Math.round(rect.height),
        },
      });
    });

    elements.sort((a, b) => {
      const score = (e: SimplifiedElement) => {
        let s = 0;
        if (e.dataSelfControlTarget) s += 100;
        if (e.dataTestid) s += 80;
        if (
          e.interactive &&
          (e.tag === 'button' ||
            e.tag === 'a' ||
            e.tag === 'input' ||
            e.tag === 'select' ||
            e.tag === 'textarea')
        )
          s += 60;
        if (e.ariaLabel) s += 40;
        if (e.text) s += 20;
        if (e.interactive) s += 10;
        return s;
      };
      return score(b) - score(a);
    });

    return elements;
  }

  private buildTargetIndex(elements: SimplifiedElement[]): Record<string, string> {
    const targets: Record<string, string> = {};
    elements.forEach((el) => {
      if (el.dataSelfControlTarget) {
        targets[el.dataSelfControlTarget] = el.text || `<${el.tag}>`;
      }
      if (el.dataTestid) {
        targets[el.dataTestid] = el.text || `<${el.tag}>`;
      }
    });
    return targets;
  }

  private buildSemanticHints(
    activeScene: string,
    activeSettingsTab: string | undefined,
    elements: SimplifiedElement[],
    targets: Record<string, string>,
  ): string[] {
    const hints: string[] = [];

    if (activeScene === 'settings') {
      hints.push(`Current scene: Settings (${activeSettingsTab ?? 'unknown tab'})`);

      if (targets['primary-model-select']) {
        hints.push('You can change the primary model via the "primary-model-select" target.');
      }
      if (targets['fast-model-select']) {
        hints.push('You can change the fast model via the "fast-model-select" target.');
      }
    }

    const hasSelect = elements.some(
      (el) => el.class?.includes('select__trigger') || el.role === 'combobox',
    );
    const hasInput = elements.some((el) => el.tag === 'input' || el.tag === 'textarea');
    const hasSwitch = elements.some(
      (el) => el.role === 'switch' || el.class?.includes('switch'),
    );

    if (hasSelect) hints.push('This page contains dropdown selects.');
    if (hasInput) hints.push('This page contains text inputs.');
    if (hasSwitch) hints.push('This page contains toggle switches.');

    const quickActions = [
      'open_scene with sceneId "session" to return to the chat',
      'open_scene with sceneId "settings" to open settings',
      'execute_task with task "open_model_settings" to jump directly to model settings',
      'execute_task with task "set_primary_model" and params { modelQuery: "..." } to set the main model',
      'execute_task with task "delete_model" and params { modelQuery: "..." } to delete a model',
    ];
    hints.push(`Quick actions: ${quickActions.join('; ')}`);

    return hints;
  }

  private async maybeAppendModelSummary(hints: string[]): Promise<void> {
    try {
      const models = await this.fetchModels(true);
      if (models.length === 0) return;
      const lines = models.map(
        (m) => `- ${m.enabled ? '[enabled]' : '[disabled]'} ${m.displayName} (${m.provider}, ID: ${m.id})`,
      );
      hints.push(`Configured models:\n${lines.join('\n')}`);
    } catch {
      // ignore
    }
  }

  private extractText(el: Element): string {
    const tag = el.tagName.toLowerCase();
    const directAria = el.getAttribute('aria-label') ?? '';
    const directTitle = (el as HTMLElement).title ?? '';

    const isContainer = [
      'div',
      'section',
      'article',
      'main',
      'nav',
      'aside',
      'header',
      'footer',
    ].includes(tag);

    if (isContainer) {
      if (directAria) return directAria;
      if (directTitle) return directTitle;
      if (el.id) return '';

      const interactiveChildren = el.querySelectorAll(
        'button, a, input, [role="button"], [role="tab"], [data-testid]',
      ).length;
      if (interactiveChildren > 4) {
        return '';
      }
    }

    const walk = (node: Node): string => {
      if (node.nodeType === Node.TEXT_NODE) {
        return node.textContent ?? '';
      }
      if (node.nodeType !== Node.ELEMENT_NODE) {
        return '';
      }
      const elNode = node as HTMLElement;
      const style = window.getComputedStyle(elNode);
      if (style.display === 'none' || style.visibility === 'hidden') {
        return '';
      }
      return Array.from(elNode.childNodes).map(walk).join('').replace(/\s+/g, ' ').trim();
    };

    const childText = walk(el);
    return (directAria || childText || directTitle || '').trim();
  }

  private isInteractive(el: Element): boolean {
    const tag = el.tagName.toLowerCase();
    const role = el.getAttribute('role');
    if (['button', 'a', 'input', 'textarea', 'select', 'label'].includes(tag)) return true;
    if (
      [
        'button',
        'link',
        'tab',
        'menuitem',
        'combobox',
        'option',
        'radio',
        'checkbox',
        'switch',
      ].includes(role ?? '')
    )
      return true;
    if ((el as HTMLElement).onclick != null) return true;
    if (el.getAttribute('tabindex') === '0') return true;
    if (el.classList.contains('select__trigger') || el.classList.contains('select__option'))
      return true;
    if (el.getAttribute('contenteditable') === 'true') return true;
    return false;
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
