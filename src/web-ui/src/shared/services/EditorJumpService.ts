import type { LineRange } from '@/shared/editor/LineRange';
import { fileTabManager, type FileTabOptions } from './FileTabManager';

/** File navigation always activates its resource tab before the editor settles its selection. */
export const editorJumpService = {
  async jumpToFile(filePath: string, line: number, column?: number, options?: Partial<FileTabOptions>): Promise<void> {
    fileTabManager.openFileAndJump(filePath, line, column, options);
  },
  async jumpToFileWithRange(filePath: string, range: LineRange, options?: Partial<FileTabOptions>): Promise<void> {
    fileTabManager.openFileAndJumpToRange(filePath, range, options);
  },
};
