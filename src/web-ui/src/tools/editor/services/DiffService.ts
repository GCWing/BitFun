/**
 * Unified frontend Diff service.
 * Provides diff computation, hunk operations, and statistics.
 * Supports both local and backend computation modes.
 */

import { diffLines, diffWords, Change } from 'diff';
import { diffAPI } from '@/infrastructure/api';
import type { DiffResult } from '@/infrastructure/api/service-api/DiffAPI';

export interface DiffOptions {
  ignoreWhitespace?: boolean;
  contextLines?: number;
  timeout?: number;
  useBackend?: boolean;
}

export interface DiffLine {
  type: 'unchanged' | 'added' | 'removed';
  content: string;
  originalLineNumber?: number;
  modifiedLineNumber?: number;
}

export interface CharDiffSegment {
  type: 'unchanged' | 'added' | 'removed';
  value: string;
}

export interface CharDiffResult {
  originalLine: string;
  modifiedLine: string;
  segments: CharDiffSegment[];
}

export interface DiffHunkData {
  id: string;
  originalStartLine: number;
  originalEndLine: number;
  modifiedStartLine: number;
  modifiedEndLine: number;
  originalContent: string[];
  modifiedContent: string[];
}

export interface DiffStats {
  additions: number;
  deletions: number;
  modifications: number;
  totalChanges: number;
  unchangedLines: number;
}

export interface DiffComputeResult {
  lines: DiffLine[];
  stats: DiffStats;
  hunks: DiffHunkData[];
}

/**
 * Unified Diff Service
 * 
 * Provides both frontend and backend diff computation modes.
 */
export class DiffService {
  private static instance: DiffService;

  private constructor() {}

  static getInstance(): DiffService {
    if (!DiffService.instance) {
      DiffService.instance = new DiffService();
    }
    return DiffService.instance;
  }

  async computeDiff(
    original: string,
    modified: string,
    options: DiffOptions = {}
  ): Promise<DiffComputeResult> {
    const { timeout = 5000, useBackend = false } = options;

    if (useBackend) {
      return this.computeDiffBackend(original, modified, options);
    }

    return this.computeDiffWithTimeout(original, modified, timeout);
  }

  async computeDiffWithTimeout(
    original: string,
    modified: string,
    timeout: number = 5000
  ): Promise<DiffComputeResult> {
    return new Promise((resolve, reject) => {
      const timeoutId = setTimeout(() => {
        reject(new Error(`Diff computation timeout after ${timeout}ms`));
      }, timeout);

      try {
        const result = this.computeDiffLocal(original, modified);
        clearTimeout(timeoutId);
        resolve(result);
      } catch (error) {
        clearTimeout(timeoutId);
        reject(error);
      }
    });
  }

  private computeDiffLocal(original: string, modified: string): DiffComputeResult {
    const changes: Change[] = diffLines(original, modified);
    const lines: DiffLine[] = [];
    const hunks: DiffHunkData[] = [];
    
    let originalLineNumber = 1;
    let modifiedLineNumber = 1;
    let additions = 0;
    let deletions = 0;
    let unchangedLines = 0;
    
    let currentHunk: DiffHunkData | null = null;
    let hunkIndex = 0;
    
    for (const change of changes) {
      const changeLines = change.value.split('\n');
      if (changeLines.length > 0 && changeLines[changeLines.length - 1] === '') {
        changeLines.pop();
      }
      
      for (const line of changeLines) {
        if (change.added) {
          lines.push({
            type: 'added',
            content: line,
            modifiedLineNumber: modifiedLineNumber++,
          });
          additions++;
          
          if (!currentHunk) {
            currentHunk = {
              id: `hunk-${hunkIndex++}`,
              originalStartLine: originalLineNumber,
              originalEndLine: originalLineNumber - 1,
              modifiedStartLine: modifiedLineNumber - 1,
              modifiedEndLine: modifiedLineNumber - 1,
              originalContent: [],
              modifiedContent: [],
            };
          }
          currentHunk.modifiedEndLine = modifiedLineNumber - 1;
          currentHunk.modifiedContent.push(line);
          
        } else if (change.removed) {
          lines.push({
            type: 'removed',
            content: line,
            originalLineNumber: originalLineNumber++,
          });
          deletions++;
          
          if (!currentHunk) {
            currentHunk = {
              id: `hunk-${hunkIndex++}`,
              originalStartLine: originalLineNumber - 1,
              originalEndLine: originalLineNumber - 1,
              modifiedStartLine: modifiedLineNumber,
              modifiedEndLine: modifiedLineNumber - 1,
              originalContent: [],
              modifiedContent: [],
            };
          }
          currentHunk.originalEndLine = originalLineNumber - 1;
          currentHunk.originalContent.push(line);
          
        } else {
          if (currentHunk) {
            hunks.push(currentHunk);
            currentHunk = null;
          }
          
          lines.push({
            type: 'unchanged',
            content: line,
            originalLineNumber: originalLineNumber++,
            modifiedLineNumber: modifiedLineNumber++,
          });
          unchangedLines++;
        }
      }
    }
    
    if (currentHunk) {
      hunks.push(currentHunk);
    }
    
    return {
      lines,
      stats: {
        additions,
        deletions,
        modifications: Math.min(additions, deletions),
        totalChanges: additions + deletions,
        unchangedLines,
      },
      hunks,
    };
  }

