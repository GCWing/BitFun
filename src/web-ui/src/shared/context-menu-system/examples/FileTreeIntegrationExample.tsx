 

import React, { useEffect } from 'react';
import { initContextMenuSystem } from '../init';

 




export function initializeFileTreeContextMenu() {
  initContextMenuSystem({
    registerBuiltinCommands: true,
    registerBuiltinProviders: true,
    debug: process.env.NODE_ENV === 'development'
  });
}











import { globalEventBus } from '../../../infrastructure/event-bus';

export function FileTreeEventHandler() {
  useEffect(() => {
    
    const unsubOpen = globalEventBus.on('file:open', (data: any) => {
      
    });

    
    const unsubNewFile = globalEventBus.on('file:new-file', (data: any) => {
      
      
      
      
    });

    
    const unsubNewFolder = globalEventBus.on('file:new-folder', (data: any) => {
      
    });

    
    const unsubRename = globalEventBus.on('file:rename', (data: any) => {
      
      
      
      
    });

    
    const unsubDelete = globalEventBus.on('file:delete', (data: any) => {
      
      
      
      
    });

    
    const unsubReveal = globalEventBus.on('file:reveal', (data: any) => {
      
      
    });

    
    const unsubTerminal = globalEventBus.on('terminal:open-at-path', (data: any) => {
      
    });

    
    return () => {
      unsubOpen();
      unsubNewFile();
      unsubNewFolder();
      unsubRename();
      unsubDelete();
      unsubReveal();
      unsubTerminal();
    };
  }, []);

  return null;
}



export function FileTreeWithContextMenuExample() {
  
  useEffect(() => {
    initializeFileTreeContextMenu();
  }, []);

  return (
    <div>
      
      
      
      
      <FileTreeEventHandler />
    </div>
  );
}


 


 

