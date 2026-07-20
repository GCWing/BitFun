/**
 * Peer-side bridge: execute HostInvoke requests through the same Tauri invoke
 * path as local UI, then report results back to Rust.
 */

import { useEffect } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { createLogger } from '@/shared/utils/logger';
import { isTauriRuntime } from '@/infrastructure/runtime';

const log = createLogger('PeerHostInvokeBridge');

function serializeInvokeError(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  try {
    return JSON.stringify(error);
  } catch {
    return 'Host operation failed';
  }
}

function safeCommandForLog(command: string): string {
  return /^[a-z0-9_]{1,80}$/i.test(command) ? command : 'invalid';
}

interface HostInvokeBridgeRequest {
  id: string;
  command: string;
  args: unknown;
}

export function PeerHostInvokeBridge(): null {
  useEffect(() => {
    if (!isTauriRuntime()) {
      return;
    }

    let disposed = false;
    let unlisten: UnlistenFn | null = null;

    void (async () => {
      try {
        unlisten = await listen<HostInvokeBridgeRequest>('peer-host-invoke://request', async (event) => {
          if (disposed) return;
          const { id, command, args } = event.payload;
          try {
            const value = args === undefined || args === null
              ? await invoke(command)
              : await invoke(command, args as Record<string, unknown>);
            await invoke('peer_host_invoke_complete', {
              id,
              ok: true,
              value: value ?? null,
              error: null,
            });
          } catch (error) {
            const message = serializeInvokeError(error);
            const loggedCommand = safeCommandForLog(command);
            log.warn('Peer host invoke failed', {
              command: loggedCommand,
              error_category: 'host_invoke',
            });
            try {
              await invoke('peer_host_invoke_complete', {
                id,
                ok: false,
                value: null,
                error: message,
              });
            } catch {
              log.error('Failed to report peer host invoke error', {
                command: loggedCommand,
                error_category: 'completion',
              });
            }
          }
        });
      } catch {
        log.error('Failed to register peer host invoke listener', {
          error_category: 'listener_registration',
        });
      }
    })();

    return () => {
      disposed = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  return null;
}
