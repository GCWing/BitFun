/**
 * Context menu command types.
 *
 * Commands are executable units that operate on a `MenuContext` and can optionally
 * support undo/history.
 */
import { MenuContext } from './context.types';

export type CommandId = string;

/**
 * Result returned by a command execution.
 */
export interface CommandResult {
   
  success: boolean;
   
  message?: string;
   
  data?: any;
   
  error?: Error;
}

/**
 * Executable command interface.
 */
export interface ICommand {
   
  readonly id: CommandId;
   
  readonly label: string;
   
  readonly description?: string;
   
  readonly icon?: string;
   
  readonly shortcut?: string;
   
  readonly category?: string;

   
  canExecute(context: MenuContext): boolean | Promise<boolean>;

   
  execute(context: MenuContext): CommandResult | Promise<CommandResult>;

   
  undo?(context: MenuContext): CommandResult | Promise<CommandResult>;
}

 
export interface CommandMetadata {
   
  id: CommandId;
   
  label: string;
   
  description?: string;
   
  icon?: string;
   
  shortcut?: string;
   
  category?: string;
   
  builtin?: boolean;
   
  createdAt?: number;
}

 
export type CommandExecutionContext = MenuContext & {
   
  commandId: CommandId;
   
  params?: Record<string, any>;
};

 
export interface CommandHistory {
   
  commandId: CommandId;
   
  context: CommandExecutionContext;
   
  result: CommandResult;
   
  timestamp: number;
   
  canUndo: boolean;
}

 
export interface CommandConfig {
   
  enableHistory?: boolean;
   
  maxHistorySize?: number;
   
  enableUndo?: boolean;
   
  timeout?: number;
   
  debug?: boolean;
}

 
export interface CommandRegistrationOptions {
   
  override?: boolean;
   
  priority?: number;
}

 
export interface CommandInterceptor {
   
  name: string;
   
  before?: (context: CommandExecutionContext) => boolean | Promise<boolean>;
   
  after?: (context: CommandExecutionContext, result: CommandResult) => void | Promise<void>;
   
  error?: (context: CommandExecutionContext, error: Error) => void | Promise<void>;
}
