 
/**
 * Context menu system.
 *
 * Exposes the primitives for building and rendering context menus, registering commands,
 * and wiring feature-specific context into menus.
 */
export * from './types';
export * from './core';
export * from './commands';
export * from './providers';
export { useContextMenuStore, selectMenuVisible, selectMenuItems, selectMenuPosition, selectMenuContext } from './store/ContextMenuStore';
export { useCommandHistoryStore } from './store/CommandHistoryStore';
export * from './utils';
export * from './config';
export * from './init';
export * from './components';


export { contextMenuManager, getContextMenuManager } from './core/ContextMenuManager';
export { contextMenuController } from './core/ContextMenuController';
export { contextMenuRegistry } from './core/ContextMenuRegistry';
export { contextResolver } from './core/ContextResolver';
export { commandRegistry } from './commands/CommandRegistry';
export { commandExecutor } from './commands/CommandExecutor';
export { menuBuilder } from './core/MenuBuilder';
