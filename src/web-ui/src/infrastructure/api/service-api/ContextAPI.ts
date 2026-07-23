 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export interface ContextStats {
  sessionId: string;
  currentTokenCount: number;
  totalTokenCount: number;
  messageCount: number;
  turnCount: number;
  compressionCount: number;
  totalMessagesCompressed: number;
  totalTokensSaved: number;
  lastCompressionTime?: string;
}

export interface SessionMetadata {
  sessionId: string;
  title: string;
  createdAt: string;
  lastAccessed: string;
  messageCount: number;
  totalTokens: number;
  tags: string[];
  isArchived: boolean;
  todos?: any[]; 
}

export interface StorageStats {
  totalSessions: number;
  activeSessions: number;
  archivedSessions: number;
  totalSizeBytes: number;
  totalMessages: number;
  totalTokens: number;
  storagePath: string;
  lastCleanup?: string;
}

export class ContextAPI {
   
  async compressContext(): Promise<string> {
    try {
      return await api.invoke('compress_context', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('compress_context', error);
    }
  }

   
  async getContextStats(): Promise<ContextStats> {
    try {
      return await api.invoke('get_context_stats', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('get_context_stats', error);
    }
  }

   
  async clearContext(): Promise<string> {
    try {
      return await api.invoke('clear_context', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('clear_context', error);
    }
  }

   
  async saveSessionData(sessionData: any): Promise<string> {
    try {
      return await api.invoke('save_session_data', { 
        request: { sessionData } 
      });
    } catch (error) {
      throw createTauriCommandError('save_session_data', error, { sessionData });
    }
  }

   
  async loadSessionData(sessionId: string): Promise<any> {
    try {
      return await api.invoke('load_session_data', { 
        request: { sessionId } 
      });
    } catch (error) {
      throw createTauriCommandError('load_session_data', error, { sessionId });
    }
  }

   
  async listSessions(includeArchived: boolean = false): Promise<SessionMetadata[]> {
    try {
      return await api.invoke('list_sessions', { 
        request: { includeArchived } 
      });
    } catch (error) {
      throw createTauriCommandError('list_sessions', error, { includeArchived });
    }
  }

   
  async searchSessions(query: string, tags?: string[]): Promise<SessionMetadata[]> {
    try {
      return await api.invoke('search_sessions', { 
        request: { query, tags } 
      });
    } catch (error) {
      throw createTauriCommandError('search_sessions', error, { query, tags });
    }
  }

   
  async deleteSession(sessionId: string): Promise<string> {
    try {
      return await api.invoke('delete_session', { 
        request: { sessionId } 
      });
    } catch (error) {
      throw createTauriCommandError('delete_session', error, { sessionId });
    }
  }

   
  async archiveSession(sessionId: string): Promise<string> {
    try {
      return await api.invoke('archive_session', { 
        request: { sessionId } 
      });
    } catch (error) {
      throw createTauriCommandError('archive_session', error, { sessionId });
    }
  }

   
  async exportSession(sessionId: string, exportPath: string): Promise<string> {
    try {
      return await api.invoke('export_session', { 
        request: { sessionId, exportPath } 
      });
    } catch (error) {
      throw createTauriCommandError('export_session', error, { sessionId, exportPath });
    }
  }

   
  async importSession(importPath: string): Promise<string> {
    try {
      return await api.invoke('import_session', { 
        request: { importPath } 
      });
    } catch (error) {
      throw createTauriCommandError('import_session', error, { importPath });
    }
  }

   
  async getStorageStats(): Promise<StorageStats> {
    try {
      return await api.invoke('get_storage_stats', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('get_storage_stats', error);
    }
  }
}


export const contextAPI = new ContextAPI();