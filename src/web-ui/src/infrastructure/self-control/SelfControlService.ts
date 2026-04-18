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
import { useMiniAppStore } from '@/app/scenes/miniapps/miniAppStore';
import { configManager } from '@/infrastructure/config';
import { getModelDisplayName } from '@/infrastructure/config/services/modelConfigs';
import { matchProviderCatalogItemByBaseUrl } from '@/infrastructure/config/services/providerCatalog';
import { createLogger } from '@/shared/utils/logger';

const logger = createLogger('SelfControlService');

/**
 * Thrown when a SelfControl action cannot be carried out (target missing,
 * invalid params, frontend pre-condition unmet, etc.).
 *
 * The {@link SelfControlEventListener} catches every Error subclass and
 * reports `success: false` to the backend, which is the only way the LLM
 * can learn that an "operation succeeded" actually failed silently.
 *
 * Background: prior to Phase 1 these failure paths returned a descriptive
 * STRING from the DOM primitives (e.g. `"Element not found: ..."`). The
 * listener wrapped that string into `{ success: true, result }` and the LLM
 * happily proceeded as if the click had landed.
 */
export class SelfControlError extends Error {
  constructor(
    message: string,
    /** Stable machine-readable code (uppercase snake) for caller branching. */
    readonly code: string = 'FRONTEND_ERROR',
    /** Optional structured hints attached to the failure. */
    readonly hints: string[] = [],
  ) {
    super(message);
    this.name = 'SelfControlError';
  }
}

/**
 * Specialisation thrown by `setDefaultModel` / `deleteModel` so that
 * `executeTask` can fall back to the UI-driven path without parsing the
 * English error text (the previous `.includes('not found')` heuristic was
 * fragile and locale-sensitive).
 */
