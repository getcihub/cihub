-- name: create-table-machines

CREATE TABLE IF NOT EXISTS machines (
  machine_name           TEXT,
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

  PRIMARY KEY(machine_name, machine_owner)
  UNIQUE(machine_token)
);

-- name: create-index-machines-owner

CREATE INDEX IF NOT EXISTS ix_machine_owner ON machines (machine_owner);

-- name: create-index-machines-status

CREATE INDEX IF NOT EXISTS ix_machine_status ON machines (machine_status);

-- name: create-index-machines-last-seen

CREATE INDEX IF NOT EXISTS ix_machine_last_seen ON machines (machine_last_seen);

-- name: alter-table-machines-add-column-labels

ALTER TABLE machines ADD COLUMN machine_labels TEXT DEFAULT '';
