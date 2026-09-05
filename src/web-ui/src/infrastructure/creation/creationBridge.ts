import { api } from '@/infrastructure/api/service-api/ApiClient';
import { isPeerDeviceModeActive } from '@/infrastructure/peer-device/peerModeFlag';
import { createLogger } from '@/shared/utils/logger';
import type { CreationValue } from './creationCapabilities';

export const CREATION_REQUEST_EVENT = 'agentic://creation-runtime-request';
const log = createLogger('CreationBridge');
interface Runtime {
  inspect: () => unknown;
  invoke: (id: string, args?: Record<string, CreationValue>) => Promise<unknown>;
}
interface Request { requestId: string; action: string; commandId?: string; arguments?: Record<string, CreationValue> }
let active: Runtime | undefined;
let activationError: string | undefined;

export function recordCreationActivationError(error: unknown): void {
  activationError = error == null ? undefined : (error instanceof Error ? error.message : String(error));
}

export function attachCreationRuntime(runtime: Runtime): () => void {
  active = runtime;
  activationError = undefined;
  return () => { if (active === runtime) active = undefined; };
}

export async function executeCreationRequest(request: Request): Promise<unknown> {
  if (isPeerDeviceModeActive()) throw new Error('Creation runtime requires the visible local Desktop');
  if (!active) throw new Error(activationError
    ? `Creation runtime activation failed: ${activationError}`
    : 'Creation runtime is not active; the shell may still be loading or customization activation failed');
  if (request.action === 'inspect') return active.inspect();
  if (request.action === 'invoke' && request.commandId) return active.invoke(request.commandId, request.arguments);
  throw new Error('Unsupported Creation runtime request');
}

/** The existing product transport owns delivery and request/response correlation. */
export function listenForCreationRequests(): () => void {
  return api.listen<Request>(CREATION_REQUEST_EVENT, request => {
    void (async () => {
      let response;
      try {
        response = { requestId: request.requestId, success: true, result: await executeCreationRequest(request) };
      } catch (error) {
        response = { requestId: request.requestId, success: false, error: error instanceof Error ? error.message : String(error) };
      }
      await api.invoke('report_openbitfun_control_result', { request: response });
    })().catch(error => log.warn('Failed to report Creation runtime result', { requestId: request.requestId, error }));
  });
}
