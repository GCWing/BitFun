 

import React, { useCallback, useState, useRef, useEffect } from 'react';
import { dragManager } from '../../services/DragManager';
import { contextRegistry } from '../../services/ContextRegistry';
import { useContextStore } from '../../stores/contextStore';
import type { IDropTarget } from '../../types/drag';
import type { DragPayload } from '../../types/drag';
import type { ContextItem, ContextType } from '../../types/context';
import './ContextDropZone.scss';
export interface ContextDropZoneProps {
  acceptedTypes?: ContextType[];
  children?: React.ReactNode;
  className?: string;
  onContextAdded?: (context: ContextItem) => void;
  onExternalFilesDrop?: (files: File[]) => void;
  disabled?: boolean;
}

export const ContextDropZone: React.FC<ContextDropZoneProps> = ({
  acceptedTypes,
  children,
  className = '',
  onContextAdded,
  onExternalFilesDrop,
  disabled = false,
}) => {
  const [isDragOver, setIsDragOver] = useState(false);
  const [canAccept, setCanAccept] = useState(false);
  const dropZoneRef = useRef<HTMLDivElement>(null);
  const dragCounterRef = useRef(0); 
  const addContext = useContextStore(state => state.addContext);
  const updateValidation = useContextStore(state => state.updateValidation);
  
  
  const acceptedTypesArray = React.useMemo(() => 
    acceptedTypes || contextRegistry.getAllTypes(), 
    [acceptedTypes]
  );
  
  
  const dropTarget = React.useMemo<IDropTarget>(() => ({
    targetId: 'context-drop-zone',
    acceptedTypes: acceptedTypesArray,
    
    canAccept: (payload: DragPayload<ContextItem>) => {
      return !disabled && acceptedTypesArray.includes(payload.dataType);
    },
    
    onDrop: async (payload: DragPayload<ContextItem>) => {
      if (disabled) return;
      const context = payload.data;
      
      
      addContext(context);
      
      
      
      updateValidation(context.id, { valid: true });
      
      
      onContextAdded?.(context);
      
      
      setIsDragOver(false);
      setCanAccept(false);
    },
    
    onDragEnter: (payload: DragPayload<ContextItem>) => {
      setIsDragOver(true);
      const accepted = dropTarget.canAccept(payload);
      setCanAccept(accepted);
    },
    
    onDragLeave: () => {
      setIsDragOver(false);
      setCanAccept(false);
    },
    
    onDragOver: () => {
      
    }
  }), [acceptedTypesArray, addContext, disabled, updateValidation, onContextAdded]);
  
  
  const dropTargetRef = useRef(dropTarget);
  
  
  useEffect(() => {
    dropTargetRef.current = dropTarget;
  }, [dropTarget]);
  
  
  React.useEffect(() => {
    const unregister = dragManager.registerTarget(dropTarget);
    
    return () => {
      unregister();
    };
  }, [dropTarget]);
  
   
  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    dragCounterRef.current++;
    if (Array.from(e.dataTransfer.types).includes('Files')) {
      setIsDragOver(true);
      setCanAccept(!disabled && Boolean(onExternalFilesDrop));
      return;
    }
    
    if (dragCounterRef.current === 1) {
      
      const payload = dragManager.getCurrentPayload();
      if (payload) {
        const accepted = dropTargetRef.current.canAccept(payload);
        setIsDragOver(true);
        setCanAccept(accepted);
        dragManager.handleDragEnter(dropTargetRef.current, e.nativeEvent);
      }
    }
  }, [disabled, onExternalFilesDrop]);
  
  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    if (Array.from(e.dataTransfer.types).includes('Files')) {
      e.dataTransfer.dropEffect = disabled || !onExternalFilesDrop ? 'none' : 'copy';
      return;
    }
    
    const payload = dragManager.getCurrentPayload();
    if (payload && dropTargetRef.current.canAccept(payload)) {
      e.dataTransfer.dropEffect = 'copy';
      dragManager.handleDragOver(dropTargetRef.current, e.nativeEvent);
    } else {
      e.dataTransfer.dropEffect = 'none';
    }
  }, [disabled, onExternalFilesDrop]);
  
  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const containsExternalFiles = Array.from(e.dataTransfer.types).includes('Files');
    
    dragCounterRef.current--;
    
    if (dragCounterRef.current === 0) {
      
      setIsDragOver(false);
      setCanAccept(false);
      if (!containsExternalFiles) {
        dragManager.handleDragLeave(dropTargetRef.current, e.nativeEvent);
      }
    }
  }, []);
  
  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    
    
    dragCounterRef.current = 0;
    setIsDragOver(false);
    setCanAccept(false);

    if (Array.from(e.dataTransfer.types).includes('Files')) {
      if (!disabled && onExternalFilesDrop) {
        onExternalFilesDrop(Array.from(e.dataTransfer.files));
      }
      return;
    }
    dragManager.handleDrop(dropTargetRef.current, e.nativeEvent);
  }, [disabled, onExternalFilesDrop]);
  
  return (
    <div
      ref={dropZoneRef}
      className={`
        openbitfun-context-drop-zone
        ${isDragOver ? 'openbitfun-context-drop-zone--drag-over' : ''}
        ${canAccept ? 'openbitfun-context-drop-zone--can-accept' : ''}
        ${!canAccept && isDragOver ? 'openbitfun-context-drop-zone--cannot-accept' : ''}
        ${className}
      `.trim()}
      
      onDragEnter={handleDragEnter}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      data-dropzone="context-drop-zone"
      data-openbitfun-component="context-list"
      data-openbitfun-part="dropZone"
      data-openbitfun-state={`${isDragOver ? 'drag-over ' : ''}${canAccept ? 'can-accept' : ''}`.trim() || undefined}
    >
      {children}
    </div>
  );
};

export default ContextDropZone;
