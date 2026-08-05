import { hilog } from '@kit.PerformanceAnalysisKit';
import RustModule from 'libbitfun_desktop_lib.so';
import {
  RustWebviewNodeController,
  WebviewInitData,
  WebviewStyle,
  JsHelper,
} from '@ohos-rs/ability';

const BROWSER_DOMAIN: number = 0x0000;
const BROWSER_TAG: string = 'BitFunBrowserWebview';

interface BrowserWebviewEntry {
  webTag: string;
  controller: JsHelper;
  initData: WebviewInitData;
}

export interface CreateRequest {
  label: string;
  url: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface EvalRequest {
  label: string;
  script: string;
}

export interface NavigateRequest {
  label: string;
  url: string;
}

export interface BoundsRequest {
  label: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface LabelRequest {
  label: string;
}

interface OkResponse {
  ok: boolean;
}

interface OkResultResponse {
  ok: boolean;
  result: string;
}

interface ErrorResponse {
  error: string;
}

/**
 * Singleton service that bridges the Rust `browser_webview_*` Tauri commands to
 * the ArkUI `RustWebviewNodeController` (vendored `@ohos-rs/ability`). Each
 * command registers an ArkTS callback via `RustModule.registerArktsFunction`
 * in `EntryAbility.onWindowStageCreate`; the callback parses a JSON-string
 * request, delegates to the matching method here, and returns a JSON-string
 * envelope (`{ok:true}` / `{ok:true,result:"..."}` / `{error:"..."}`) that the
 * Rust side decodes uniformly.
 *
 * Page-load lifecycle (`started` / `finished`) is forwarded to the web-ui by
 * calling `RustModule.emitBrowserPageLoad(label, event, url)` from the
 * `onPageBegin` / `onPageEnd` callbacks wired into each created webview. The
 * web-ui's `useEmbeddedBrowserWebview` hook listens on the
 * `browser-webview-page-load` event (emitted by the Rust `#[napi]` function)
 * and updates `isLoading`, the address bar URL, and re-injects the
 * `BLANK_TARGET_INTERCEPT_SCRIPT` + `STREAM_RENDER_OPTIMIZATION_SCRIPT`.
 */
export class BrowserWebviewService {
  private static instance: BrowserWebviewService | null = null;
  private entries: Map<string, BrowserWebviewEntry> = new Map();
  private controller: RustWebviewNodeController | null = null;

  private constructor() {}

  static getInstance(): BrowserWebviewService {
    if (BrowserWebviewService.instance === null) {
      BrowserWebviewService.instance = new BrowserWebviewService();
    }
    return BrowserWebviewService.instance;
  }

  private resolveController(): RustWebviewNodeController | null {
    if (this.controller !== null) {
      return this.controller;
    }
    const fromStorage = AppStorage.get<RustWebviewNodeController>('rustWebviewController');
    if (fromStorage) {
      this.controller = fromStorage;
      return this.controller;
    }
    hilog.warn(BROWSER_DOMAIN, BROWSER_TAG, 'rustWebviewController not yet registered in AppStorage');
    return null;
  }

  private errorEnvelope(message: string): string {
    const err: ErrorResponse = { error: message };
    return JSON.stringify(err);
  }

  private okEnvelope(): string {
    const ok: OkResponse = { ok: true };
    return JSON.stringify(ok);
  }

  async create(request: CreateRequest): Promise<string> {
    const controller = this.resolveController();
    if (controller === null) {
      return this.errorEnvelope('controller not ready');
    }
    const label = request.label;
    // Drop any stale entry sharing this label before re-creating so a
    // failed/re-tried create does not leave an orphaned BuilderNode.
    await this.closeInternal(label);
    const style: WebviewStyle = {
      x: request.x,
      y: request.y,
      width: request.width,
      height: request.height,
      visible: 'hidden',
    };
    // The page-load callbacks close over `label` (stable for the webview's
    // lifetime) and forward to the Rust-side emitter so the web-ui's listener
    // can react exactly like the desktop `.on_page_load` path.
    const initData: WebviewInitData = {
      webTag: label,
      url: request.url,
      style: style,
      javascriptEnable: true,
      devtools: true,
      onNavigationRequest: (url: string): boolean => {
        // RustWebviewNodeController forwards this value to ArkUI Web's
        // `onLoadIntercept`: `true` means the request is intercepted (and the
        // page is not loaded), while `false` allows normal navigation.
        // The embedded browser must allow both the initial URL and subsequent
        // address-bar navigations to reach the Web component.
        return false;
      },
      onPageBegin: (url: string): void => {
        RustModule.emitBrowserPageLoad(label, 'started', url);
      },
      onPageEnd: (url: string): void => {
        RustModule.emitBrowserPageLoad(label, 'finished', url);
      },
    };
    const ret = controller.addWebview(initData);
    const entry: BrowserWebviewEntry = {
      webTag: ret.webTag,
      controller: ret.controller,
      initData: initData,
    };
    this.entries.set(label, entry);
    hilog.info(BROWSER_DOMAIN, BROWSER_TAG, `created webview label=${label} webTag=${ret.webTag} url=${request.url}`);
    return this.okEnvelope();
  }

