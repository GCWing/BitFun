 

import { ICommand, CommandId, CommandMetadata, CommandRegistrationOptions } from '../types/command.types';

 
export class CommandRegistry {
  private commands: Map<CommandId, ICommand>;
  private metadata: Map<CommandId, CommandMetadata>;

  constructor() {
    this.commands = new Map();
    this.metadata = new Map();
  }

   
  register(command: ICommand, options: CommandRegistrationOptions = {}): void {
    
    if (this.commands.has(command.id) && !options.override) {
      throw new Error(`Command with id "${command.id}" already exists`);
    }

    
    this.commands.set(command.id, command);

    
    this.metadata.set(command.id, {
      id: command.id,
      label: command.label,
      description: command.description,
      icon: command.icon,
      shortcut: command.shortcut,
      category: command.category,
      builtin: false,
      createdAt: Date.now()
    });
  }

   
  registerMany(commands: ICommand[], options: CommandRegistrationOptions = {}): void {
    commands.forEach(command => this.register(command, options));
  }

   
  unregister(commandId: CommandId): boolean {
    const deleted = this.commands.delete(commandId);
    this.metadata.delete(commandId);

    return deleted;
  }

   
  getCommand(commandId: CommandId): ICommand | undefined {
    return this.commands.get(commandId);
  }

   
  getAllCommands(): ICommand[] {
    return Array.from(this.commands.values());
  }

   
  getMetadata(commandId: CommandId): CommandMetadata | undefined {
    return this.metadata.get(commandId);
  }

   
  getAllMetadata(): CommandMetadata[] {
    return Array.from(this.metadata.values());
  }

   
  has(commandId: CommandId): boolean {
    return this.commands.has(commandId);
  }

   
  getCommandsByCategory(category: string): ICommand[] {
    return Array.from(this.commands.values()).filter(
      cmd => cmd.category === category
    );
  }

   
  search(query: string): ICommand[] {
    const lowerQuery = query.toLowerCase();
    return Array.from(this.commands.values()).filter(cmd =>
      cmd.label.toLowerCase().includes(lowerQuery) ||
      cmd.id.toLowerCase().includes(lowerQuery) ||
      cmd.description?.toLowerCase().includes(lowerQuery)
    );
  }

   
  clear(): void {
    this.commands.clear();
    this.metadata.clear();
  }

   
  getStats() {
    const categories = new Map<string, number>();
    
    this.commands.forEach(cmd => {
      if (cmd.category) {
        categories.set(cmd.category, (categories.get(cmd.category) || 0) + 1);
      }
    });

    return {
      totalCommands: this.commands.size,
      categories: Object.fromEntries(categories),
      builtinCommands: Array.from(this.metadata.values()).filter(m => m.builtin).length
    };
  }
}

 
export const commandRegistry = new CommandRegistry();

