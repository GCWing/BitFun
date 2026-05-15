/**
 * LSP module exports and initialization helpers.
 */

// Services
export { lspService, LspService } from './services/LspService';
export { MonacoLspAdapter, GlobalAdapterRegistry } from './services/MonacoLspAdapter';
export { lspAdapterManager } from './services/LspAdapterManager';
export { WorkspaceLspManager } from './services/WorkspaceLspManager';
export { workspaceLspInitializer } from './services/WorkspaceLspInitializer';
export { LspDiagnostics } from './services/LspDiagnostics';
export { HoverPositionCalculator } from './services/HoverPositionCalculator';
export { lspDocumentService } from './services/LspDocumentService';
export { lspExtensionRegistry } from './services/LspExtensionRegistry';
export type { SupportedExtensionsResponse } from './services/LspExtensionRegistry';
export { lspRefreshManager, LspRefreshManager } from './services/LspRefreshManager';

export type { PositionCalculatorOptions, PositionResult } from './services/HoverPositionCalculator';

// Hooks
export { useLspPlugins, useLspInit } from './hooks/useLsp';
export { useMonacoLsp } from './hooks/useMonacoLsp';

// Components
export { LspPluginList } from './components/LspPluginList/LspPluginList';
export type { LspPluginListProps } from './components/LspPluginList/LspPluginList';

// Types
export type {
  LspPlugin,
  ServerConfig,
  CapabilitiesConfig,
  CompletionItem,
  Position,
  Range,
  Diagnostic,
  HoverInfo,
  Location,
  TextEdit,
} from './types';

export { CompletionItemKind, DiagnosticSeverity } from './types';
export { lspConfigService } from './services/LspConfigService';

// Initialization
export { initializeLsp } from './initializeLsp';