  async eval(request: EvalRequest): Promise<string> {
    const entry = this.entries.get(request.label);
    if (entry === undefined) {
      return this.errorEnvelope(`webview not found: ${request.label}`);
    }
    // The JsHelper callback wrapper fires exactly once when the underlying
    // WebviewController.runJavaScript promise resolves; wrap it in a Promise
    // so the Rust side can await the script's return value.
    const result = await new Promise<string>((resolve: (value: string) => void): void => {
      entry.controller.runJavaScript(request.script, (res: string | undefined): void => {
        resolve(res ?? '');
      });
    });
    const ok: OkResultResponse = { ok: true, result: result };
    return JSON.stringify(ok);
  }

  async navigate(request: NavigateRequest): Promise<string> {
    const entry = this.entries.get(request.label);
    if (entry === undefined) {
      return this.errorEnvelope(`webview not found: ${request.label}`);
    }
    entry.controller.loadUrl(request.url);
    hilog.info(BROWSER_DOMAIN, BROWSER_TAG, `navigated label=${request.label} url=${request.url}`);
    return this.okEnvelope();
  }

  async reload(request: LabelRequest): Promise<string> {
    const entry = this.entries.get(request.label);
    if (entry === undefined) {
      return this.errorEnvelope(`webview not found: ${request.label}`);
    }
    entry.controller.refresh();
    return this.okEnvelope();
  }

  async setBounds(request: BoundsRequest): Promise<string> {
    const entry = this.entries.get(request.label);
    if (entry === undefined) {
      return this.errorEnvelope(`webview not found: ${request.label}`);
    }
    const style = entry.initData.style;
    if (style === undefined) {
      return this.errorEnvelope(`webview has no style: ${request.label}`);
    }
    style.x = request.x;
    style.y = request.y;
    style.width = request.width;
    style.height = request.height;
    // Re-build the BuilderNode with the mutated style so the Web component
    // re-applies .width/.height/.position. Same pattern the original
    // DefaultXComponent used for setVisible/setBackgroundColor.
    const controller = this.resolveController();
    const node = controller?.getWebviewNode(entry.webTag);
    node?.update(entry.initData);
    return this.okEnvelope();
  }

  async show(request: LabelRequest): Promise<string> {
    const entry = this.entries.get(request.label);
    if (entry === undefined) {
      return this.errorEnvelope(`webview not found: ${request.label}`);
    }
    entry.controller.setVisible(true);
    return this.okEnvelope();
  }

  async hide(request: LabelRequest): Promise<string> {
    const entry = this.entries.get(request.label);
    if (entry === undefined) {
      return this.errorEnvelope(`webview not found: ${request.label}`);
    }
    entry.controller.setVisible(false);
    return this.okEnvelope();
  }

  async setFocus(request: LabelRequest): Promise<string> {
    const entry = this.entries.get(request.label);
    if (entry === undefined) {
      return this.errorEnvelope(`webview not found: ${request.label}`);
    }
    entry.controller.requestFocus();
    return this.okEnvelope();
  }

  async close(request: LabelRequest): Promise<string> {
    await this.closeInternal(request.label);
    return this.okEnvelope();
  }

  private async closeInternal(label: string): Promise<void> {
    const entry = this.entries.get(label);
    if (entry === undefined) {
      return;
    }
    const controller = this.resolveController();
    controller?.removeWebview(entry.webTag);
    this.entries.delete(label);
    hilog.info(BROWSER_DOMAIN, BROWSER_TAG, `closed webview label=${label} webTag=${entry.webTag}`);
  }
}
