import { SnapshotFile, DiffBlock } from './SnapshotStateManager';

export interface CompactDiffResult {
  filePath: string;
  criticalBlocks: DiffBlock[];
  totalBlocks: number;
  summary: {
    additions: number;
    deletions: number;
    modifications: number;
  };
}

export interface FullDiffResult {
  filePath: string;
  originalContent: string;
  modifiedContent: string;
  diffBlocks: DiffBlock[];
  contextLines: string[];
  navigation: BlockNavigation[];
}

export interface BlockNavigation {
  blockId: string;
  type: 'added' | 'removed' | 'modified';
  status: 'pending' | 'accepted' | 'rejected';
  lineNumber: number;
  description: string;
}

export class BlockPriorityAnalyzer {
  filterCriticalBlocks(blocks: DiffBlock[]): DiffBlock[] {
    return blocks.filter(block => {
      if (block.priority === 'critical' || block.priority === 'important') {
        return true;
      }

      const lineCount = Math.abs(block.originalEndLine - block.originalStartLine);
      if (lineCount > 5) {
        return true;
      }

      const content = block.modifiedContent.toLowerCase();
      const criticalKeywords = ['function', 'class', 'import', 'export', 'const', 'let', 'var', 'if', 'for', 'while'];
      if (criticalKeywords.some(keyword => content.includes(keyword))) {
        return true;
      }

      return false;
    });
  }

  analyzePriority(block: DiffBlock): 'critical' | 'important' | 'minor' {
    const content = block.modifiedContent.toLowerCase();
    
    if (content.includes('function') || content.includes('class') || content.includes('interface')) {
      return 'critical';
    }

    if (content.includes('import') || content.includes('export')) {
      return 'important';
    }

    if (content.includes('if') || content.includes('for') || content.includes('while')) {
      return 'important';
    }

    const lineCount = Math.abs(block.originalEndLine - block.originalStartLine);
    if (lineCount > 10) {
      return 'important';
    }

    return 'minor';
  }
}

export class DiffDisplayEngine {
  private blockPriorityAnalyzer: BlockPriorityAnalyzer;

  constructor() {
    this.blockPriorityAnalyzer = new BlockPriorityAnalyzer();
  }

  generateCompactDiff(file: SnapshotFile): CompactDiffResult {
    const diffBlocks = file.diffBlocks || [];
    const criticalBlocks = this.blockPriorityAnalyzer.filterCriticalBlocks(diffBlocks);
    
    const summary = this.calculateSummary(diffBlocks);

    return {
      filePath: file.filePath,
      criticalBlocks,
      totalBlocks: diffBlocks.length,
      summary
    };
  }

  generateFullDiff(file: SnapshotFile): FullDiffResult {
    const diffBlocks = file.diffBlocks || [];
    const contextLines = this.generateContextLines(file);
    const navigation = this.generateBlockNavigation(diffBlocks);

    return {
      filePath: file.filePath,
      originalContent: file.originalContent,
      modifiedContent: file.modifiedContent,
      diffBlocks,
      contextLines,
      navigation
    };
  }

  private calculateSummary(blocks: DiffBlock[]): {
    additions: number;
    deletions: number;
    modifications: number;
  } {
    let additions = 0;
    let deletions = 0;
    let modifications = 0;

    blocks.forEach(block => {
      switch (block.type) {
        case 'added':
          additions += block.modifiedEndLine - block.modifiedStartLine + 1;
          break;
        case 'removed':
          deletions += block.originalEndLine - block.originalStartLine + 1;
          break;
        case 'modified':
          modifications += Math.max(
            block.originalEndLine - block.originalStartLine + 1,
            block.modifiedEndLine - block.modifiedStartLine + 1
          );
          break;
      }
    });

    return { additions, deletions, modifications };
  }

  private generateContextLines(file: SnapshotFile): string[] {
    // treat original file lines as "context".
    return file.originalContent.split('\n');
  }

  private generateBlockNavigation(blocks: DiffBlock[]): BlockNavigation[] {
    return blocks.map(block => ({
      blockId: block.id,
      type: block.type,
      status: block.status,
      lineNumber: block.originalStartLine,
      description: this.generateBlockDescription(block)
    }));
  }

  private generateBlockDescription(block: DiffBlock): string {
    const lineCount = Math.abs(block.originalEndLine - block.originalStartLine + 1);
    
    switch (block.type) {
      case 'added':
        return `Added ${lineCount} lines`;
      case 'removed':
        return `Removed ${lineCount} lines`;
      case 'modified':
        return `Modified ${lineCount} lines`;
      default:
        return `Changed ${lineCount} lines`;
    }
  }

  generateDiffForMode(file: SnapshotFile, mode: 'compact' | 'full'): CompactDiffResult | FullDiffResult {
    if (mode === 'compact') {
      return this.generateCompactDiff(file);
    } else {
      return this.generateFullDiff(file);
    }
  }

  hasCriticalChanges(file: SnapshotFile): boolean {
    const diffBlocks = file.diffBlocks || [];
    const criticalBlocks = this.blockPriorityAnalyzer.filterCriticalBlocks(diffBlocks);
    return criticalBlocks.length > 0;
  }

  getFileStats(file: SnapshotFile): {
    totalBlocks: number;
    criticalBlocks: number;
    pendingBlocks: number;
    acceptedBlocks: number;
    rejectedBlocks: number;
  } {
    const diffBlocks = file.diffBlocks || [];
    const criticalBlocks = this.blockPriorityAnalyzer.filterCriticalBlocks(diffBlocks);

    return {
      totalBlocks: diffBlocks.length,
      criticalBlocks: criticalBlocks.length,
      pendingBlocks: diffBlocks.filter(b => b.status === 'pending').length,
      acceptedBlocks: diffBlocks.filter(b => b.status === 'accepted').length,
      rejectedBlocks: diffBlocks.filter(b => b.status === 'rejected').length
    };
  }
}
