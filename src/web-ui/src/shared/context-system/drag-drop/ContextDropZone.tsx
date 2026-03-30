 

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
}

export const ContextDropZone: React.FC<ContextDropZoneProps> = ({
  acceptedTypes,
  children,
  className = '',
  onContextAdded
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
      return acceptedTypesArray.includes(payload.dataType);
    },
    
    onDrop: async (payload: DragPayload<ContextItem>) => {
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
  }), [acceptedTypesArray, addContext, updateValidation, onContextAdded]);
  
  
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
    
    if (dragCounterRef.current === 1) {
      
      const payload = dragManager.getCurrentPayload();
      if (payload) {
        const accepted = dropTargetRef.current.canAccept(payload);
        setIsDragOver(true);
        setCanAccept(accepted);
        dragManager.handleDragEnter(dropTargetRef.current, e.nativeEvent);
      }
    }
  }, []);
  
  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    
    
    const payload = dragManager.getCurrentPayload();
    if (payload && dropTargetRef.current.canAccept(payload)) {
      e.dataTransfer.dropEffect = 'copy';
      dragManager.handleDragOver(dropTargetRef.current, e.nativeEvent);
    } else {
      e.dataTransfer.dropEffect = 'none';
    }
  }, []);
  
  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    
    dragCounterRef.current--;
    
    if (dragCounterRef.current === 0) {
      
      setIsDragOver(false);
      setCanAccept(false);
      dragManager.handleDragLeave(dropTargetRef.current, e.nativeEvent);
    }
  }, []);
  
  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    
    
    dragCounterRef.current = 0;
    setIsDragOver(false);
    setCanAccept(false);
    
    dragManager.handleDrop(dropTargetRef.current, e.nativeEvent);
  }, []);
  
  return (
    <div
      ref={dropZoneRef}
      className={`
        bitfun-context-drop-zone
        ${isDragOver ? 'bitfun-context-drop-zone--drag-over' : ''}
        ${canAccept ? 'bitfun-context-drop-zone--can-accept' : ''}
        ${!canAccept && isDragOver ? 'bitfun-context-drop-zone--cannot-accept' : ''}
        ${className}
      `.trim()}
      
      onDragEnter={handleDragEnter}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      data-dropzone="context-drop-zone"
    >
      {children}
    </div>
  );
};

export default ContextDropZone;
