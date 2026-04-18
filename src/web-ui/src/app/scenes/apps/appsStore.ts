import { create } from 'zustand';

export type AppsScenePage = 'home' | 'app-detail' | 'agent-detail';
export type AppsTab = 'agent-app' | 'live-app' | 'bridge-app';

interface AppsStoreState {
  activeTab: AppsTab;
  page: AppsScenePage;
  searchQuery: string;
  selectedAppId: string | null;
  selectedAgentId: string | null;
  setActiveTab: (tab: AppsTab) => void;
  setSearchQuery: (query: string) => void;
  openHome: () => void;
  openAppDetail: (appId: string) => void;
  openAgentDetail: (agentId: string, appId?: string | null) => void;
}

export const useAppsStore = create<AppsStoreState>((set) => ({
  activeTab: 'agent-app',
  page: 'home',
  searchQuery: '',
  selectedAppId: null,
  selectedAgentId: null,
  setActiveTab: (activeTab) => set({ activeTab, page: 'home', selectedAppId: null, selectedAgentId: null }),
  setSearchQuery: (query) => set({ searchQuery: query }),
  openHome: () => set({
    page: 'home',
    selectedAppId: null,
    selectedAgentId: null,
  }),
  openAppDetail: (appId) => set({
    page: 'app-detail',
    selectedAppId: appId,
    selectedAgentId: null,
  }),
  openAgentDetail: (agentId, appId = null) => set({
    page: 'agent-detail',
    selectedAgentId: agentId,
    selectedAppId: appId,
  }),
}));
