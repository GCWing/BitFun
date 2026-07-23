 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';



export interface DiffOptions {
  ignore_whitespace?: boolean;
  context_lines?: number;
}

export interface DiffResult {
  hunks: DiffHunk[];
  additions: number;
  deletions: number;
  changes: number;
}

export interface DiffHunk {
  old_start: number;
  old_lines: number;
  new_start: number;
  new_lines: number;
  lines: DiffLine[];
}

export interface DiffLine {
  line_type: 'context' | 'add' | 'delete';
  content: string;
  old_line_number?: number;
  new_line_number?: number;
}



export class DiffAPI {
   
  async computeDiff(oldContent: string, newContent: string, options?: DiffOptions): Promise<DiffResult> {
    try {
      return await api.invoke('compute_diff', {
        request: { oldContent, newContent, options }
      });
    } catch (error) {
      throw createTauriCommandError('compute_diff', error, { oldContent, newContent, options });
    }
  }

   
  async applyPatch(content: string, patch: string): Promise<string> {
    try {
      return await api.invoke('apply_patch', {
        request: { content, patch }
      });
    } catch (error) {
      throw createTauriCommandError('apply_patch', error, { content, patch });
    }
  }

   
  async saveMergedDiffContent(filePath: string, content: string): Promise<void> {
    try {
      await api.invoke('save_merged_diff_content', {
        request: { filePath, content }
      });
    } catch (error) {
      throw createTauriCommandError('save_merged_diff_content', error, { filePath, content });
    }
  }
}


export const diffAPI = new DiffAPI();