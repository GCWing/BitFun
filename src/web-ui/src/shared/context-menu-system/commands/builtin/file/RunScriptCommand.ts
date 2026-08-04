import { BaseCommand } from '../../BaseCommand';
import { CommandResult } from '../../../types/command.types';
import { MenuContext, ContextType, FileNodeContext, TabContext } from '../../../types/context.types';
import { i18nService } from '@/infrastructure/i18n';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { isRemoteWorkspace } from '@/shared/types';
import { hasNonFileUriScheme } from '@/shared/utils/pathUtils';
import { detectLanguage } from '@/infrastructure/language-detection/utils/helpers';
import { getTerminalService } from '@/tools/terminal/services/TerminalService';
import { openShellSessionTarget } from '@/shared/services/openShellSessionTarget';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('RunScriptCommand');

/** Language ID → interpreter command prefix. */
const INTERPRETER_MAP: Record<string, string> = {
  python: 'python',
  javascript: 'node',
  typescript: 'npx tsx',
  shell: 'bash',
  powershell: 'powershell -File',
  batch: 'cmd /c',
  ruby: 'ruby',
  go: 'go run',
  rust: 'cargo run',
  php: 'php',
  lua: 'lua',
  perl: 'perl',
};

/** Windows path-safe quoting for a file path. */
function quotePath(filePath: string): string {
  // Single-quote wrapping with internal single-quote escaping for POSIX shells.
  return `'${filePath.replace(/'/g, `'\\''`)}'`;
}

function getContextFilePath(context: MenuContext): string | undefined {
  if (context.type === ContextType.FILE_NODE || context.type === ContextType.FOLDER_NODE) {
    return (context as FileNodeContext).filePath;
  }
  if (context.type === ContextType.TAB) {
    return (context as TabContext).filePath;
  }
  return undefined;
}

function getContextWorkspacePath(context: MenuContext): string | undefined {
  if (context.type === ContextType.FILE_NODE || context.type === ContextType.FOLDER_NODE) {
    return (context as FileNodeContext).workspacePath;
  }
  if (context.type === ContextType.TAB) {
    return (context as TabContext).workspacePath;
  }
  return undefined;
}

/** Get the interpreter command for a file, or undefined if unsupported. */
export function getInterpreterForFile(filePath: string): string | undefined {
  const result = detectLanguage(filePath);
  return INTERPRETER_MAP[result.language.id];
}

export class RunScriptCommand extends BaseCommand {
  constructor() {
    const t = i18nService.getT();
    super({
      id: 'file.run-script',
      label: t('common:file.runScript'),
      description: t('common:file.runScriptDescription'),
      icon: 'Play',
      category: 'file',
    });
  }

  canExecute(context: MenuContext): boolean {
    const currentWorkspace = workspaceManager.getState().currentWorkspace;
    if (isRemoteWorkspace(currentWorkspace)) return false;

    const filePath = getContextFilePath(context);
    if (!filePath || hasNonFileUriScheme(filePath)) return false;

    return Boolean(getInterpreterForFile(filePath));
  }

  async execute(context: MenuContext): Promise<CommandResult> {
    const t = i18nService.getT();
    try {
      const filePath = getContextFilePath(context);
      if (!filePath) {
        return this.failure(t('errors:file.runScriptFailed'));
      }

      const interpreter = getInterpreterForFile(filePath);
      if (!interpreter) {
        return this.failure(t('errors:file.runScriptUnsupported'));
      }

      const workspacePath = getContextWorkspacePath(context)
        ?? workspaceManager.getState().currentWorkspace?.path;

      const service = getTerminalService();
      const session = await service.createSession({
        name: t('common:file.runScriptSessionName', { file: filePath.split(/[/\\]/).pop() }),
        workingDirectory: workspacePath,
        source: 'manual',
      });

      // Wait for the shell to be ready before sending the command.
      await new Promise(resolve => setTimeout(resolve, 600));

      const command = `${interpreter} ${quotePath(filePath)}`;
      await service.sendCommand(session.id, command);

      openShellSessionTarget({ sessionId: session.id, sessionName: session.name });

      log.info('Script run started', { filePath, interpreter, sessionId: session.id });

      return this.success(t('common:file.runScriptStarting', { file: filePath.split(/[/\\]/).pop() }), {
        sessionId: session.id,
        command,
      });
    } catch (error) {
      log.error('Failed to run script', { error });
      return this.failure(t('errors:file.runScriptFailed'), error as Error);
    }
  }
}
