/**
 * Editor extensions - pluggable editor extensions
 */

export {
  ExtensionPriority,
  type EditorExtension,
  type EditorExtensionContext,
  type LspExtensionConfig,
  type AiCompletionExtensionConfig,
  type TabCompletionExtensionConfig,
  type ExtensionState,
  type ExtensionLifecycleEvent,
  type DecorationsExtension,
  type CommandsExtension,
} from './types';

export { createLspExtension, lspExtension } from './LspExtension';
