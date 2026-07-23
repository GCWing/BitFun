 

import { ICommand, CommandId, CommandResult } from '../types/command.types';
import { MenuContext } from '../types/context.types';

 
export abstract class BaseCommand implements ICommand {
  readonly id: CommandId;
  readonly label: string;
  readonly description?: string;
  readonly icon?: string;
  readonly shortcut?: string;
  readonly category?: string;

  constructor(config: {
    id: CommandId;
    label: string;
    description?: string;
    icon?: string;
    shortcut?: string;
    category?: string;
  }) {
    this.id = config.id;
    this.label = config.label;
    this.description = config.description;
    this.icon = config.icon;
    this.shortcut = config.shortcut;
    this.category = config.category;
  }

   
  canExecute(_context: MenuContext): boolean | Promise<boolean> {
    return true;
  }

   
  abstract execute(context: MenuContext): CommandResult | Promise<CommandResult>;

   
  undo?(context: MenuContext): CommandResult | Promise<CommandResult>;

   
  protected success(message?: string, data?: any): CommandResult {
    return {
      success: true,
      message,
      data
    };
  }

   
  protected failure(message: string, error?: Error): CommandResult {
    return {
      success: false,
      message,
      error
    };
  }
}

