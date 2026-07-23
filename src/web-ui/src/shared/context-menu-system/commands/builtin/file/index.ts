 

export * from './NewFileCommand';
export * from './NewFolderCommand';
export * from './RenameCommand';
export * from './DeleteCommand';
export * from './CopyPathCommand';
export * from './CopyRelativePathCommand';
export * from './RevealInExplorerCommand';
export * from './OpenHtmlInBrowserCommand';

import { NewFileCommand } from './NewFileCommand';
import { NewFolderCommand } from './NewFolderCommand';
import { RenameCommand } from './RenameCommand';
import { DeleteFileCommand } from './DeleteCommand';
import { CopyPathCommand } from './CopyPathCommand';
import { CopyRelativePathCommand } from './CopyRelativePathCommand';
import { RevealInExplorerCommand } from './RevealInExplorerCommand';
import { OpenHtmlInBrowserCommand } from './OpenHtmlInBrowserCommand';

 
export function getFileCommands() {
  return [
    new NewFileCommand(),
    new NewFolderCommand(),
    new RenameCommand(),
    new DeleteFileCommand(),
    new CopyPathCommand(),
    new CopyRelativePathCommand(),
    new RevealInExplorerCommand(),
    new OpenHtmlInBrowserCommand()
  ];
}

