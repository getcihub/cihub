-- name: create-table-machines

CREATE TABLE IF NOT EXISTS machines (
  machine_name           TEXT PRIMARY KEY COLLATE NOCASE,
  machine_owner          TEXT,
  machine_arch           TEXT,
  machine_cpu            INTEGER,
  machine_cpu_limit      INTEGER,
  machine_cpu_allocated  INTEGER,
  machine_ram_total      INTEGER,
  machine_ram_available  INTEGER,
  machine_ram_limit      INTEGER,
  machine_ram_allocated  INTEGER,
  machine_status         TEXT,
  machine_created        INTEGER,
  machine_last_seen      INTEGER,
  machine_updated        INTEGER,
  machine_token          TEXT,
  UNIQUE(machine_token)
);

CREATE INDEX IF NOT EXISTS ix_machine_owner ON machines (machine_owner);
CREATE INDEX IF NOT EXISTS ix_machine_status ON machines (machine_status);
CREATE INDEX IF NOT EXISTS ix_machine_last_seen ON machines (machine_last_seen);