export class ModelNotFoundError extends SelfControlError {
  constructor(message: string, hints: string[] = []) {
    super(message, 'NOT_FOUND', hints);
    this.name = 'ModelNotFoundError';
  }
}

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
  /**
   * Phase 3: pagination metadata for `elements`. Always present so the
   * model can decide whether to fetch the next page (`offset + returned`).
   */
  pagination?: {
    offset: number;
    limit: number;
    returned: number;
    total: number;
    hasMore: boolean;
  };
  /** Best-effort identifier for the originating Tauri webview. */
  webviewId?: string;
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
  | { type: 'open_miniapp'; miniAppId: string }
  | { type: 'set_config'; key: string; configValue: unknown }
  | { type: 'get_config'; key: string }
  | { type: 'list_models'; includeDisabled?: boolean }
  | { type: 'set_default_model'; modelQuery: string; slot?: 'primary' | 'fast' }
  | { type: 'select_option'; selector: string; optionText: string }
  | {
      type: 'get_page_state';
      /** Pagination — first element index to include (default 0). */
      offset?: number;
      /**
       * Pagination — max elements to include in `elements` (default 60).
       * Phase 3: the legacy implementation always returned at most 60
       * elements with no way to get the rest, which made BitFun's own
       * settings panes (often >60 controls) un-driveable past the first
       * page. The result now reports `totalElements` and `hasMoreElements`
       * so the model can page through.
       */
      limit?: number;
    }
  | {
      /**
       * Phase 3: poll the DOM for a selector to appear (e.g. wait for a
       * modal animation to finish before clicking inside it). Until now
       * the agent had to fall back to fixed `wait { durationMs }` calls,
       * which were either too short (race) or too long (slow). Mirrors
       * Playwright's `waitForSelector` semantics.
       */
      type: 'wait_for_selector';
      selector: string;
      timeoutMs?: number;
      /** `'visible'` requires a non-zero bounding rect. */
      state?: 'attached' | 'visible';
    }
  | { type: 'wait'; durationMs: number }
  | { type: 'press_key'; key: string; targetSelector?: string }
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
  miniAppId?: string;
  mini_app_id?: string;
  miniappId?: string;
  optionText?: string;
  option_text?: string;
  durationMs?: number;
  duration_ms?: number;
  includeDisabled?: boolean;
  include_disabled?: boolean;
  targetSelector?: string;
  target_selector?: string;
  offset?: number;
  limit?: number;
  timeoutMs?: number;
  timeout_ms?: number;
  state?: 'attached' | 'visible';
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

  async getPageState(opts?: { offset?: number; limit?: number }): Promise<PageState> {
    const offset = Math.max(0, Math.floor(opts?.offset ?? 0));
    const limit = Math.max(1, Math.floor(opts?.limit ?? 60));

    const activeScene = useSceneStore.getState().activeTabId;
    const activeSettingsTab =
      activeScene === 'settings' ? useSettingsStore.getState().activeTab : undefined;
    const elements = this.collectInteractiveElements();
    const targets = this.buildTargetIndex(elements);
    const semanticHints = this.buildSemanticHints(activeScene, activeSettingsTab, elements, targets);

    if (activeScene === 'settings' && activeSettingsTab === 'models') {
      await this.maybeAppendModelSummary(semanticHints);
    }

    const pagedElements = elements.slice(offset, offset + limit);
    const hasMoreElements = offset + pagedElements.length < elements.length;

    return {
      title: document.title,
      activeScene,
      activeSettingsTab,
      elements: pagedElements,
      targets,
      semanticHints,
      // Phase 3: pagination metadata + a stable webview identifier so the
      // backend (and the model) can tell which Tauri webview produced the
      // page state. `webview_id` is best-effort: we use the window name
      // when available, falling back to a per-tab uuid stored on `window`.
      pagination: {
        offset,
        limit,
        returned: pagedElements.length,
        total: elements.length,
        hasMore: hasMoreElements,
      },
      webviewId: this.resolveWebviewId(),
    };
  }

  /**
   * Best-effort identifier for the current webview. Tauri exposes this
   * through `window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label`
   * but that path isn't part of the public contract — fall back to a
   * per-tab uuid persisted on `window` so at minimum the value is stable
   * within a single page lifetime.
   */
  private resolveWebviewId(): string {
    const w = window as unknown as {
      __BITFUN_WEBVIEW_ID__?: string;
      __TAURI_INTERNALS__?: {
        metadata?: { currentWindow?: { label?: string } };
      };
    };
    const tauriLabel = w.__TAURI_INTERNALS__?.metadata?.currentWindow?.label;
    if (tauriLabel) return tauriLabel;
    if (!w.__BITFUN_WEBVIEW_ID__) {
      w.__BITFUN_WEBVIEW_ID__ = `webview-${Math.random().toString(36).slice(2, 10)}`;
    }
    return w.__BITFUN_WEBVIEW_ID__;
  }

  /**
   * Phase 3: poll the DOM for a selector. Resolves with a JSON summary
   * when the element is found (`visible` mode also requires non-zero
   * bounding rect); throws `SelfControlError(code='TIMEOUT')` if the
   * deadline elapses. Polling cadence is 100 ms — short enough for
   * snappy feedback, infrequent enough to avoid burning CPU.
   */
  async waitForSelector(
    selector: string,
    timeoutMs: number = 5000,
    state: 'attached' | 'visible' = 'visible',
  ): Promise<string> {
    const deadline = Date.now() + Math.max(0, timeoutMs);
    const requireVisible = state === 'visible';

    while (true) {
      const el = document.querySelector(selector) as HTMLElement | null;
      if (el) {
        if (!requireVisible) {
          return JSON.stringify({ found: true, selector, state });
        }
        const rect = el.getBoundingClientRect();
        if (rect.width > 0 && rect.height > 0) {
          const cs = window.getComputedStyle(el);
          if (cs.display !== 'none' && cs.visibility !== 'hidden') {
            return JSON.stringify({
              found: true,
              selector,
              state,
              rect: { x: Math.round(rect.x), y: Math.round(rect.y), w: Math.round(rect.width), h: Math.round(rect.height) },
            });
          }
        }
      }
      if (Date.now() >= deadline) {
        throw new SelfControlError(
          `Timed out after ${timeoutMs}ms waiting for selector '${selector}' to be ${state}`,
          'TIMEOUT',
        );
      }
      await new Promise((r) => setTimeout(r, 100));
    }
  }

  // ── Dispatcher ───────────────────────────────────────────────────────────

  async executeAction(rawAction: SelfControlIncomingAction | SelfControlAction): Promise<string> {
    const action = this.normalizeAction(rawAction);
    logger.info('Executing self-control action', { type: action.type });

    switch (action.type) {
      case 'execute_task':
        return this.executeTask(action.task, action.params);

      case 'get_page_state':
        return JSON.stringify(
          await this.getPageState({ offset: action.offset, limit: action.limit }),
          null,
          2,
        );

      case 'wait_for_selector':
        return this.waitForSelector(
          action.selector,
          action.timeoutMs ?? 5000,
          action.state ?? 'visible',
        );

      // Region 2: App State
      case 'open_scene':
        return this.openScene(action.sceneId);
      case 'open_settings_tab':
        return this.openSettingsTab(action.tabId);
      case 'open_miniapp':
        return this.openMiniApp(action.miniAppId);

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
        return this.pressKey(action.key, action.targetSelector);
      case 'read_text':
        return this.readText(action.selector);

      default:
        throw new SelfControlError(
          `Unknown action type: ${(action as { type: string }).type}`,
          'INVALID_PARAMS',
        );
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
        if (!modelQuery) {
          throw new SelfControlError(`Missing modelQuery for ${task}`, 'INVALID_PARAMS');
        }

        // Try the config-driven path first; on ModelNotFoundError fall back
        // to the visual UI dropdown. Any other error propagates so the caller
        // sees the actual reason instead of being misled into the UI fallback.
        try {
          return await this.setDefaultModel(modelQuery, slot);
        } catch (err) {
          if (err instanceof ModelNotFoundError) {
            return await this.setDefaultModelViaUI(modelQuery, slot);
          }
          throw err;
        }
      }

      case 'open_model_settings': {
        return this.openSettingsTab('models');
      }

      case 'return_to_session': {
        return this.openScene('session');
      }

      case 'delete_model': {
        const modelQuery = params?.modelQuery ?? params?.model ?? '';
        if (!modelQuery) {
          throw new SelfControlError('Missing modelQuery for delete_model', 'INVALID_PARAMS');
        }
        return this.deleteModel(modelQuery);
      }

      case 'open_miniapp_gallery': {
        return this.openScene('miniapps');
      }

      case 'open_miniapp': {
        const miniAppId = params?.miniAppId ?? params?.mini_app_id ?? params?.miniappId ?? '';
        if (!miniAppId) {
          throw new SelfControlError('Missing miniAppId for open_miniapp', 'INVALID_PARAMS');
        }
        return this.openMiniApp(miniAppId);
      }

      default:
        throw new SelfControlError(
          `Unknown task: ${task}. Available tasks: set_primary_model, set_fast_model, open_model_settings, return_to_session, delete_model, open_miniapp_gallery, open_miniapp.`,
          'INVALID_PARAMS',
        );
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
      miniAppId: raw.miniAppId ?? raw.mini_app_id ?? raw.miniappId,
      optionText: raw.optionText ?? raw.option_text,
      durationMs: raw.durationMs ?? raw.duration_ms,
      includeDisabled: raw.includeDisabled ?? raw.include_disabled,
      targetSelector: raw.targetSelector ?? raw.target_selector,
      timeoutMs: raw.timeoutMs ?? raw.timeout_ms,
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

  private openMiniApp(miniAppId: string): string {
    const id = (miniAppId ?? '').trim();
    if (!id) {
      throw new SelfControlError('open_miniapp requires miniAppId', 'INVALID_PARAMS');
    }
    const known = useMiniAppStore.getState().apps.find((app) => app.id === id);
    if (!known) {
      const available = useMiniAppStore
        .getState()
        .apps.map((app) => `"${app.name}" (id=${app.id})`)
        .join(', ');
      throw new SelfControlError(
        `Mini-app id "${id}" is not installed.`,
        'NOT_FOUND',
        [
          available
            ? `Installed mini-apps: ${available}.`
            : 'No mini-apps are installed yet.',
          'Call ControlHub domain="app" action="list_miniapps" first to discover ids.',
        ],
      );
    }
    useMiniAppStore.getState().openApp(id);
    useSceneStore.getState().openScene(`miniapp:${id}` as any);
    return `Opened mini-app "${known.name}" (id=${id})`;
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
      throw new SelfControlError(
        'No enabled models found. Please configure models first.',
        'NOT_FOUND',
      );
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
    throw new ModelNotFoundError(`Model "${modelQuery}" not found in enabled models.`, [
      `Available enabled models: ${available}`,
      'Use list_models to see exact names, or call execute_task with task="set_primary_model" to let SelfControl try the UI dropdown.',
    ]);
  }

  private async setDefaultModelViaUI(modelQuery: string, slot: 'primary' | 'fast'): Promise<string> {
    this.openSettingsTab('models');
    await this.wait(300);

    const targetAttr = slot === 'primary' ? 'primary-model-select' : 'fast-model-select';
    const selector = `[data-self-control-target="${targetAttr}"] .select__trigger`;
    const trigger = document.querySelector(selector) as HTMLElement | null;

    if (!trigger) {
      throw new SelfControlError(
        `Could not find ${slot} model selector in the UI. The model settings page may not be fully loaded.`,
        'NOT_FOUND',
        ['Call get_page_state to verify the settings/models tab is rendered, then retry.'],
      );
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
      throw new ModelNotFoundError(
        `Model "${modelQuery}" not found in the ${slot} dropdown.`,
        [`Available options: ${optionTexts}`],
      );
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
      throw new SelfControlError('No models configured.', 'NOT_FOUND');
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
      throw new ModelNotFoundError(`Model matching "${modelQuery}" not found.`, [
        `Available models: ${available}`,
      ]);
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
    if (!el) {
      throw new SelfControlError(`Element not found: ${selector}`, 'NOT_FOUND', [
        'Call get_page_state and use a real selector / data-testid before retrying.',
      ]);
    }
    this.flashHighlight(el);
    this.dispatchClick(el);
    return `Clicked element: ${selector}`;
  }

  private clickElementByText(text: string, tag?: string): string {
    const selector = tag ?? '*';
    const elements = Array.from(document.querySelectorAll(selector));
    const query = text.toLowerCase().trim();

    const matches = elements.filter((el) => {
      const candidates = [
        this.extractText(el).toLowerCase(),
        (el.getAttribute('aria-label') ?? '').toLowerCase(),
        (el.getAttribute('title') ?? '').toLowerCase(),
        ((el as HTMLInputElement).placeholder ?? '').toLowerCase(),
      ];
      return candidates.some((c) => c.includes(query));
    });

    if (matches.length === 0) {
      throw new SelfControlError(`Element with text "${text}" not found`, 'NOT_FOUND', [
        'Call get_page_state to see the actual visible labels.',
        'Or pass `tag` to narrow the search (e.g. tag="button").',
      ]);
    }

    // Disambiguate: prefer interactive elements (button/a/role=button/tab/menuitem).
    const interactive = matches.filter((el) => this.isInteractive(el as HTMLElement));
    const candidates = interactive.length > 0 ? interactive : matches;

    if (candidates.length > 1) {
      const previews = candidates
        .slice(0, 5)
        .map((el, i) => `${i + 1}: <${el.tagName.toLowerCase()}> "${this.extractText(el).slice(0, 60)}"`)
        .join(' | ');
      throw new SelfControlError(
        `Ambiguous: ${candidates.length} elements match text "${text}".`,
        'AMBIGUOUS',
        [
          `First candidates: ${previews}.`,
          'Pass `tag` (e.g. "button") to narrow, or use action="click" with a precise CSS selector / data-testid.',
        ],
      );
    }

    const target = candidates[0] as HTMLElement;
    this.flashHighlight(target);
    this.dispatchClick(target);
    return `Clicked element with text: ${text}`;
  }

  private inputText(selector: string, value: string): string {
    const el = document.querySelector(selector) as HTMLInputElement | HTMLTextAreaElement | null;
    if (!el) {
      throw new SelfControlError(`Input element not found: ${selector}`, 'NOT_FOUND');
    }

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

    throw new SelfControlError(
      `Element ${selector} is not a writable input/textarea/contenteditable.`,
      'INVALID_PARAMS',
    );
  }

  private async scroll(
    selector: string | undefined,
    direction: 'up' | 'down' | 'top' | 'bottom',
  ): Promise<string> {
    const el = selector
      ? (document.querySelector(selector) as HTMLElement | null)
      : (document.scrollingElement as HTMLElement | null);

    if (!el) {
      throw new SelfControlError(
        `Scroll target not found: ${selector ?? 'document'}`,
        'NOT_FOUND',
      );
    }

    const scrollAmount = 500;
    const before = el.scrollTop;
    switch (direction) {
      case 'up':
        el.scrollBy({ top: -scrollAmount, behavior: 'smooth' });
        break;
      case 'down':
        el.scrollBy({ top: scrollAmount, behavior: 'smooth' });
        break;
      case 'top':
        el.scrollTo({ top: 0, behavior: 'smooth' });
        break;
      case 'bottom':
        el.scrollTo({ top: el.scrollHeight, behavior: 'smooth' });
        break;
    }

    // Smooth scroll is async; wait until the scroll position settles so the
    // next action observes the new viewport. Bail out after ~600ms either way
    // (matches a typical CSS smooth-scroll duration).
    await this.waitForScrollSettle(el, before);
    return `Scrolled ${direction} ${selector ?? 'document'} (from=${Math.round(before)} to=${Math.round(el.scrollTop)})`;
  }

  /** Poll until two consecutive `scrollTop` reads match, capped at 600ms. */
  private async waitForScrollSettle(el: HTMLElement, _before: number): Promise<void> {
    const start = performance.now();
    let last = el.scrollTop;
    while (performance.now() - start < 600) {
      await new Promise((r) => setTimeout(r, 60));
      const cur = el.scrollTop;
      if (Math.abs(cur - last) < 0.5) return;
      last = cur;
    }
  }

  private async selectOption(selector: string, optionText: string): Promise<string> {
    const trigger = document.querySelector(selector) as HTMLElement | null;
    if (!trigger) {
      throw new SelfControlError(`Select trigger not found: ${selector}`, 'NOT_FOUND');
    }

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
      throw new SelfControlError(
        `Option "${optionText}" not found in dropdown.`,
        'NOT_FOUND',
        [`Available options: ${optionTexts}`],
      );
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

  private pressKey(key: string, targetSelector?: string): string {
    const normalized = key.trim();
    if (!normalized) {
      throw new SelfControlError('No key specified', 'INVALID_PARAMS');
    }

    // Prefer an explicit target → focused element → document. Dispatching key
    // events on `document` only works if some element already absorbs them;
    // otherwise the keystroke is silently dropped, which historically caused
    // the model to think a "Pressed Enter" had submitted a form when it hadn't.
    let target: EventTarget;
    if (targetSelector) {
      const el = document.querySelector(targetSelector) as HTMLElement | null;
      if (!el) {
        throw new SelfControlError(
          `press_key target_selector not found: ${targetSelector}`,
          'NOT_FOUND',
        );
      }
      el.focus();
      target = el;
    } else {
      const active = document.activeElement;
      if (!active || active === document.body) {
        throw new SelfControlError(
          'press_key requires `target_selector` (or some element to be focused first).',
          'MISSING_SESSION',
          [
            'Pass `target_selector` so the keystroke lands somewhere observable.',
            'Or call action="click" / action="input" first to focus an input, then retry.',
          ],
        );
      }
      target = active;
    }

    target.dispatchEvent(
      new KeyboardEvent('keydown', { key: normalized, bubbles: true, cancelable: true }),
    );
    target.dispatchEvent(
      new KeyboardEvent('keyup', { key: normalized, bubbles: true, cancelable: true }),
    );
    return `Pressed key: ${normalized}${targetSelector ? ` on ${targetSelector}` : ''}`;
  }

  private readText(selector: string): string {
    const el = document.querySelector(selector);
    if (!el) {
      throw new SelfControlError(`Element not found: ${selector}`, 'NOT_FOUND');
    }
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
