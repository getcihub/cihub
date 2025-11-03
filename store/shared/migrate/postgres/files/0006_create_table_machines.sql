-- name: create-table-machines

CREATE TABLE IF NOT EXISTS machines (
  machine_name      VARCHAR(255) PRIMARY KEY,
  machine_owner     VARCHAR(255),
  machine_arch      VARCHAR(50),
  machine_cpu       BIGINT,
  machine_ram       BIGINT,
  machine_status    VARCHAR(50),
  machine_created   BIGINT,
  machine_last_seen BIGINT,
  machine_updated   BIGINT,
  machine_token     TEXT,
  UNIQUE(machine_token)
);

CREATE INDEX IF NOT EXISTS ix_machine_owner ON machines (machine_owner);
CREATE INDEX IF NOT EXISTS ix_machine_status ON machines (machine_status);
CREATE INDEX IF NOT EXISTS ix_machine_last_seen ON machines (machine_last_seen);
