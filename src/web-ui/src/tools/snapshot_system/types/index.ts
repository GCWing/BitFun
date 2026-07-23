export enum ConflictResolution {
  ACCEPT_OTHER = 'accept_other',
  WAIT = 'wait',
  CANCEL = 'cancel'
}

// Tool names used by the backend snapshot system.
export const FILE_OPERATION_TOOLS = [
  'Write',
  'Edit', 
  'Delete'
] as const;

export type FileOperationTool = typeof FILE_OPERATION_TOOLS[number];

export interface FileLock {
  session_id: string;
  locked_at: string;
  operation_type: string;
  tool_name: string;
}

export interface FileLockStatus {
  locks: Record<string, FileLock>;
  waiting_queue: Record<string, Array<{
    session_id: string;
    requested_at: string;
  }>>;
}

export interface SessionSimpleState {
  session_id: string;
  state: 'working' | 'pending' | 'completed';
  locked_files: string[];
  locked_files_count: number;
}

export interface ConflictInfo {
  conflicting_file: string;
  current_session: string;
  blocking_session: string;
  blocking_operation: {
    tool_name: string;
    locked_at: string;
    operation_type: string;
  };
}
