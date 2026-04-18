/**
 * useLiveAppBridge — handles postMessage JSON-RPC from the Live App iframe:
 * worker.call → JS Worker, dialog.open/save/message → Tauri dialog,
 * ai.* → Host AI client, clipboard.* → Host navigator.clipboard.
 * Also handles bitfun/request-theme and pushes theme changes to the iframe.
 */
import { useLayoutEffect, useRef, useEffect, RefObject } from 'react';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import { open as dialogOpen, save as dialogSave, message as dialogMessage } from '@tauri-apps/plugin-dialog';
import type { MiniApp } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useTheme } from '@/infrastructure/theme/hooks/useTheme';
import { buildLiveAppThemeVars } from '../buildLiveAppThemeVars';
import { api } from '@/infrastructure/api/service-api/ApiClient';

interface JSONRPC {
  jsonrpc?: string;
  id: number | string;
  method: string;
  params?: Record<string, unknown>;
}

interface AiStreamPayload {
  appId: string;
  streamId: string;
  type: 'chunk' | 'done' | 'error';
  data: Record<string, unknown>;
}

export function useLiveAppBridge(
  iframeRef: RefObject<HTMLIFrameElement>,
  app: MiniApp,
) {
  const { workspacePath } = useCurrentWorkspace();
  const { theme: currentTheme } = useTheme();
  const themeRef = useRef(currentTheme);
  themeRef.current = currentTheme;
  const workspacePathRef = useRef(workspacePath);
  workspacePathRef.current = workspacePath;

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
        const payload = buildLiveAppThemeVars(themeRef.current);
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
            workspacePathRef.current || undefined,
          );
          reply(result);
          return;
        }
        if (method === 'dialog.open') {
          reply(await dialogOpen(params as unknown as Parameters<typeof dialogOpen>[0]));
          return;
        }
        if (method === 'dialog.save') {
          reply(await dialogSave(params as unknown as Parameters<typeof dialogSave>[0]));
          return;
        }
        if (method === 'dialog.message') {
          reply(await dialogMessage(params as unknown as Parameters<typeof dialogMessage>[0]));
          return;
        }

        if (method === 'ai.complete') {
          const result = await miniAppAPI.aiComplete(appId, (params.prompt as string) ?? '', {
            systemPrompt: params.systemPrompt as string | undefined,
            model: params.model as string | undefined,
            maxTokens: params.maxTokens as number | undefined,
            temperature: params.temperature as number | undefined,
          });
          reply(result);
          return;
        }
        if (method === 'ai.chat') {
          const result = await miniAppAPI.aiChat(
            appId,
            (params.messages as { role: 'user' | 'assistant'; content: string }[]) ?? [],
            (params.streamId as string) ?? '',
            {
              systemPrompt: params.systemPrompt as string | undefined,
              model: params.model as string | undefined,
              maxTokens: params.maxTokens as number | undefined,
              temperature: params.temperature as number | undefined,
            },
          );
          reply(result);
          return;
        }
        if (method === 'ai.cancel') {
          await miniAppAPI.aiCancel(appId, (params.streamId as string) ?? '');
          reply(null);
          return;
        }
        if (method === 'ai.getModels') {
          const models = await miniAppAPI.aiListModels(appId);
          reply(models);
          return;
        }

        if (method === 'clipboard.writeText') {
          await navigator.clipboard.writeText((params.text as string) ?? '');
          reply(null);
          return;
        }
        if (method === 'clipboard.readText') {
          const text = await navigator.clipboard.readText();
          reply(text);
          return;
        }

        replyError(`Unknown method: ${method}`);
      } catch (error) {
        replyError(typeof error === 'string' ? error : String(error));
      }
    };
    window.addEventListener('message', handler);
    return () => {
      window.removeEventListener('message', handler);
    };
  }, [iframeRef]);

  useEffect(() => {
    const payload = buildLiveAppThemeVars(currentTheme);
    if (!payload || !iframeRef.current?.contentWindow) return;
    iframeRef.current.contentWindow.postMessage(
      { type: 'bitfun:event', event: 'themeChange', payload },
      '*',
    );
  }, [currentTheme, iframeRef]);

  useEffect(() => {
    const currentAppId = app.id;
    const unlisten = api.listen<AiStreamPayload>('miniapp://ai-stream', (payload) => {
      if (!iframeRef.current?.contentWindow) return;
      if (payload.appId !== currentAppId) return;
      iframeRef.current.contentWindow.postMessage(
        {
          type: 'bitfun:event',
          event: 'ai:stream',
          payload: {
            streamId: payload.streamId,
            type: payload.type,
            data: payload.data,
          },
        },
        '*',
      );
    });

    return () => {
      unlisten();
    };
  }, [app.id, iframeRef]);

  useEffect(() => {
    const currentAppId = app.id;
    const eventName = `miniapp://worker-event:${currentAppId}`;
    const unlisten = api.listen<{ appId: string; event: string; data: unknown }>(
      eventName,
      (payload) => {
        if (!iframeRef.current?.contentWindow) return;
        iframeRef.current.contentWindow.postMessage(
          {
            type: 'bitfun:event',
            event: 'worker:event',
            payload: {
              event: payload.event,
              data: payload.data,
            },
          },
          '*',
        );
      },
    );

    return () => {
      unlisten();
    };
  }, [app.id, iframeRef]);
}
