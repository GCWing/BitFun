/**
 * useMiniAppBridge — handles postMessage JSON-RPC from the MiniApp iframe:
 * worker.call → JS Worker, dialog.open/save/message → Tauri dialog.
 * Also handles bitfun/request-theme and pushes theme changes to the iframe.
 */
import { useLayoutEffect, useRef, useEffect, RefObject } from 'react';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import { open as dialogOpen, save as dialogSave, message as dialogMessage } from '@tauri-apps/plugin-dialog';
import type { MiniApp } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useTheme } from '@/infrastructure/theme/hooks/useTheme';
import { buildMiniAppThemeVars } from '../utils/buildMiniAppThemeVars';

interface JSONRPC {
  jsonrpc?: string;
  id: number | string;
  method: string;
  params?: Record<string, unknown>;
}

export function useMiniAppBridge(
  iframeRef: RefObject<HTMLIFrameElement>,
  app: MiniApp,
) {
  const { theme: currentTheme } = useTheme();
  const themeRef = useRef(currentTheme);
  themeRef.current = currentTheme;

  const appIdRef = useRef(app.id);
  useLayoutEffect(() => {
    appIdRef.current = app.id;
  }, [app.id]);

  useLayoutEffect(() => {
    const handler = async (event: MessageEvent) => {
      if (!iframeRef.current || event.source !== iframeRef.current.contentWindow) return;
      const msg = event.data as JSONRPC & { method?: string };
      if (!msg?.method) return;

      const { id, method, params = {} } = msg;
      const appId = appIdRef.current;
      const reply = (result: unknown) =>
        iframeRef.current?.contentWindow?.postMessage({ jsonrpc: '2.0', id, result }, '*');
      const replyError = (message: string) =>
        iframeRef.current?.contentWindow?.postMessage(
          { jsonrpc: '2.0', id, error: { code: -32000, message } },
          '*',
        );

      if (method === 'bitfun/request-theme') {
        const payload = buildMiniAppThemeVars(themeRef.current);
        if (payload && iframeRef.current?.contentWindow) {
          iframeRef.current.contentWindow.postMessage(
            { type: 'bitfun:event', event: 'themeChange', payload },
            '*',
          );
        }
        return;
      }

      try {
        if (method === 'worker.call') {
          const result = await miniAppAPI.workerCall(
            appId,
            (params.method as string) ?? '',
            (params.params as Record<string, unknown>) ?? {},
          );
          reply(result);
          return;
        }
        if (method === 'dialog.open') {
          reply(await dialogOpen(params as Parameters<typeof dialogOpen>[0]));
          return;
        }
        if (method === 'dialog.save') {
          reply(await dialogSave(params as Parameters<typeof dialogSave>[0]));
          return;
        }
        if (method === 'dialog.message') {
          reply(await dialogMessage(params as Parameters<typeof dialogMessage>[0]));
          return;
        }
        replyError(`Unknown method: ${method}`);
      } catch (error) {
        replyError(String(error));
      }
    };
    window.addEventListener('message', handler);
    return () => {
      window.removeEventListener('message', handler);
    };
  }, []);

  useEffect(() => {
    const payload = buildMiniAppThemeVars(currentTheme);
    if (!payload || !iframeRef.current?.contentWindow) return;
    iframeRef.current.contentWindow.postMessage(
      { type: 'bitfun:event', event: 'themeChange', payload },
      '*',
    );
  }, [currentTheme]);
}
