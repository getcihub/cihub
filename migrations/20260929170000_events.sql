-- The log of the queues (AGENTS.md D5): facts and decisions, append-only,
-- partitioned by month on occurred_at (D36). A user actor points to `users`,
-- which keeps the login (D42).

CREATE TYPE event_type AS ENUM (
    'item.submitted', 'item.cancel_requested', 'item.restart_requested', 'pr.observed',
    'target.pushed', 'check.observed', 'targets.received', 'test_branch.assembled',
    'test_branch.assembly_failed', 'merge.succeeded', 'merge.failed', 'queue.control_requested',
    'queue.settings_changed', 'access.lost', 'access.restored', 'item.transitioned',
    'item.readiness_changed', 'queue.renumbered', 'group.formed', 'group.transitioned',
    'run.transitioned', 'queue.transitioned', 'queue.window_changed'
);

CREATE TYPE actor_kind AS ENUM ('system', 'bot', 'user');

CREATE TABLE events (
    id            text        NOT NULL,
    org_id        text        NOT NULL REFERENCES orgs (id),
    -- The foreign key to `queues` arrives with the table (20260930090000_queues.sql).
    queue_id      text        NOT NULL,
    -- Rank in the log of the queue, from 1: the replay order.
    seq           bigint      NOT NULL CHECK (seq > 0),
    type          event_type  NOT NULL,
    actor_kind    actor_kind  NOT NULL,
    actor_user_id text        REFERENCES users (id),
    -- The fact that caused a decision.
    cause_id      text,
    item_id       text,
    group_id      text,
    run_id        text,
    -- The fields of the type, as `cihub-domain` serializes them.
    payload       jsonb       NOT NULL,
    occurred_at   timestamptz NOT NULL,
    PRIMARY KEY (id, occurred_at),
    CHECK ((actor_kind = 'user') = (actor_user_id IS NOT NULL))
) PARTITION BY RANGE (occurred_at);

CREATE INDEX events_queue_id_seq ON events (queue_id, seq);
CREATE INDEX events_item_id_occurred_at ON events (item_id, occurred_at) WHERE item_id IS NOT NULL;

ALTER TABLE events ENABLE ROW LEVEL SECURITY;
ALTER TABLE events FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON events
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());

-- Creates `events_<YYYY>_<MM>` for the UTC month of `month` if it is missing,
-- with its own forced policy: a query on the partition itself does not pass
-- through the policy of `events`. Two callers at the same time wait for each
-- other.
CREATE FUNCTION cihub_create_events_partition(month date) RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
    first_day timestamp := date_trunc('month', month::timestamp);
    partition text := format('events_%s', to_char(first_day, 'YYYY_MM'));
BEGIN
    PERFORM pg_advisory_xact_lock(hashtext('cihub_create_events_partition'));
    IF to_regclass(partition) IS NOT NULL THEN
        RETURN;
    END IF;
    EXECUTE format(
        'CREATE TABLE %I PARTITION OF events FOR VALUES FROM (%L) TO (%L)',
        partition,
        first_day AT TIME ZONE 'UTC',
        (first_day + interval '1 month') AT TIME ZONE 'UTC'
    );
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', partition);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', partition);
    EXECUTE format(
        'CREATE POLICY tenant ON %I
            USING (cihub_system() OR org_id = cihub_org_id())
            WITH CHECK (cihub_system() OR org_id = cihub_org_id())',
        partition
    );
END
$$;
