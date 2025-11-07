-- name: create-table-machines

CREATE TABLE IF NOT EXISTS machines (
  machine_name            VARCHAR(255),
  machine_owner           VARCHAR(255),
  machine_arch            VARCHAR(50),
  machine_cpu             BIGINT,
  machine_cpu_limit       BIGINT,
  machine_cpu_allocated   BIGINT,
  machine_ram_total       BIGINT,
  machine_ram_available   BIGINT,
  machine_ram_limit       BIGINT,
  machine_ram_allocated   BIGINT,
  machine_status          VARCHAR(50),
  machine_created         BIGINT,
  machine_last_seen       BIGINT,
  machine_updated         BIGINT,
  machine_token           TEXT,

  PRIMARY KEY(machine_name, machine_owner),
  UNIQUE(machine_token)
);

-- name: create-index-machines-owner

CREATE INDEX IF NOT EXISTS ix_machine_owner ON machines (machine_owner);

-- name: create-index-machines-status

CREATE INDEX IF NOT EXISTS ix_machine_status ON machines (machine_status);

-- name: create-index-machines-last-seen

CREATE INDEX IF NOT EXISTS ix_machine_last_seen ON machines (machine_last_seen);
