/**
 * Drag-and-drop protocol types.
 *
 * This module defines the shared payload shape used by drag sources and drop targets.
 * The payload is strongly typed around `ContextItem` so features can exchange data
 * consistently across the app.
 */
import { ContextItem, ContextType } from './context';

export type DragSourceType =
  | 'file-tree'
  | 'code-editor'
  | 'mermaid-diagram'
  | 'terminal'
  | 'image-viewer'
  | 'search-result'
  | 'custom';

/**
 * Canonical drag payload passed between sources and targets.
 *
 * `dataType` should match the `ContextItem['type']` discriminator so targets can
 * validate compatibility without inspecting `data`.
 */
export interface DragPayload<T = unknown> {
  id: string;                    
  sourceType: DragSourceType;    
  dataType: ContextType;         
  timestamp: number;

  data: T;                       

  metadata?: {
    sourceId?: string;           
    sourcePath?: string[];       
    preview?: PreviewData;       
    constraints?: DragConstraints; 
  };
}

/**
 * Optional transfer constraints for consumers to enforce.
 */
export interface DragConstraints {
  maxSize?: number;              
  allowedTargets?: string[];     
  requiresConfirmation?: boolean;
  expiresAt?: number;           
}

/**
 * Lightweight preview data for UI rendering during drag operations.
 */
export interface PreviewData {
  type: 'text' | 'image' | 'custom';
  title: string;
  subtitle?: string;
  icon?: string;
  thumbnail?: string;            
  customHTML?: string;           
}

export interface IDragSource<T = unknown> {
  readonly sourceId: string;
  readonly sourceType: DragSourceType;

  createPayload(data: T): DragPayload<ContextItem>;

  generatePreview?(data: T): PreviewData;

  onDragStart?(payload: DragPayload<ContextItem>): void;

  onDragEnd?(payload: DragPayload<ContextItem>, success: boolean): void;
}

export interface IDropTarget {
  readonly targetId: string;
  readonly acceptedTypes: ContextType[];

  canAccept(payload: DragPayload<ContextItem>): boolean;

  onDrop(payload: DragPayload<ContextItem>): void | Promise<void>;

  
  onDragEnter?(payload: DragPayload<ContextItem>): void;

  
  onDragLeave?(): void;

  
  onDragOver?(event: DragEvent): void;
}

export interface DragEventPayload {
  type: 'dragstart' | 'dragend' | 'dragenter' | 'dragleave' | 'dragover' | 'drop';
  payload: DragPayload<ContextItem>;
  target?: IDropTarget;
  event?: DragEvent;
}

/**
 * Custom MIME type for transferring BitFun context payloads via the browser DnD API.
 */
export const BITFUN_CONTEXT_MIME_TYPE = 'application/x-bitfun-context';
export const BITFUN_CONTEXT_JSON_MIME_TYPE = 'application/json';
