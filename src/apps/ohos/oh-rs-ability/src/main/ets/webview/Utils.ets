import webview from "@ohos.web.webview";

export const getCookies = (url: string): string => {
  return webview.WebCookieManager.fetchCookieSync(url);
};

export interface JsHelper {
  getCookies: (url: string) => string;
  getUrl: () => string;
  loadUrl: (url: string, header?: Record<string, string>) => void;
  loadHtml: (html: string) => void;
  zoom: (scale: number) => void;
  refresh: () => void;
  requestFocus: () => void;
  runJavaScript: (code: string, callback: (result?: string) => void) => void;
  setBackgroundColor: (color: string) => void;
  setVisible: (visible: boolean) => void;
  clearAllBrowsingData: () => void;
}
