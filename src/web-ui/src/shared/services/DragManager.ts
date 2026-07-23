/**
 * Drag-and-drop manager.
 *
 * Centralizes drag state and dispatches drag lifecycle events to registered
 * drop targets.
 */
import { ContextItem } from '../types/context';
import { 
  DragPayload, 
  IDragSource, 
  IDropTarget, 
  DragEventPayload,
  BITFUN_CONTEXT_MIME_TYPE 
} from '../types/drag';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('DragManager');

type DragEventListener = (event: DragEventPayload) => void;

export class DragManager {
  private static instance: DragManager;
  
  private currentPayload: DragPayload<ContextItem> | null = null;
  private currentSource: IDragSource | null = null;
  private listeners: Set<DragEventListener> = new Set();
  private registeredTargets = new Map<string, IDropTarget>();
  
  private constructor() {}
  
  /**
   * Returns the singleton instance.
   */
  static getInstance(): DragManager {
    if (!this.instance) {
      this.instance = new DragManager();
    }
    return this.instance;
  }
  
   
  startDrag(source: IDragSource, payload: DragPayload<ContextItem>, event: DragEvent): void {
    this.currentPayload = payload;
    this.currentSource = source;
    
    
    if (event.dataTransfer) {
      event.dataTransfer.setData(BITFUN_CONTEXT_MIME_TYPE, JSON.stringify(payload));
      event.dataTransfer.setData('text/plain', this.getPlainText(payload));
      event.dataTransfer.effectAllowed = 'copy';
    }
    
    
    source.onDragStart?.(payload);
    
    
    this.emit({
      type: 'dragstart',
      payload,
      event
    });
  }
  
   
  endDrag(event: DragEvent, success: boolean = false): void {
    if (!this.currentPayload || !this.currentSource) return;
    
    
    this.currentSource.onDragEnd?.(this.currentPayload, success);
    
    
    this.emit({
      type: 'dragend',
      payload: this.currentPayload,
      event
    });
    
    
    this.currentPayload = null;
    this.currentSource = null;
  }
  
   
  handleDrop(target: IDropTarget, event: DragEvent): void {
    event.preventDefault();
    event.stopPropagation();
    
    
    const payload = this.currentPayload || this.getPayloadFromEvent(event);
    
    if (!payload) {
      log.warn('No payload found in drop event');
      return;
    }
    
    
    if (!target.canAccept(payload)) {
      log.warn('Target cannot accept payload', { targetId: target.targetId, dataType: payload.dataType });
      this.endDrag(event, false);
      return;
    }
    
    
    this.emit({
      type: 'drop',
      payload,
      target,
      event
    });
    
    
    Promise.resolve(target.onDrop(payload))
      .then(() => {
        this.endDrag(event, true);
      })
      .catch(error => {
        log.error('Drop failed', error);
        this.endDrag(event, false);
      });
  }
  
   
  handleDragEnter(target: IDropTarget, event: DragEvent): void {
    event.preventDefault();
    event.stopPropagation();
    
    
    if (!this.currentPayload) {
      return;
    }
    
    const payload = this.currentPayload;
    
    
    this.emit({
      type: 'dragenter',
      payload,
      target,
      event
    });
    
    target.onDragEnter?.(payload);
  }
  
   
  handleDragLeave(target: IDropTarget, event: DragEvent): void {
    
    if (this.currentPayload) {
      this.emit({
        type: 'dragleave',
        payload: this.currentPayload,
        target,
        event
      });
    }
    
    target.onDragLeave?.();
  }
  
   
  handleDragOver(target: IDropTarget, event: DragEvent): void {
    event.preventDefault();
    event.stopPropagation();
    
    
    if (!this.currentPayload) {
      if (event.dataTransfer) {
        event.dataTransfer.dropEffect = 'none';
      }
      return;
    }
    
    const payload = this.currentPayload;
    
    
    const canAccept = target.canAccept(payload);
    
    if (canAccept) {
      if (event.dataTransfer) {
        event.dataTransfer.dropEffect = 'copy';
      }
    } else {
      if (event.dataTransfer) {
        event.dataTransfer.dropEffect = 'none';
      }
    }
    
    target.onDragOver?.(event);
  }
  
   
  registerTarget(target: IDropTarget): () => void {
    this.registeredTargets.set(target.targetId, target);
    
    return () => {
      this.registeredTargets.delete(target.targetId);
    };
  }
  
   
  subscribe(listener: DragEventListener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
  
   
  getCurrentPayload(): DragPayload<ContextItem> | null {
    return this.currentPayload;
  }
  
   
  private getPayloadFromEvent(event: DragEvent): DragPayload<ContextItem> | null {
    if (!event.dataTransfer) {
      return null;
    }
    
    try {
      const data = event.dataTransfer.getData(BITFUN_CONTEXT_MIME_TYPE);
      
      if (!data) return null;
      
      const parsed = JSON.parse(data) as DragPayload<ContextItem>;
      return parsed;
    } catch (error) {
      log.error('Failed to parse payload from event', error);
      return null;
    }
  }
  
   
  private getPlainText(payload: DragPayload<ContextItem>): string {
    const context = payload.data;
    
    switch (context.type) {
      case 'file':
        return context.filePath;
      case 'directory':
        return context.directoryPath;
      case 'code-snippet':
        return context.selectedText;
      case 'pull-request':
        return context.content;
      case 'mermaid-node':
        return context.nodeText;
      case 'image':
        return context.imagePath;
      case 'terminal-command':
        return context.command;
      case 'git-ref':
        return context.refValue;
      case 'url':
        return context.url;
      default:
        return '';
    }
  }
  
   
  private emit(event: DragEventPayload): void {
    this.listeners.forEach(listener => {
      try {
        listener(event);
      } catch (error) {
        log.error('Error in listener', error);
      }
    });
  }
}


export const dragManager = DragManager.getInstance();
