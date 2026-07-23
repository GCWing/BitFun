 

 
export const KEYBINDINGS = {
  
  copy: 'Ctrl+C',
  cut: 'Ctrl+X',
  paste: 'Ctrl+V',
  selectAll: 'Ctrl+A',
  undo: 'Ctrl+Z',
  redo: 'Ctrl+Y',

  
  newFile: 'Ctrl+N',
  openFile: 'Ctrl+O',
  save: 'Ctrl+S',
  saveAs: 'Ctrl+Shift+S',
  closeFile: 'Ctrl+W',

  
  refresh: 'F5',
  toggleSidebar: 'Ctrl+B',
  toggleTerminal: 'Ctrl+`',

  
  find: 'Ctrl+F',
  replace: 'Ctrl+H',
  findInFiles: 'Ctrl+Shift+F',

  
  terminalCopy: 'Ctrl+Shift+C',
  terminalPaste: 'Ctrl+Shift+V',
  terminalClear: 'Ctrl+L',

  
  rename: 'F2',
  delete: 'Delete',
  devTools: 'F12'
};

 
export function formatShortcut(shortcut: string): string {
  return shortcut
    .replace('Ctrl', '⌃')
    .replace('Alt', '⌥')
    .replace('Shift', '⇧')
    .replace('Cmd', '⌘');
}

 
export function matchesShortcut(event: KeyboardEvent, shortcut: string): boolean {
  const parts = shortcut.split('+').map(p => p.toLowerCase());
  
  const hasCtrl = parts.includes('ctrl') === event.ctrlKey;
  const hasAlt = parts.includes('alt') === event.altKey;
  const hasShift = parts.includes('shift') === event.shiftKey;
  const hasMeta = parts.includes('cmd') === event.metaKey;
  
  const key = parts[parts.length - 1];
  const matchesKey = event.key.toLowerCase() === key.toLowerCase();
  
  return hasCtrl && hasAlt && hasShift && hasMeta && matchesKey;
}

