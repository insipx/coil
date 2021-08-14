
CREATE TABLE IF NOT EXISTS _background_tasks (
  id BIGSERIAL PRIMARY KEY NOT NULL,
  job_type TEXT NOT NULL,
  -- priority INTEGER NOT NULL,
  data JSONB NOT NULL,
  retries INTEGER NOT NULL DEFAULT 0,
  last_retry TIMESTAMP NOT NULL DEFAULT '1970-01-01',
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  locked_at timestamptz,
  locked_by integer
);
