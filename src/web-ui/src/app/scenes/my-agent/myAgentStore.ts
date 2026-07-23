import { create } from 'zustand';

interface MyAgentState {
  selectedAssistantWorkspaceId: string | null;
  setSelectedAssistantWorkspaceId: (workspaceId: string | null) => void;
}

export const useMyAgentStore = create<MyAgentState>((set) => ({
  selectedAssistantWorkspaceId: null,
  setSelectedAssistantWorkspaceId: (workspaceId) => set({ selectedAssistantWorkspaceId: workspaceId }),
}));
