/**
 * gitSceneStore — Zustand store for the Git scene.
 *
 * Shared between GitNav (left sidebar) and GitScene content area
 * so both reflect the same active view and selection state.
 */

import { create } from 'zustand';

export type GitSceneView = 'working-copy' | 'branches' | 'graph';

interface GitSceneState {
  activeView: GitSceneView;
  /** Working-copy: file path selected for diff preview */
  selectedFile: string | null;
  /** History: selected commit hash */
  selectedCommit: string | null;
  /** History: file path selected within commit detail */
  selectedCommitFile: string | null;
  /** Working-copy: file list column width (px) */
  fileListWidth: number;

  setActiveView: (view: GitSceneView) => void;
  setSelectedFile: (file: string | null) => void;
  setSelectedCommit: (hash: string | null) => void;
  setSelectedCommitFile: (file: string | null) => void;
  setFileListWidth: (width: number) => void;
}

const DEFAULT_FILE_LIST_WIDTH = 260;

export const useGitSceneStore = create<GitSceneState>((set) => ({
  activeView: 'working-copy',
  selectedFile: null,
  selectedCommit: null,
  selectedCommitFile: null,
  fileListWidth: DEFAULT_FILE_LIST_WIDTH,

  setActiveView: (view) => set({ activeView: view }),
  setSelectedFile: (file) => set({ selectedFile: file }),
  setSelectedCommit: (hash) => set({ selectedCommit: hash, selectedCommitFile: null }),
  setSelectedCommitFile: (file) => set({ selectedCommitFile: file }),
  setFileListWidth: (width) => set({ fileListWidth: Math.max(180, Math.min(400, width)) }),
}));
