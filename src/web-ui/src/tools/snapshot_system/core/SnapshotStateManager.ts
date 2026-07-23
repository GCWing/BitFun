import { SnapshotEventBus, SNAPSHOT_EVENTS } from './SnapshotEventBus';
import { SnapshotSystemService } from '../services/SnapshotSystemService';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('SnapshotStateManager');

export interface SnapshotFile {
  filePath: string;
  sessionId: string;
  originalContent: string;
  modifiedContent: string;
  fileStatus: 'pending' | 'partial' | 'accepted' | 'rejected';
  lastModified: number;
  diffBlocks?: DiffBlock[];
}

export interface DiffBlock {
  id: string;
  type: 'added' | 'removed' | 'modified';
  status: 'pending' | 'accepted' | 'rejected';
  originalStartLine: number;
  originalEndLine: number;
  modifiedStartLine: number;
  modifiedEndLine: number;
  originalContent: string;
  modifiedContent: string;
  priority: 'critical' | 'important' | 'minor';
}

export interface SessionState {
  sessionId: string;
  files: Map<string, SnapshotFile>;
  totalBlocks: number;
  acceptedBlocks: number;
  rejectedBlocks: number;
  pendingBlocks: number;
  sessionStatus: 'working' | 'pending' | 'partial' | 'completed';
  lastActivity: number;
}

export class SnapshotStateManager {
  private static instance: SnapshotStateManager;
  private eventBus: SnapshotEventBus;
  private snapshotService: SnapshotSystemService;
  private sessions: Map<string, SessionState> = new Map();
  // NOTE: Indexed by filePath (not session-scoped). Assumes the UI only tracks one active
  // snapshot session per file at a time.
  private files: Map<string, SnapshotFile> = new Map();

  private constructor() {
    this.eventBus = SnapshotEventBus.getInstance();
    this.snapshotService = SnapshotSystemService.getInstance();
    this.setupEventListeners();
  }

  public static getInstance(): SnapshotStateManager {
    if (!SnapshotStateManager.instance) {
      SnapshotStateManager.instance = new SnapshotStateManager();
    }
    return SnapshotStateManager.instance;
  }

  private setupEventListeners(): void {
    this.eventBus.on(SNAPSHOT_EVENTS.FILE_OPERATION_COMPLETED, async (event) => {
      const { sessionId, filePath } = event;
      if (sessionId && filePath) {
        await this.refreshFileState(sessionId, filePath);
      }
    });

    this.eventBus.on(SNAPSHOT_EVENTS.USER_ACCEPT_FILE, async (event) => {
      const { sessionId, filePath } = event;
      if (sessionId && filePath) {
        await this.handleUserFileAction(sessionId, filePath, 'accept');
      }
    });

    this.eventBus.on(SNAPSHOT_EVENTS.USER_REJECT_FILE, async (event) => {
      const { sessionId, filePath } = event;
      if (sessionId && filePath) {
        await this.handleUserFileAction(sessionId, filePath, 'reject');
      }
    });

    this.setupGlobalEventListeners();
  }

  private async setupGlobalEventListeners(): Promise<void> {
    const { globalEventBus } = await import('@/infrastructure/event-bus');
    
    globalEventBus.on('snapshot:rollback-completed', async (data: { sessionId: string; turnIndex: number; restoredFiles: string[] }) => {
      if (data.sessionId) {
        await this.refreshSessionState(data.sessionId);
      }
    });
  }

  getSessionState(sessionId: string): SessionState | null {
    return this.sessions.get(sessionId) || null;
  }

  getFileState(filePath: string): SnapshotFile | null {
    return this.files.get(filePath) || null;
  }

  getSessionFiles(sessionId: string): SnapshotFile[] {
    const sessionState = this.sessions.get(sessionId);
    if (!sessionState) {
      log.warn('Session not found', { sessionId });
      return [];
    }

    return Array.from(sessionState.files.values());
  }

