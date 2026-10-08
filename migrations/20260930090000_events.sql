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
    queue_id      text        NOT NULL,
    seq           bigint      NOT NULL CHECK (seq > 0),
    type          event_type  NOT NULL,
    actor_kind    actor_kind  NOT NULL,
    actor_user_id text        REFERENCES users (id),
    cause_id      text,
    item_id       text,
    run_id        text,
    payload       jsonb       NOT NULL,
    occurred_at   timestamptz NOT NULL,
    PRIMARY KEY (id, occurred_at),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id),
    CHECK ((actor_kind = 'user') = (actor_user_id IS NOT NULL))
) PARTITION BY RANGE (occurred_at);

CREATE INDEX events_queue_id_seq ON events (queue_id, seq);
CREATE INDEX events_item_id_occurred_at ON events (item_id, occurred_at) WHERE item_id IS NOT NULL;
CREATE INDEX events_run_id ON events (run_id) WHERE run_id IS NOT NULL;

SELECT cihub_tenant_table('events');

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
    PERFORM cihub_tenant_table(partition::regclass);
END
$$;
