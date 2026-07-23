import { create } from 'zustand';

interface MessageEditState {
  editingTurnId: string | null;
  draft: string;
  isSubmitting: boolean;
  beginEdit: (turnId: string, content: string) => void;
  cancelEdit: () => void;
  setDraft: (draft: string) => void;
  setSubmitting: (isSubmitting: boolean) => void;
}

export const useMessageEditStore = create<MessageEditState>((set) => ({
  editingTurnId: null,
  draft: '',
  isSubmitting: false,

  beginEdit: (turnId, content) => set({
    editingTurnId: turnId,
    draft: content,
    isSubmitting: false,
  }),

  cancelEdit: () => set({
    editingTurnId: null,
    draft: '',
    isSubmitting: false,
  }),

  setDraft: (draft) => set({ draft }),

  setSubmitting: (isSubmitting) => set({ isSubmitting }),
}));