  private calculateFileStatus(file: SnapshotFile): 'pending' | 'partial' | 'accepted' | 'rejected' {
    if (!file.diffBlocks || file.diffBlocks.length === 0) {
      return 'pending';
    }

    const pendingBlocks = file.diffBlocks.filter(block => block.status === 'pending');
    const acceptedBlocks = file.diffBlocks.filter(block => block.status === 'accepted');
    const rejectedBlocks = file.diffBlocks.filter(block => block.status === 'rejected');

    if (pendingBlocks.length > 0) {
      if (acceptedBlocks.length > 0 || rejectedBlocks.length > 0) {
        return 'partial';
      }
      return 'pending';
    }

    if (acceptedBlocks.length > 0 && rejectedBlocks.length === 0) {
      return 'accepted';
    } else if (rejectedBlocks.length > 0 && acceptedBlocks.length === 0) {
      return 'rejected';
    } else {
      return 'partial';
    }
  }

  async refreshSessionState(sessionId: string): Promise<void> {
    try {
      const stats = await this.snapshotService.getSessionStats(sessionId);
      
      const sessionState: SessionState = {
        sessionId,
        files: new Map(),
        totalBlocks: 0,
        acceptedBlocks: 0,
        rejectedBlocks: 0,
        pendingBlocks: 0,
        sessionStatus: stats.total_changes > 0 ? 'pending' : 'working',
        lastActivity: Date.now()
      };

      // NOTE: `getSessionStats` does not include file lists. Fetch files separately when available.
      try {
        const { snapshotAPI } = await import('@/infrastructure/api');
        const files = await snapshotAPI.getSessionFiles(sessionId);

        for (const filePath of files) {
          const snapshotFile: SnapshotFile = {
            filePath,
            sessionId,
            originalContent: '', // loaded on demand
            modifiedContent: '', // loaded on demand
            fileStatus: 'pending',
            lastModified: Date.now()
          };

          sessionState.files.set(filePath, snapshotFile);
          this.files.set(filePath, snapshotFile);
        }
      } catch (fileListError) {
        log.warn('Failed to get file list, using basic stats', { sessionId, error: fileListError });
      }

      this.sessions.set(sessionId, sessionState);
      this.eventBus.emit(SNAPSHOT_EVENTS.SESSION_STATE_CHANGED, sessionState, sessionId);
    } catch (error) {
      log.error('Failed to refresh session state', { sessionId, error });
    }
  }

  async refreshFileState(sessionId: string, filePath: string): Promise<void> {
    try {
      const response = await this.snapshotService.getOperationDiff(sessionId, filePath);
      
      const existingFile = this.files.get(filePath);
      const snapshotFile: SnapshotFile = {
        filePath,
        sessionId,
        originalContent: response.originalCode,
        modifiedContent: response.modifiedCode,
        fileStatus: existingFile?.fileStatus || 'pending',
        lastModified: Date.now()
      };

      this.files.set(filePath, snapshotFile);

      const sessionState = this.sessions.get(sessionId);
      if (sessionState) {
        sessionState.files.set(filePath, snapshotFile);
        sessionState.lastActivity = Date.now();
      }

      this.eventBus.emit(SNAPSHOT_EVENTS.FILE_STATE_CHANGED, snapshotFile, sessionId, filePath);
      
    } catch (error) {
      log.error('Failed to refresh file state', { sessionId, filePath, error });
    }
  }

  async handleUserFileAction(sessionId: string, filePath: string, action: 'accept' | 'reject'): Promise<void> {
    try {
      if (action === 'accept') {
        await this.snapshotService.acceptFileModifications(sessionId, filePath);
      } else {
        await this.snapshotService.rejectFileModifications(sessionId, filePath);
      }

      const file = this.files.get(filePath);
      if (file) {
        file.fileStatus = action === 'accept' ? 'accepted' : 'rejected';
        file.lastModified = Date.now();

        this.eventBus.emit(SNAPSHOT_EVENTS.FILE_STATE_CHANGED, file, sessionId, filePath);
      }

      await this.refreshSessionState(sessionId);
      
    } catch (error) {
      log.error('User file action failed', { sessionId, filePath, action, error });
      throw error;
    }
  }