  private async computeDiffBackend(
    original: string,
    modified: string,
    options: DiffOptions
  ): Promise<DiffComputeResult> {
    const result = await diffAPI.computeDiff(original, modified, {
      ignore_whitespace: options.ignoreWhitespace,
      context_lines: options.contextLines,
    });
    
    return this.convertBackendResult(result, original, modified);
  }

  private convertBackendResult(
    result: DiffResult,
    original: string,
    _modified: string
  ): DiffComputeResult {
    const lines: DiffLine[] = [];
    const hunks: DiffHunkData[] = [];
    
    for (const hunk of result.hunks) {
      const hunkData: DiffHunkData = {
        id: `hunk-${hunks.length}`,
        originalStartLine: hunk.old_start,
        originalEndLine: hunk.old_start + hunk.old_lines - 1,
        modifiedStartLine: hunk.new_start,
        modifiedEndLine: hunk.new_start + hunk.new_lines - 1,
        originalContent: [],
        modifiedContent: [],
      };
      
      for (const line of hunk.lines) {
        const diffLine: DiffLine = {
          type: line.line_type === 'add' ? 'added' : line.line_type === 'delete' ? 'removed' : 'unchanged',
          content: line.content,
          originalLineNumber: line.old_line_number,
          modifiedLineNumber: line.new_line_number,
        };
        lines.push(diffLine);
        
        if (line.line_type === 'delete') {
          hunkData.originalContent.push(line.content);
        } else if (line.line_type === 'add') {
          hunkData.modifiedContent.push(line.content);
        }
      }
      
      hunks.push(hunkData);
    }
    
    const originalLines = original.split('\n').length;
    
    return {
      lines,
      stats: {
        additions: result.additions,
        deletions: result.deletions,
        modifications: Math.min(result.additions, result.deletions),
        totalChanges: result.changes,
        unchangedLines: originalLines - result.deletions,
      },
      hunks,
    };
  }

  computeCharDiff(originalLine: string, modifiedLine: string): CharDiffResult {
    const changes = diffWords(originalLine, modifiedLine);
    const segments: CharDiffSegment[] = [];
    
    for (const change of changes) {
      segments.push({
        type: change.added ? 'added' : change.removed ? 'removed' : 'unchanged',
        value: change.value,
      });
    }
    
    return {
      originalLine,
      modifiedLine,
      segments,
    };
  }

  acceptHunk(content: string, hunk: DiffHunkData): string {
    const lines = content.split('\n');
    
    const startIndex = hunk.originalStartLine - 1;
    const deleteCount = hunk.originalEndLine - hunk.originalStartLine + 1;
    
    lines.splice(startIndex, deleteCount, ...hunk.modifiedContent);
    
    return lines.join('\n');
  }

  rejectHunk(content: string, _hunk: DiffHunkData): string {
    return content;
  }

  acceptHunks(content: string, hunks: DiffHunkData[]): string {
    // Apply from bottom to top to avoid line number offset
    const sortedHunks = [...hunks].sort((a, b) => b.originalStartLine - a.originalStartLine);
    
    let result = content;
    for (const hunk of sortedHunks) {
      result = this.acceptHunk(result, hunk);
    }
    
    return result;
  }

  getDiffStats(diffResult: DiffComputeResult): DiffStats {
    return diffResult.stats;
  }

  applyContextCollapsing(
    lines: DiffLine[],
    contextLines: number
  ): DiffLine[] {
    if (contextLines < 0) return lines;
    
    const result: DiffLine[] = [];
    const changeIndices: number[] = [];
    
    lines.forEach((line, index) => {
      if (line.type === 'added' || line.type === 'removed') {
        changeIndices.push(index);
      }
    });
    
    if (changeIndices.length === 0) {
      return lines;
    }
    
    const showLine = new Set<number>();
    for (const idx of changeIndices) {
      for (let i = Math.max(0, idx - contextLines); i <= Math.min(lines.length - 1, idx + contextLines); i++) {
        showLine.add(i);
      }
    }
    
    let lastShownIndex = -1;
    for (let i = 0; i < lines.length; i++) {
      if (showLine.has(i)) {
        if (lastShownIndex >= 0 && i > lastShownIndex + 1) {
          const skippedCount = i - lastShownIndex - 1;
          result.push({
            type: 'unchanged',
            content: `... ${skippedCount} lines omitted ...`,
          });
        }
        result.push(lines[i]);
        lastShownIndex = i;
      }
    }
    
    return result;
  }
}

export const diffService = DiffService.getInstance();

export const computeDiff = diffService.computeDiff.bind(diffService);
export const computeCharDiff = diffService.computeCharDiff.bind(diffService);
export const acceptHunk = diffService.acceptHunk.bind(diffService);
export const rejectHunk = diffService.rejectHunk.bind(diffService);
export const getDiffStats = diffService.getDiffStats.bind(diffService);
