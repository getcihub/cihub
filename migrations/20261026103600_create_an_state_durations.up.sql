-- One row per pass of a concluded item through a state; `pass` counts them
-- from 0, as one reconciliation can move an item twice at the same time.
CREATE TABLE an_state_durations (
    org_id      text        NOT NULL REFERENCES orgs (id),
    queue_id    text        NOT NULL,
    item_id     text        NOT NULL REFERENCES an_item_conclusions (item_id),
    pass        integer     NOT NULL,
    state       item_state  NOT NULL,
    entered_at  timestamptz NOT NULL,
    left_at     timestamptz NOT NULL,
    exit_reason reason      NOT NULL,
    PRIMARY KEY (item_id, pass),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

SELECT cihub_tenant_table('an_state_durations');
