import { api } from '@/infrastructure/api/service-api/ApiClient';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { workspaceAPI, type FileMetadata } from '@/infrastructure/api/service-api/WorkspaceAPI';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('MemoryLibraryAPI');

const MEMORY_INDEX_FILE = 'MEMORY.md';
const MAX_MEMORY_FILES = 250;

export type MemoryScopeKey = 'global' | 'workspace';
export type MemoryRecordType = 'index' | 'user' | 'feedback' | 'project' | 'reference' | 'workspace_overview' | 'unknown';

export interface MemoryStoragePaths {
  userConfigDir: string;
  userDataDir: string;
  cacheRoot: string;
  logsDir: string;
  tempDir: string;
  agenticOsMemoryDir: string;
}

export interface ProjectStoragePaths {
  projectRoot: string;
  runtimeRoot: string;
  agentsDir: string;
  sessionsDir: string;
  memoryDir: string;
  plansDir: string;
}

export interface MemorySpace {
  scope: MemoryScopeKey;
  label: string;
  memoryDir: string;
  available: boolean;
}

export interface MemoryRecord {
  id: string;
  scope: MemoryScopeKey;
  memoryDir: string;
  path: string;
  relativePath: string;
  title: string;
  description: string;
  type: MemoryRecordType;
  content: string;
  body: string;
  updatedAt?: number;
  size?: number;
  isIndex: boolean;
  isWorkspaceOverview: boolean;
}

export interface AutoMemoryStatus {
  globalEnabled: boolean;
  globalEvery: number;
  workspaceEnabled: boolean;
  workspaceEvery: number;
}

interface FrontmatterResult {
  data: Record<string, string>;
  body: string;
}

const normalizePath = (path: string) => path.replace(/\\/g, '/');

const joinPath = (basePath: string, child: string): string => {
  const separator = basePath.includes('\\') ? '\\' : '/';
  return `${basePath.replace(/[\\/]+$/, '')}${separator}${child.replace(/^[\\/]+/, '')}`;
};

const relativePath = (memoryDir: string, path: string): string => {
  const base = normalizePath(memoryDir).replace(/\/+$/, '');
  const target = normalizePath(path);
  return target.startsWith(`${base}/`) ? target.slice(base.length + 1) : target;
};

