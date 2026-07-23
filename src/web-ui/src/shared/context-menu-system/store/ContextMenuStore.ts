 

import { create } from 'zustand';
import { MenuState, MenuItem, MenuPosition } from '../types/menu.types';
import { MenuContext } from '../types/context.types';

 
interface ContextMenuStore extends MenuState {
  // Actions
   
  showMenu: (position: MenuPosition, items: MenuItem[], context: MenuContext) => void;
   
  hideMenu: () => void;
   
  updateItems: (items: MenuItem[]) => void;
   
  setActiveSubmenu: (submenuId: string | null) => void;
   
  setFocusedIndex: (index: number) => void;
   
  setSearchQuery: (query: string) => void;
   
  reset: () => void;
}

 
const initialState: MenuState = {
  visible: false,
  position: null,
  items: [],
  context: null,
  activeSubmenuId: null,
  focusedIndex: -1,
  searchQuery: ''
};

 
export const useContextMenuStore = create<ContextMenuStore>((set) => ({
  ...initialState,

  showMenu: (position, items, context) => {
    set({
      visible: true,
      position,
      items,
      context,
      focusedIndex: -1,
      activeSubmenuId: null,
      searchQuery: ''
    });
  },

  hideMenu: () => {
    set({
      visible: false,
      activeSubmenuId: null,
      focusedIndex: -1
    });
  },

  updateItems: (items) => {
    set({ items });
  },

  setActiveSubmenu: (submenuId) => {
    set({ activeSubmenuId: submenuId });
  },

  setFocusedIndex: (index) => {
    set({ focusedIndex: index });
  },

  setSearchQuery: (query) => {
    set({ searchQuery: query });
  },

  reset: () => {
    set(initialState);
  }
}));

 
export const selectMenuVisible = (state: ContextMenuStore) => state.visible;

 
export const selectMenuItems = (state: ContextMenuStore) => state.items;

 
export const selectMenuPosition = (state: ContextMenuStore) => state.position;

 
export const selectMenuContext = (state: ContextMenuStore) => state.context;

 
export const selectFocusedIndex = (state: ContextMenuStore) => state.focusedIndex;

 
export const selectSearchQuery = (state: ContextMenuStore) => state.searchQuery;

 
export const selectFilteredItems = (state: ContextMenuStore) => {
  if (!state.searchQuery) {
    return state.items;
  }
  
  const query = state.searchQuery.toLowerCase();
  return state.items.filter(item => 
    item.label.toLowerCase().includes(query) ||
    item.id.toLowerCase().includes(query)
  );
};