  async handleUserSessionAction(sessionId: string, action: 'accept' | 'reject'): Promise<void> {
    try {
      if (action === 'accept') {
        await this.snapshotService.acceptSessionModifications(sessionId);
      } else {
        await this.snapshotService.rejectSessionModifications(sessionId);
      }

      const sessionState = this.sessions.get(sessionId);
      if (sessionState) {
        sessionState.sessionStatus = 'completed';
        sessionState.lastActivity = Date.now();

        for (const file of sessionState.files.values()) {
          file.fileStatus = action === 'accept' ? 'accepted' : 'rejected';
          file.lastModified = Date.now();
        }
      }

      this.eventBus.emit(SNAPSHOT_EVENTS.SESSION_STATE_CHANGED, sessionState, sessionId);
      
    } catch (error) {
      log.error('User session action failed', { sessionId, action, error });
      throw error;
    }
  }

  onSessionStateChange(callback: (sessionState: SessionState) => void): () => void {
    return this.eventBus.on(SNAPSHOT_EVENTS.SESSION_STATE_CHANGED, (event) => {
      callback(event.payload);
    });
  }

  onFileStateChange(callback: (file: SnapshotFile) => void): () => void {
    return this.eventBus.on(SNAPSHOT_EVENTS.FILE_STATE_CHANGED, (event) => {
      callback(event.payload);
    });
  }

  clearSession(sessionId: string): void {
    const sessionState = this.sessions.get(sessionId);
    if (sessionState) {
      for (const filePath of sessionState.files.keys()) {
        this.files.delete(filePath);
      }
      
      this.sessions.delete(sessionId);
    }
  }

  getActiveSessions(): SessionState[] {
    return Array.from(this.sessions.values())
      .filter(session => session.sessionStatus !== 'completed');
  }

  async handleUserBlockAction(sessionId: string, filePath: string, blockId: string, action: 'accept' | 'reject'): Promise<void> {
    try {
      // Update in-memory state first for responsive UI; backend persistence is best-effort.
      const file = this.files.get(filePath);
      if (file && file.diffBlocks) {
        const block = file.diffBlocks.find(b => b.id === blockId);
        if (block) {
          block.status = action === 'accept' ? 'accepted' : 'rejected';
          
          file.fileStatus = this.calculateFileStatus(file);
          
          this.files.set(filePath, file);
          
          const sessionState = this.sessions.get(sessionId);
          if (sessionState) {
            sessionState.files.set(filePath, file);
            
            let totalBlocks = 0;
            let pendingBlocks = 0;
            let acceptedBlocks = 0;
            let rejectedBlocks = 0;
            
            for (const sessionFile of sessionState.files.values()) {
              if (sessionFile.diffBlocks) {
                totalBlocks += sessionFile.diffBlocks.length;
                pendingBlocks += sessionFile.diffBlocks.filter(b => b.status === 'pending').length;
                acceptedBlocks += sessionFile.diffBlocks.filter(b => b.status === 'accepted').length;
                rejectedBlocks += sessionFile.diffBlocks.filter(b => b.status === 'rejected').length;
              }
            }
            
            sessionState.totalBlocks = totalBlocks;
            sessionState.pendingBlocks = pendingBlocks;
            sessionState.acceptedBlocks = acceptedBlocks;
            sessionState.rejectedBlocks = rejectedBlocks;
            
            if (pendingBlocks === 0) {
              sessionState.sessionStatus = 'completed';
            } else if (acceptedBlocks > 0 || rejectedBlocks > 0) {
              sessionState.sessionStatus = 'partial';
            } else {
              sessionState.sessionStatus = 'pending';
            }
            
            sessionState.lastActivity = Date.now();
            this.sessions.set(sessionId, sessionState);
            
            this.eventBus.emit(SNAPSHOT_EVENTS.SESSION_STATE_CHANGED, sessionState, sessionId);
            this.eventBus.emit(SNAPSHOT_EVENTS.FILE_STATE_CHANGED, file);
          }
        }
      }
      
      try {
        if (action === 'accept') {
          await this.snapshotService.acceptDiffBlock(sessionId, filePath, blockId);
        } else {
          await this.snapshotService.rejectDiffBlock(sessionId, filePath, blockId);
        }
      } catch (backendError) {
        log.warn('Backend diff block operation failed, but frontend state updated', { sessionId, filePath, blockId, action, error: backendError });
      }
      
    } catch (error) {
      log.error('Failed to handle user diff block action', { sessionId, filePath, blockId, action, error });
      throw error;
    }
  }
}
