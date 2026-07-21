/**
 * useMiniAppBridge — handles postMessage JSON-RPC from the MiniApp iframe:
 * worker.call → JS Worker, dialog.open/save/message → Tauri dialog,
 * ai.* → Host AI client, agent.* → Host agent bridge (hidden subagent runs),
 * deck.renderPage → hidden host WebView slide rasterization (export),
 * clipboard.* → Host navigator.clipboard.
 * Also handles bitfun/request-theme and pushes theme changes to the iframe.
 */
import { useLayoutEffect, useRef, useEffect, RefObject } from 'react';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import { open as dialogOpen, save as dialogSave, message as dialogMessage } from '@tauri-apps/plugin-dialog';
import type { MiniApp } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useTheme } from '@/infrastructure/theme/hooks/useTheme';
import { buildMiniAppThemeVars } from '../utils/buildMiniAppThemeVars';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { useI18n } from '@/infrastructure/i18n';
import type { MiniAppRunScope } from '../customization/miniAppCustomizationTypes';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { workspaceAPI } from '@/infrastructure/api';

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

export function useMiniAppBridge(
  iframeRef: RefObject<HTMLIFrameElement>,
  app: MiniApp,
  runScope: MiniAppRunScope,
) {
  const { workspacePath } = useCurrentWorkspace();
  const { theme: currentTheme } = useTheme();
  const { currentLanguage } = useI18n('scenes/miniapp');
  const themeRef = useRef(currentTheme);
  themeRef.current = currentTheme;
  const workspacePathRef = useRef(workspacePath);
  workspacePathRef.current = workspacePath;
  const localeRef = useRef(currentLanguage);
  localeRef.current = currentLanguage;

  const runScopeRef = useRef<MiniAppRunScope>(runScope);
  runScopeRef.current = runScope;
  // Whether this app opts out of the JS Worker. When true, framework primitive
  // calls (fs.*/shell.*/os.*/net.*) are routed to the host directly via
  // `miniapp_host_call`, so the app does not require Bun/Node at runtime.
  // `storage.*` and any custom user RPC method still go through `worker.call`,
  // but for `node.enabled = false` apps `storage.*` is served by the manager
  // (no worker), and any non-namespaced custom call will fail with a clear error.
  const nodeDisabledRef = useRef(app.permissions?.node?.enabled === false);
  const systemNotificationsAllowedRef = useRef(app.permissions?.notifications?.system === true);
  const agentEnabledRef = useRef(app.permissions?.agent?.enabled === true);
  useLayoutEffect(() => {
    nodeDisabledRef.current = app.permissions?.node?.enabled === false;
    systemNotificationsAllowedRef.current = app.permissions?.notifications?.system === true;
    agentEnabledRef.current = app.permissions?.agent?.enabled === true;
  }, [app.id, app.permissions?.node?.enabled, app.permissions?.notifications?.system, app.permissions?.agent?.enabled]);

  // Hidden agent sessions started by this iframe; used to filter the global
  // agentic:// event stream before forwarding events into the iframe.
  const agentSessionIdsRef = useRef<Set<string>>(new Set());

  useLayoutEffect(() => {
    const handler = async (event: MessageEvent) => {
      if (!iframeRef.current || event.source !== iframeRef.current.contentWindow) return;
      const msg = event.data as JSONRPC & { method?: string };
      if (!msg?.method) return;

      const { id, method, params = {} } = msg;
      const scope = runScopeRef.current;
      const appId = scope.appId;
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

      if (method === 'bitfun/request-locale') {
        // Reply with the current locale id (e.g. "zh-CN" / "en-US"). The MiniApp
        // can use this both as the initial value and to look up its own i18n bundle.
        reply({ locale: localeRef.current });
        if (iframeRef.current?.contentWindow) {
          iframeRef.current.contentWindow.postMessage(
            { type: 'bitfun:event', event: 'localeChange', payload: { locale: localeRef.current } },
            '*',
          );
        }
        return;
      }

      try {
        if (method === 'worker.call') {
          const innerMethod = (params.method as string) ?? '';
          const innerParams = (params.params as Record<string, unknown>) ?? {};
          const ns = innerMethod.split('.')[0];
          const isHostPrimitive = ns === 'fs' || ns === 'shell' || ns === 'os' || ns === 'net';
          const isStorage = ns === 'storage';

          // For node-disabled apps, framework primitives go to the host directly
          // (no Bun/Node Worker required). Storage is served by the manager.
          // For node-enabled apps, keep the legacy path so user `worker.js` exports
          // (including overrides of fs/shell) continue to work.
          if (nodeDisabledRef.current) {
            if (isHostPrimitive) {
              const result = scope.kind === 'draft'
                ? await miniAppAPI.draftHostCall(
                  appId,
                  scope.draftId,
                  innerMethod,
                  innerParams,
                  workspacePathRef.current || undefined,
                )
                : await miniAppAPI.hostCall(
                  appId,
                  innerMethod,
                  innerParams,
                  workspacePathRef.current || undefined,
                );
              reply(result);
              return;
            }
            if (isStorage) {
              const subName = innerMethod.split('.')[1];
              const key = String(innerParams.key ?? '');
              if (subName === 'get') {
                const value = scope.kind === 'draft'
                  ? await miniAppAPI.getDraftStorage(appId, scope.draftId, key)
                  : await api.invoke('get_miniapp_storage', { appId, key });
                reply(value ?? null);
                return;
              }
              if (subName === 'set') {
                if (scope.kind === 'draft') {
                  await miniAppAPI.setDraftStorage(appId, scope.draftId, key, innerParams.value ?? null);
                } else {
                  await api.invoke('set_miniapp_storage', {
                    appId,
                    key,
                    value: innerParams.value ?? null,
                  });
                }
                reply(null);
                return;
              }
              replyError(`Unknown storage method: ${innerMethod}`);
              return;
            }
            // Custom user RPC for an app without a worker — fail loudly so the dev
            // sees what's wrong instead of getting a generic worker-pool error.
            replyError(
              `MiniApp '${appId}' has node.enabled=false; cannot call custom worker method '${innerMethod}'. ` +
                `Either set node.enabled=true and ship a worker.js, or use a host primitive (fs.*/shell.*/os.*/net.*).`,
            );
            return;
          }

          const result = scope.kind === 'draft'
            ? await miniAppAPI.draftWorkerCall(
              appId,
              scope.draftId,
              innerMethod,
              innerParams,
              workspacePathRef.current || undefined,
            )
            : await miniAppAPI.workerCall(
              appId,
              innerMethod,
              innerParams,
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

        // ── AI commands ──────────────────────────────────────────────────────
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

        // ── Agent bridge commands ────────────────────────────────────────────
        if (method.startsWith('agent.')) {
          if (!agentEnabledRef.current) {
            replyError(`MiniApp '${appId}' does not have agent permission (permissions.agent.enabled).`);
            return;
          }
          if (method === 'agent.run') {
            const result = await miniAppAPI.agentRun(
              appId,
              (params.prompt as string) ?? '',
              workspacePathRef.current || undefined,
              {
                runId: params.runId as string | undefined,
                sessionName: params.sessionName as string | undefined,
                enableTools: params.enableTools as boolean | undefined,
                sessionId: params.sessionId as string | undefined,
                appDataWorkspace: params.appDataWorkspace as string | undefined,
                model: typeof params.model === 'string' ? params.model : undefined,
              },
            );
            agentSessionIdsRef.current.add(result.sessionId);
            reply(result);
            return;
          }
          if (method === 'agent.cancel') {
            await miniAppAPI.agentCancel(
              appId,
              (params.sessionId as string) ?? '',
              (params.turnId as string) ?? '',
            );
            reply(null);
            return;
          }
          if (method === 'agent.turnText') {
            const result = await miniAppAPI.agentTurnText(
              appId,
              (params.sessionId as string) ?? '',
              (params.turnId as string) ?? '',
            );
            reply(result);
            return;
          }
          if (method === 'agent.cancelStaleRuns') {
            const result = await miniAppAPI.agentCancelStaleRuns(appId);
            reply(result);
            return;
          }
          replyError(`Unknown agent method: ${method}`);
          return;
        }

        // ── Deck export commands ─────────────────────────────────────────────
        if (method === 'deck.renderPage') {
          const result = await miniAppAPI.renderSlidePage(appId, {
            html: String(params.html ?? ''),
            format: String(params.format ?? 'png'),
            width: params.width as number | undefined,
            height: params.height as number | undefined,
          });
          reply(result);
          return;
        }

        // ── Clipboard commands ───────────────────────────────────────────────
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

        if (method === 'system.openExternal') {
          const url = String(params.url ?? '');
          let parsed: URL;
          try {
            parsed = new URL(url);
          } catch {
            replyError('Invalid URL.');
            return;
          }
          if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') {
            replyError('Only http(s) URLs can be opened.');
            return;
          }
          await systemAPI.openExternal(parsed.toString());
          reply(null);
          return;
        }

        if (method === 'system.revealInFolder') {
          // When `path` is omitted, open the system Downloads folder.
          let targetPath = String(params.path ?? '');
          if (!targetPath) {
            const { downloadDir } = await import('@tauri-apps/api/path');
            targetPath = await downloadDir();
          }
          if (!targetPath) {
            replyError('Could not determine the folder to open.');
            return;
          }
          await workspaceAPI.revealInExplorer(targetPath);
          reply(null);
          return;
        }

        if (method === 'notifications.system') {
          if (!systemNotificationsAllowedRef.current) {
            replyError(`MiniApp '${appId}' does not have notifications.system permission.`);
            return;
          }
          await systemAPI.sendSystemNotification(
            String(params.title ?? ''),
            params.body == null ? undefined : String(params.body),
          );
          reply(null);
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
    const payload = buildMiniAppThemeVars(currentTheme);
    if (!payload || !iframeRef.current?.contentWindow) return;
    iframeRef.current.contentWindow.postMessage(
      { type: 'bitfun:event', event: 'themeChange', payload },
      '*',
    );
  }, [currentTheme, iframeRef]);

  // Push locale changes to the iframe so MiniApps can re-render their UI strings
  // without reloading. MiniApps subscribe via `app.on('localeChange', fn)`.
  useEffect(() => {
    if (!iframeRef.current?.contentWindow) return;
    iframeRef.current.contentWindow.postMessage(
      { type: 'bitfun:event', event: 'localeChange', payload: { locale: currentLanguage } },
      '*',
    );
  }, [currentLanguage, iframeRef]);

  // Listen for AI stream events from Tauri and forward them to the iframe.
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

  // Forward agentic:// events for MiniApp-owned hidden agent sessions into the
  // iframe as 'agent:event' (consumed via app.agent.onEvent in the SDK).
  useEffect(() => {
    if (app.permissions?.agent?.enabled !== true) return;

    const forwardedEvents = [
      'dialog-turn-started',
      'model-round-started',
      'model-round-completed',
      'text-chunk',
      'tool-event',
      'dialog-turn-completed',
      'dialog-turn-failed',
      'dialog-turn-cancelled',
      'token-usage-updated',
      'subagent-session-linked',
    ];

    const unlistenSubagentLink = api.listen<{
      sessionId?: string;
      parentSessionId?: string;
    }>('agentic://subagent-session-linked', (payload) => {
      if (!payload?.sessionId || !payload?.parentSessionId) return;
      if (!agentSessionIdsRef.current.has(payload.parentSessionId)) return;
      agentSessionIdsRef.current.add(payload.sessionId);
    });

    const unlisteners = forwardedEvents.map((eventName) =>
      api.listen<{ sessionId?: string; parentSessionId?: string; [key: string]: unknown }>(
        `agentic://${eventName}`,
        (payload) => {
          if (!iframeRef.current?.contentWindow) return;
          const eventSessionId = payload?.sessionId;
          if (!eventSessionId) return;
          const parentSessionId = payload.parentSessionId;
          const ownsSession =
            agentSessionIdsRef.current.has(eventSessionId)
            || (eventName === 'subagent-session-linked'
              && typeof parentSessionId === 'string'
              && agentSessionIdsRef.current.has(parentSessionId));
          if (!ownsSession) return;
          iframeRef.current.contentWindow.postMessage(
            {
              type: 'bitfun:event',
              event: 'agent:event',
              payload: { sourceEvent: eventName, ...payload },
            },
            '*',
          );
        },
      ),
    );

    return () => {
      unlistenSubagentLink();
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [app.id, app.permissions?.agent?.enabled, iframeRef]);

  // Listen for Worker push events and forward them to the iframe.
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
