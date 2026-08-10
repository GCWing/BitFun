/**
 * Static driver registry. All drivers are registered at module load — no
 * dynamic registration, no ordering hazard, and resolution stays synchronous
 * so hot paths can use it.
 */

import type { SessionConfig } from '../services/flow-chat-manager/types';
import {
  resolveSessionDriverId,
  resolveSessionDriverIdForCreation,
  type DriverResolvableSession,
  type SessionDriverId,
} from './resolve';
import type { SessionDriver } from './types';
import { localSessionDriver } from './local/LocalSessionDriver';
import { dispatchSessionDriver } from './dispatch/DispatchSessionDriver';

const drivers: Record<SessionDriverId, SessionDriver> = {
  local: localSessionDriver,
  dispatch: dispatchSessionDriver,
};

export function sessionDriverById(id: SessionDriverId): SessionDriver {
  return drivers[id];
}

export function driverForSession(
  sessionId: string,
  session: DriverResolvableSession | undefined,
): SessionDriver {
  return drivers[resolveSessionDriverId(sessionId, session)];
}

export function driverForCreation(
  config: Pick<SessionConfig, 'dispatchTargetRequest'>,
): SessionDriver {
  return drivers[resolveSessionDriverIdForCreation(config)];
}
