 

import { getContextMenuManager } from './core/ContextMenuManager';
import { commandRegistry } from './commands/CommandRegistry';
import { contextMenuRegistry } from './core/ContextMenuRegistry';
import { getBuiltinCommands } from './commands/builtin';
import { getBuiltinProviders } from './providers';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ContextMenuSystem');

 
export interface InitConfig {
   
  registerBuiltinCommands?: boolean;
   
  registerBuiltinProviders?: boolean;
   
  debug?: boolean;
   
  customCommands?: any[];
   
  customProviders?: any[];
}

 
export function initContextMenuSystem(config: InitConfig = {}): void {  
  const {
    registerBuiltinCommands = true,
    registerBuiltinProviders = true,
    debug = false,
    customCommands = [],
    customProviders = []
  } = config;

  let commandCount = 0;
  let providerCount = 0;

  
  if (registerBuiltinCommands) {
    const builtinCommands = getBuiltinCommands();
    builtinCommands.forEach(command => {
      try {
        commandRegistry.register(command);
        commandCount++;
      } catch (error) {
        log.error('Failed to register command', { commandId: command.id, error });
      }
    });
  }

  
    customCommands.forEach(command => {
      try {
        commandRegistry.register(command);
        commandCount++;
      } catch (error) {
        log.error('Failed to register custom command', error as Error);
      }
    });

  
  if (registerBuiltinProviders) {
    const builtinProviders = getBuiltinProviders();
    builtinProviders.forEach(provider => {
      try {
        contextMenuRegistry.register(provider);
        providerCount++;
      } catch (error) {
        log.error('Failed to register provider', { providerId: provider.id, error });
      }
    });
  }

  
    customProviders.forEach(provider => {
      try {
        contextMenuRegistry.register(provider);
        providerCount++;
      } catch (error) {
        log.error('Failed to register custom provider', error);
      }
    });

  
  getContextMenuManager({
    registry: contextMenuRegistry,
    debug: debug
  });

  log.info('Initialization completed', { commandCount, providerCount });
}

 
export function destroyContextMenuSystem(): void {
  const manager = getContextMenuManager();
  if (manager) {
    manager.destroy();
  }
  commandRegistry.clear();
  contextMenuRegistry.clear();
}

