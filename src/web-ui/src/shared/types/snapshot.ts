/**
 * Snapshot subsystem types (frontend DTOs).
 *
 * Uses snake_case to match backend payloads.
 */
export interface FileModificationState {
  file_path: string;
  status: 'created' | 'modified' | 'deleted' | 'unchanged';
  lines_added: number;
  lines_removed: number;
  last_turn: number;
  operations: string[];
}

 
export interface SessionFileStatesResponse {
  session_id: string;
  files: FileModificationState[];
}

 
export interface DialogTurnSnapshot {
  turn_id: string;
  turn_index: number;
  timestamp: number;
  user_message: string;
  ai_response_summary?: string;
  operations: string[];
  cumulative_stats: {
    total_files_modified: number;
    total_lines_added: number;
    total_lines_removed: number;
    files_created: string[];
    files_deleted: string[];
  };
}

 
export type SnapshotEventData =
  | SessionCreatedEvent
  | FileStateUpdatedEvent
  | DialogTurnCompletedEvent
  | DiffStateUpdatedEvent;

export interface SessionCreatedEvent {
  session_id: string;
  agent_type: string;
  timestamp: number;
}

export interface FileStateUpdatedEvent {
  session_id: string;
  file_path: string;
  status: 'created' | 'modified' | 'deleted';
  lines_added: number;
  lines_removed: number;
  timestamp: number;
}

export interface DialogTurnCompletedEvent {
  session_id: string;
  turn_id: string;
  turn_index: number;
  files_changed: number;
  lines_added: number;
  lines_removed: number;
  timestamp: number;
}

export interface DiffStateUpdatedEvent {
  session_id: string;
  total_files_modified: number;
  total_lines_added: number;
  total_lines_removed: number;
  timestamp: number;
}
