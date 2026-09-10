CREATE TABLE stage1_outputs (
  thread_id TEXT PRIMARY KEY NOT NULL,
  workspace_path TEXT NOT NULL,
  rollout_path TEXT NOT NULL,
  source_updated_at INTEGER NOT NULL,
  raw_memory TEXT NOT NULL,
  rollout_summary TEXT NOT NULL,
  rollout_slug TEXT,
  generated_at INTEGER NOT NULL,
  usage_count INTEGER,
  last_usage INTEGER,
  selected_for_phase2 INTEGER NOT NULL DEFAULT 0,
  selected_for_phase2_source_updated_at INTEGER
);

CREATE INDEX idx_stage1_outputs_source_updated_at
  ON stage1_outputs(source_updated_at DESC, thread_id DESC);

CREATE TABLE jobs (
  kind TEXT NOT NULL,
  job_key TEXT NOT NULL,
  status TEXT NOT NULL,
  worker_id TEXT,
  ownership_token TEXT,
  started_at INTEGER,
  finished_at INTEGER,
  lease_until INTEGER,
  retry_at INTEGER,
  retry_remaining INTEGER NOT NULL,
  last_error TEXT,
  input_watermark INTEGER,
  last_success_watermark INTEGER,
  PRIMARY KEY (kind, job_key)
);

CREATE INDEX idx_jobs_kind_status_retry_lease
  ON jobs(kind, status, retry_at, lease_until);

INSERT INTO stage1_outputs VALUES (
  'session-1',
  'C:\fixture-workspace',
  'C:\fixture-workspace\sessions\session-1',
  1,
  'Synthetic migration fixture memory.',
  'Synthetic migration fixture summary.',
  'fixture-memory',
  2,
  0,
  NULL,
  0,
  NULL
);

INSERT INTO jobs (
  kind, job_key, status, retry_remaining, input_watermark,
  last_success_watermark
) VALUES ('memory_stage1', 'session-1', 'done', 3, 1, 1);
