import { useEffect, useState } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import {
  ACPClientAPI,
  type AcpAvailableCommand,
  type AcpAvailableCommandsUpdatedEvent,
} from '@/infrastructure/api/service-api/ACPClientAPI';
import type { AcpSessionRef } from '../utils/acpSession';

const AVAILABLE_COMMANDS_EVENT = 'agentic://acp-available-commands-updated';

/**
 * Filter advertised slash commands by a query (the text the user has typed
 * after `/`). A leading slash is tolerated. Matching is case-insensitive over
 * both the command name and its description. An empty query returns all.
 */
export function filterSlashCommands(
  commands: AcpAvailableCommand[],
  query: string,
): AcpAvailableCommand[] {
  const q = query.trim().toLowerCase().replace(/^\//, '');
  if (!q) return commands;
  return commands.filter(
    (command) =>
      command.name.toLowerCase().includes(q) ||
      command.description.toLowerCase().includes(q),
  );
}

/**
 * Track the slash commands an ACP agent advertises for the given session.
 *
 * Fetches the current list on mount / session change and then keeps it live by
 * subscribing to `agentic://acp-available-commands-updated`. Returns an empty
 * list for non-ACP sessions or before the agent has advertised any commands.
 */
export function useAcpSlashCommands(
  acpSession: AcpSessionRef | null,
): { commands: AcpAvailableCommand[] } {
  const [commands, setCommands] = useState<AcpAvailableCommand[]>([]);

  const sessionId = acpSession?.sessionId ?? null;
  const clientId = acpSession?.clientId ?? null;
  const workspacePath = acpSession?.workspacePath;
  const remoteConnectionId = acpSession?.remoteConnectionId;
  const remoteSshHost = acpSession?.remoteSshHost;

  // Reset immediately when the session changes so a stale list never leaks
  // across sessions.
  useEffect(() => {
    setCommands([]);
  }, [sessionId]);

  // Initial fetch of whatever the agent has already advertised.
  useEffect(() => {
    if (!sessionId || !clientId) return;
    let cancelled = false;
    ACPClientAPI.getSessionCommands({
      sessionId,
      clientId,
      workspacePath,
      remoteConnectionId,
      remoteSshHost,
    })
      .then((list) => {
        if (!cancelled) setCommands(list);
      })
      .catch(() => {
        /* commands stay empty; the live event may still populate them */
      });
    return () => {
      cancelled = true;
    };
  }, [sessionId, clientId, workspacePath, remoteConnectionId, remoteSshHost]);

  // Live updates while a turn streams (the agent can change its command set).
  useEffect(() => {
    if (!sessionId) return;
    let active = true;
    let unlisten: UnlistenFn | undefined;
    listen<AcpAvailableCommandsUpdatedEvent>(AVAILABLE_COMMANDS_EVENT, (event) => {
      if (event.payload.sessionId === sessionId) {
        setCommands(event.payload.commands);
      }
    }).then((fn) => {
      if (active) unlisten = fn;
      else fn();
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [sessionId]);

  return { commands };
}
