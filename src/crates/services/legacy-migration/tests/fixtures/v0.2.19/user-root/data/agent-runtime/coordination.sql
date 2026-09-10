PRAGMA user_version = 2;
CREATE TABLE coordination_sessions (
  parent_session_id TEXT PRIMARY KEY,
  next_auto_agent_seq INTEGER NOT NULL DEFAULT 1,
  updated_at_ms INTEGER NOT NULL
);
CREATE TABLE agents (
  agent_pk INTEGER PRIMARY KEY AUTOINCREMENT,
  parent_session_id TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  child_session_id TEXT,
  next_bg_seq INTEGER NOT NULL DEFAULT 1,
  state TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL
);
CREATE TABLE background_tasks (
  task_pk INTEGER PRIMARY KEY AUTOINCREMENT,
  parent_session_id TEXT NOT NULL,
  agent_pk INTEGER NOT NULL,
  bg_task_id TEXT NOT NULL,
  bg_ordinal INTEGER NOT NULL,
  parent_dialog_turn_id TEXT NOT NULL,
  parent_tool_call_id TEXT NOT NULL,
  child_dialog_turn_id TEXT NOT NULL,
  status TEXT NOT NULL,
  error_code TEXT,
  error_message TEXT,
  execution_owner_token TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  terminal_at_ms INTEGER
);
CREATE TABLE swarm_trees (root_session_id TEXT PRIMARY KEY, created_at_ms INTEGER NOT NULL);
CREATE TABLE swarm_nodes (
  session_id TEXT PRIMARY KEY,
  root_session_id TEXT NOT NULL,
  parent_session_id TEXT,
  agent_type TEXT NOT NULL,
  depth INTEGER NOT NULL,
  created_at_ms INTEGER NOT NULL
);
INSERT INTO coordination_sessions VALUES ('session-1', 2, 1);
INSERT INTO agents VALUES (1, 'session-1', 'helper-1', 'session-child-1', 2, 'historical', 1);
INSERT INTO background_tasks VALUES (1, 'session-1', 1, 'bg-1', 1, 'turn-1', 'call-1', 'turn-child-1', 'completed', NULL, NULL, 'fixture-owner', 1, 2);
INSERT INTO swarm_trees VALUES ('session-1', 1);
INSERT INTO swarm_nodes VALUES ('session-1', 'session-1', NULL, 'Ultra', 0, 1);
INSERT INTO swarm_nodes VALUES ('session-child-1', 'session-1', 'session-1', 'researcher', 1, 1);
