import { NodeContent } from "@kit.ArkUI";

export interface ApplicationLifecycle {
  environmentCallback: EnvironmentCallback;
  windowStageEventCallback: WindowStageEventCallback;
}

export interface EnvironmentCallback {
  onConfigurationUpdated: (arg: object) => void;
  onMemoryLevel: (arg: number) => void;
}

export interface WindowStageEventCallback {
  onWindowStageCreate: () => void;
  onWindowStageDestroy: () => void;
  onAbilityCreate: (arg: string) => void;
  onAbilityDestroy: () => void;
  onAbilitySaveState: () => string;
  onWindowStageEvent: (arg: number) => void;
  onWindowSizeChange: (arg: object) => void;
  onWindowRectChange: (arg: object) => void;
}

export interface WebViewComponentEventCallback {
  onComponentCreated: () => void;
  onComponentDestroyed: () => void;
}

export interface OnDownloadStartResult {
  allow?: boolean;
  tempPath?: string;
}

export interface WebViewInitData {
  url?: string;
  id?: string;
  style?: WebViewStyle;
  javascriptEnable?: boolean;
  devtools?: boolean;
  transparent?: boolean;
  autoplay?: boolean;
  userAgent?: string;
  initializationScripts?: string[];
  headers?: Record<string, string>;
  html?: string;
  onDragAndDrop?: (event: string) => void;
  onDownloadStart?: (url: string, tempPath: string | undefined) => OnDownloadStartResult;
  onDownloadEnd?: (url: string, tempPath: string | undefined, success: boolean) => void;
  onNavigationRequest?: (url: string) => boolean;
  onTitleChange?: (title: string) => void;
}

export interface WebViewStyle {
  x?: number | string;
  y?: number | string;
}

export interface Module {
  init: () => ApplicationLifecycle;
  // XComponent mode
  render: (helper: ArkHelper, slot: NodeContent) => void;

  registerCustomProtocol: () => void;
}

export interface ArkHelper {
  exit: (code: number) => void;
  createWebview: (data: WebViewInitData) => Object;
}
