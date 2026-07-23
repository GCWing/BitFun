 

export * from './CopyCommand';
export * from './CutCommand';
export * from './PasteCommand';
export * from './SelectAllCommand';
export * from './RefreshCommand';
export * from './file';

import { CopyCommand } from './CopyCommand';
import { CutCommand } from './CutCommand';
import { PasteCommand } from './PasteCommand';
import { SelectAllCommand } from './SelectAllCommand';
import { RefreshCommand } from './RefreshCommand';
import { getFileCommands } from './file';

 
export function getBuiltinCommands() {
  return [
    new CopyCommand(),
    new CutCommand(),
    new PasteCommand(),
    new SelectAllCommand(),
    new RefreshCommand(),
    ...getFileCommands()
  ];
}

