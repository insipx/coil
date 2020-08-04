
CREATE TABLE _tasks (
  id BIGSERIAL NOT NULL UNIQUE,
  priority int NOT NULL,
  job_type varchar NOT NULL,
  sync boolean NOT NULL,
  data bytea NOT NULL,
)