const parseFrontmatter = (content: string): FrontmatterResult => {
  if (!content.startsWith('---')) {
    return { data: {}, body: content };
  }

  const lines = content.split(/\r?\n/);
  const endIndex = lines.findIndex((line, index) => index > 0 && line.trim() === '---');
  if (endIndex < 0) {
    return { data: {}, body: content };
  }

  const data: Record<string, string> = {};
  for (const line of lines.slice(1, endIndex)) {
    const separatorIndex = line.indexOf(':');
    if (separatorIndex <= 0) {
      continue;
    }
    const key = line.slice(0, separatorIndex).trim();
    const value = line.slice(separatorIndex + 1).trim().replace(/^['"]|['"]$/g, '');
    if (key) {
      data[key] = value;
    }
  }

  return {
    data,
    body: lines.slice(endIndex + 1).join('\n').trimStart(),
  };
};

const titleFromPath = (path: string): string => {
  const fileName = normalizePath(path).split('/').pop() ?? path;
  return fileName.replace(/\.md$/i, '').replace(/[-_]+/g, ' ');
};

const normalizeRecordType = (value: string | undefined, relative: string): MemoryRecordType => {
  const type = value?.trim().toLowerCase();
  if (relative === MEMORY_INDEX_FILE) return 'index';
  if (relative.startsWith('workspaces_overview/')) return 'workspace_overview';
  if (type === 'user' || type === 'feedback' || type === 'project' || type === 'reference') {
    return type;
  }
  return 'unknown';
};

async function readMetadata(path: string): Promise<Partial<FileMetadata>> {
  try {
    return await workspaceAPI.getFileMetadata(path);
  } catch (error) {
    log.warn('Failed to read memory file metadata', { path, error });
    return {};
  }
}

export class MemoryLibraryAPI {
  async getStoragePaths(): Promise<MemoryStoragePaths> {
    return api.invoke<MemoryStoragePaths>('get_storage_paths', {});
  }

  async getProjectStoragePaths(workspacePath: string): Promise<ProjectStoragePaths> {
    return api.invoke<ProjectStoragePaths>('get_project_storage_paths', { workspacePath });
  }

  async getAutoMemoryStatus(): Promise<AutoMemoryStatus> {
    const [
      globalEnabled,
      globalEvery,
      workspaceEnabled,
      workspaceEvery,
    ] = await Promise.all([
      configManager.getConfig<boolean>('ai.auto_memory.global.enabled'),
      configManager.getConfig<number>('ai.auto_memory.global.extract_every_eligible_turns'),
      configManager.getConfig<boolean>('ai.auto_memory.workspace.enabled'),
      configManager.getConfig<number>('ai.auto_memory.workspace.extract_every_eligible_turns'),
    ]);

    return {
      globalEnabled: globalEnabled ?? true,
      globalEvery: globalEvery ?? 6,
      workspaceEnabled: workspaceEnabled ?? true,
      workspaceEvery: workspaceEvery ?? 1,
    };
  }

  async ensureMemorySpace(memoryDir: string): Promise<void> {
    const exists = await systemAPI.checkPathExists(memoryDir);
    if (!exists) {
      await workspaceAPI.createDirectory(memoryDir);
    }

    const indexPath = joinPath(memoryDir, MEMORY_INDEX_FILE);
    const indexExists = await systemAPI.checkPathExists(indexPath);
    if (!indexExists) {
      await workspaceAPI.writeFileContent(memoryDir, indexPath, '');
    }
  }

  async listMemoryRecords(space: MemorySpace): Promise<MemoryRecord[]> {
    if (!space.available) {
      return [];
    }

    await this.ensureMemorySpace(space.memoryDir);
    const files = await this.collectMarkdownFiles(space.memoryDir);
    const sortedFiles = files.sort((left, right) => {
      if (relativePath(space.memoryDir, left) === MEMORY_INDEX_FILE) return -1;
      if (relativePath(space.memoryDir, right) === MEMORY_INDEX_FILE) return 1;
      return relativePath(space.memoryDir, left).localeCompare(relativePath(space.memoryDir, right));
    });

    const records = await Promise.all(
      sortedFiles.slice(0, MAX_MEMORY_FILES).map((path) => this.readMemoryRecord(space, path))
    );

    return records.filter((record): record is MemoryRecord => Boolean(record));
  }

  async saveMemoryRecord(record: MemoryRecord, content: string): Promise<MemoryRecord> {
    await workspaceAPI.writeFileContent(record.path, record.path, content);
    const refreshed = await this.readMemoryRecord(
      {
        scope: record.scope,
        label: record.scope,
        memoryDir: record.memoryDir,
        available: true,
      },
      record.path
    );
    return refreshed ?? { ...record, content };
  }

  async deleteMemoryRecord(record: MemoryRecord): Promise<void> {
    await workspaceAPI.deleteFile(record.path);
  }

  async revealMemoryRecord(record: MemoryRecord): Promise<void> {
    await workspaceAPI.revealInExplorer(record.path);
  }

  async revealMemorySpace(space: MemorySpace): Promise<void> {
    await workspaceAPI.revealInExplorer(space.memoryDir);
  }

  private async collectMarkdownFiles(memoryDir: string): Promise<string[]> {
    const collected: string[] = [];

    const visit = async (dir: string): Promise<void> => {
      if (collected.length >= MAX_MEMORY_FILES) {
        return;
      }

      let children;
      try {
        children = await workspaceAPI.getDirectoryChildren(dir);
      } catch (error) {
        log.warn('Failed to list memory directory', { dir, error });
        return;
      }

      for (const child of children) {
        if (collected.length >= MAX_MEMORY_FILES) {
          return;
        }
        if (child.isDirectory) {
          await visit(child.path);
        } else if (child.name.toLowerCase().endsWith('.md')) {
          collected.push(child.path);
        }
      }
    };

    await visit(memoryDir);

    const indexPath = joinPath(memoryDir, MEMORY_INDEX_FILE);
    if (!collected.some((path) => normalizePath(path) === normalizePath(indexPath))) {
      collected.unshift(indexPath);
    }

    return collected;
  }

  private async readMemoryRecord(space: MemorySpace, path: string): Promise<MemoryRecord | null> {
    try {
      const content = await workspaceAPI.readFileContent(path);
      const metadata = await readMetadata(path);
      const rel = relativePath(space.memoryDir, path);
      const frontmatter = parseFrontmatter(content);
      const type = normalizeRecordType(frontmatter.data.type, rel);
      const isIndex = type === 'index';
      const isWorkspaceOverview = type === 'workspace_overview';
      const title = isIndex
        ? 'MEMORY.md'
        : frontmatter.data.name || titleFromPath(rel);

      return {
        id: `${space.scope}:${rel}`,
        scope: space.scope,
        memoryDir: space.memoryDir,
        path,
        relativePath: rel,
        title,
        description: frontmatter.data.description || firstContentLine(frontmatter.body),
        type,
        content,
        body: frontmatter.body,
        updatedAt: typeof metadata.modified === 'number' ? metadata.modified : undefined,
        size: typeof metadata.size === 'number' ? metadata.size : undefined,
        isIndex,
        isWorkspaceOverview,
      };
    } catch (error) {
      log.warn('Failed to read memory record', { path, error });
      return null;
    }
  }
}

function firstContentLine(content: string): string {
  return content
    .split(/\r?\n/)
    .map((line) => line.replace(/^#+\s*/, '').trim())
    .find(Boolean) ?? '';
}

export const memoryLibraryAPI = new MemoryLibraryAPI();
