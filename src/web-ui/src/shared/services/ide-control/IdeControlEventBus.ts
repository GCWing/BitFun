/**
 * IDE control event bus.
 *
 * Listens to backend IDE control events and dispatches them to registered controllers.
 */
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { IdeControlEvent, IdeController, IdeControlOperation } from './types';
import { PanelController } from './PanelController';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('IdeControlEventBus');

 
export class IdeControlEventBus {
  private static instance: IdeControlEventBus;
  private controllers: Map<string, IdeController>;
  private initialized: boolean;
  private unlistenFn?: UnlistenFn;

  private constructor() {
    this.controllers = new Map();
    this.initialized = false;
  }

   
  public static getInstance(): IdeControlEventBus {
    if (!IdeControlEventBus.instance) {
      IdeControlEventBus.instance = new IdeControlEventBus();
    }
    return IdeControlEventBus.instance;
  }

   
  async initialize(): Promise<void> {
    if (this.initialized) {
      return;
    }

    
    this.registerController('panel', new PanelController());
    this.registerController('open_panel', new PanelController());
    this.registerController('close_panel', new PanelController());
    this.registerController('toggle_panel', new PanelController());

    
    try {
      this.unlistenFn = await listen<IdeControlEvent>(
        'ide-control-event',
        this.handleEvent.bind(this)
      );

      this.initialized = true;
      log.info('Initialized successfully');
    } catch (error) {
      log.error('Failed to initialize', error);
      throw error;
    }
  }

   
  registerController(operation: string, controller: IdeController): void {
    this.controllers.set(operation, controller);
  }

   
  private async handleEvent(event: { payload: IdeControlEvent }): Promise<void> {
    const { operation, target, options, metadata } = event.payload;

    try {
      
      const controller = this.getController(operation);

      if (controller) {
        await controller.execute(target, options, metadata);
      } else {
        log.warn('No controller found for operation', { operation });
      }
    } catch (error) {
      log.error('Error handling event', error);

      
      if (metadata?.request_id) {
        this.sendErrorResult(metadata.request_id, error);
      }
    }
  }

   
  private getController(operation: IdeControlOperation): IdeController | undefined {
    
    let controller = this.controllers.get(operation);

    
    if (!controller) {
      
      if (
        operation === 'open_panel' ||
        operation === 'close_panel' ||
        operation === 'toggle_panel'
      ) {
        controller = this.controllers.get('panel');
      }
    }

    return controller;
  }

   
  private async sendErrorResult(requestId: string, error: any): Promise<void> {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('report_ide_control_result', {
        request_id: requestId,
        success: false,
        message: undefined,
        error: error instanceof Error ? error.message : String(error),
        timestamp: Date.now(),
      });
    } catch (invokeError) {
      log.error('Failed to send error result', invokeError);
    }
  }

   
  async destroy(): Promise<void> {
    if (this.unlistenFn) {
      this.unlistenFn();
      this.unlistenFn = undefined;
    }

    this.controllers.clear();
    this.initialized = false;
  }

   
  isInitialized(): boolean {
    return this.initialized;
  }
}

 
export function getIdeControlEventBus(): IdeControlEventBus {
  return IdeControlEventBus.getInstance();
}

 
export async function initializeIdeControl(): Promise<void> {
  const eventBus = getIdeControlEventBus();
  await eventBus.initialize();
}
