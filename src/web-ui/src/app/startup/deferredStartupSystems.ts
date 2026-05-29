import {
  backgroundTaskScheduler,
  type BackgroundTaskHandle,
  type BackgroundTaskScheduler,
} from '@/shared/utils/backgroundTaskScheduler';
import { createLogger } from '@/shared/utils/logger';
import { startupTrace } from '@/shared/utils/startupTrace';

const log = createLogger('DeferredStartupSystems');

interface DeferredStartupLog {
  debug: (message: string, data?: unknown) => void;
  warn: (message: string, data?: unknown) => void;
  error: (message: string, data?: unknown) => void;
}

interface DeferredStartupTrace {
  markPhase: (phase: string, data?: Record<string, unknown>) => void;
}

export interface DeferredStartupSystemsDependencies {
  scheduler?: Pick<BackgroundTaskScheduler, 'schedule'>;
  log?: DeferredStartupLog;
  trace?: DeferredStartupTrace;
  initializeIdeControl?: () => Promise<void>;
  initializeMcpServers?: () => Promise<void>;
  initializeAcpClients?: () => Promise<void>;
  probeAcpClientRequirements?: () => Promise<void>;
}

async function initializeIdeControlDefault(): Promise<void> {
  const { initializeIdeControl } = await import('@/shared/services/ide-control');
  await initializeIdeControl();
}

async function initializeMcpServersDefault(): Promise<void> {
  const { MCPAPI } = await import('@/infrastructure/api/service-api/MCPAPI');
  await MCPAPI.initializeServers();
}

async function initializeAcpClientsDefault(): Promise<void> {
  const { ACPClientAPI } = await import('@/infrastructure/api/service-api/ACPClientAPI');
  await ACPClientAPI.initializeClients();
}

async function probeAcpClientRequirementsDefault(): Promise<void> {
  const { ACPClientAPI } = await import('@/infrastructure/api/service-api/ACPClientAPI');
  await ACPClientAPI.probeClientRequirements();
}

export function scheduleDeferredStartupSystems(
  dependencies: DeferredStartupSystemsDependencies = {}
): BackgroundTaskHandle<void> {
  const scheduler = dependencies.scheduler ?? backgroundTaskScheduler;
  const logger = dependencies.log ?? log;
  const trace = dependencies.trace ?? startupTrace;
  const initializeIdeControl = dependencies.initializeIdeControl ?? initializeIdeControlDefault;
  const initializeMcpServers = dependencies.initializeMcpServers ?? initializeMcpServersDefault;
  const initializeAcpClients = dependencies.initializeAcpClients ?? initializeAcpClientsDefault;
  const probeAcpClientRequirements =
    dependencies.probeAcpClientRequirements ?? probeAcpClientRequirementsDefault;

  return scheduler.schedule(async signal => {
    if (signal.aborted) {
      return;
    }

    trace.markPhase('deferred_startup_systems_start');

    const runStep = async (name: string, step: () => Promise<void>) => {
      if (signal.aborted) {
        return;
      }
      try {
        await step();
        logger.debug('Deferred startup system initialized', { system: name });
      } catch (error) {
        logger.error('Deferred startup system failed', { system: name, error });
      }
    };

    await runStep('ide_control', initializeIdeControl);
    await runStep('mcp_servers', initializeMcpServers);
    await runStep('acp_clients', initializeAcpClients);
    await runStep('acp_client_requirements', probeAcpClientRequirements);

    if (!signal.aborted) {
      trace.markPhase('deferred_startup_systems_end');
    }
  }, {
    idle: true,
    priority: 'low',
    inFlightKey: 'startup:deferred-systems',
  });
}
