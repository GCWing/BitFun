 

import { CommandRegistry } from './CommandRegistry';
import { useCommandHistoryStore } from '../store/CommandHistoryStore';
import {
  ICommand,
  CommandId,
  CommandResult,
  CommandExecutionContext,
  CommandInterceptor,
  CommandConfig
} from '../types/command.types';
import { MenuContext } from '../types/context.types';
import { globalEventBus } from '../../../infrastructure/event-bus';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('CommandExecutor');

 
export class CommandExecutor {
  private registry: CommandRegistry;
  private interceptors: CommandInterceptor[] = [];
  private config: CommandConfig;

  constructor(registry: CommandRegistry, config: CommandConfig = {}) {
    this.registry = registry;
    this.config = {
      enableHistory: true,
      maxHistorySize: 50,
      enableUndo: true,
      timeout: 30000,
      debug: false,
      ...config
    };
  }

   
  async execute(
    commandId: CommandId,
    context: MenuContext,
    params?: Record<string, any>
  ): Promise<CommandResult> {
    const command = this.registry.getCommand(commandId);

    if (!command) {
      const error = new Error(`Command "${commandId}" not found`);
      return this.createFailureResult(`Command not found: ${commandId}`, error);
    }

    const executionContext: CommandExecutionContext = {
      ...context,
      commandId,
      params
    };

    if (this.config.debug) {
      log.debug('Executing command', { commandId, context: executionContext });
    }

    
    globalEventBus.emit('command:before-execute', { commandId, context: executionContext });

    try {
      
      const canProceed = await this.runBeforeInterceptors(executionContext);
      if (!canProceed) {
        return this.createFailureResult('Command execution was intercepted');
      }

      
      const canExecute = await command.canExecute(context);
      if (!canExecute) {
        return this.createFailureResult('Command cannot be executed in current context');
      }

      
      const result = await this.executeWithTimeout(command, context);

      
      await this.runAfterInterceptors(executionContext, result);

      
      if (this.config.enableHistory && result.success) {
        this.recordHistory(executionContext, result, !!command.undo);
      }

      
      if (result.success) {
        globalEventBus.emit('command:success', { commandId, context: executionContext, result });
      } else {
        globalEventBus.emit('command:failure', { commandId, context: executionContext, result });
      }

      if (this.config.debug) {
        log.debug('Command result', { commandId, result });
      }

      return result;

    } catch (error) {
      const err = error as Error;
      log.error('Command execution failed', err);

      
      await this.runErrorInterceptors(executionContext, err);

      
      globalEventBus.emit('command:error', { commandId, context: executionContext, error: err });

      return this.createFailureResult(`Command execution failed: ${err.message}`, err);
    }
  }

   
  async undo(): Promise<CommandResult> {
    const historyStore = useCommandHistoryStore.getState();
    
    if (!historyStore.canUndo()) {
      return this.createFailureResult('No command to undo');
    }

    const record = historyStore.undo();
    if (!record) {
      return this.createFailureResult('Failed to get undo record');
    }

    const command = this.registry.getCommand(record.commandId);
    if (!command || !command.undo) {
      return this.createFailureResult('Command does not support undo');
    }

    try {
      const result = await command.undo(record.context);
      
      if (result.success) {
        globalEventBus.emit('command:undo', { commandId: record.commandId, context: record.context });
      }

      return result;
    } catch (error) {
      const err = error as Error;
      log.error('Undo failed', err);
      return this.createFailureResult(`Undo failed: ${err.message}`, err);
    }
  }

   
  async redo(): Promise<CommandResult> {
    const historyStore = useCommandHistoryStore.getState();
    
    if (!historyStore.canRedo()) {
      return this.createFailureResult('No command to redo');
    }

    const record = historyStore.redo();
    if (!record) {
      return this.createFailureResult('Failed to get redo record');
    }

    return await this.execute(record.commandId, record.context, record.context.params);
  }

   
  addInterceptor(interceptor: CommandInterceptor): void {
    this.interceptors.push(interceptor);
  }

   
  removeInterceptor(name: string): boolean {
    const index = this.interceptors.findIndex(i => i.name === name);
    if (index > -1) {
      this.interceptors.splice(index, 1);
      return true;
    }
    return false;
  }

   
  private async runBeforeInterceptors(context: CommandExecutionContext): Promise<boolean> {
    for (const interceptor of this.interceptors) {
      if (interceptor.before) {
        try {
          const result = await interceptor.before(context);
          if (result === false) {
            return false;
          }
        } catch (error) {
          log.error('Interceptor before hook failed', { interceptor: interceptor.name, error });
          return false;
        }
      }
    }
    return true;
  }

   
  private async runAfterInterceptors(
    context: CommandExecutionContext,
    result: CommandResult
  ): Promise<void> {
    for (const interceptor of this.interceptors) {
      if (interceptor.after) {
        try {
          await interceptor.after(context, result);
        } catch (error) {
          log.error('Interceptor after hook failed', { interceptor: interceptor.name, error });
        }
      }
    }
  }

   
  private async runErrorInterceptors(
    context: CommandExecutionContext,
    error: Error
  ): Promise<void> {
    for (const interceptor of this.interceptors) {
      if (interceptor.error) {
        try {
          await interceptor.error(context, error);
        } catch (err) {
          log.error('Interceptor error handler failed', { interceptor: interceptor.name, error: err });
        }
      }
    }
  }

   
  private async executeWithTimeout(
    command: ICommand,
    context: MenuContext
  ): Promise<CommandResult> {
    if (!this.config.timeout) {
      return await command.execute(context);
    }

    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        reject(new Error(`Command execution timeout after ${this.config.timeout}ms`));
      }, this.config.timeout);

      Promise.resolve(command.execute(context))
        .then((result: CommandResult) => {
          clearTimeout(timer);
          resolve(result);
        })
        .catch((error: unknown) => {
          clearTimeout(timer);
          reject(error);
        });
    });
  }

   
  private recordHistory(
    context: CommandExecutionContext,
    result: CommandResult,
    canUndo: boolean
  ): void {
    const historyStore = useCommandHistoryStore.getState();
    
    historyStore.addHistory({
      commandId: context.commandId,
      context,
      result,
      timestamp: Date.now(),
      canUndo
    });
  }

   
  private createFailureResult(message: string, error?: Error): CommandResult {
    return {
      success: false,
      message,
      error
    };
  }

   
  getConfig(): CommandConfig {
    return { ...this.config };
  }

   
  setConfig(config: Partial<CommandConfig>): void {
    this.config = { ...this.config, ...config };
  }
}

 
import { commandRegistry as registryInstance } from './CommandRegistry';
export const commandExecutor = new CommandExecutor(registryInstance);
