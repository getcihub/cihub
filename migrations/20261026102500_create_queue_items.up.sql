CREATE TYPE submitted_via AS ENUM ('comment', 'label', 'api');

CREATE TABLE queue_items (
    id                 text          PRIMARY KEY,
    org_id             text          NOT NULL REFERENCES orgs (id),
    queue_id           text          NOT NULL,
    pr_number          bigint        NOT NULL,
    pr_head_sha        text          NOT NULL,
    pr_url             text,
    pr_author_id       text          NOT NULL REFERENCES users (id),
    state              item_state    NOT NULL,
    state_reason       reason        NOT NULL,
    state_changed_at   timestamptz   NOT NULL,
    position           bigint,
    priority_value     smallint      NOT NULL,
    no_batch           boolean       NOT NULL,
    profile            text,
    submitted_by       text          REFERENCES users (id),
    submitted_via      submitted_via NOT NULL,
    sender_permission  permission,
    submitted_at       timestamptz   NOT NULL,
    block_reasons      jsonb         NOT NULL,
    group_id           text,
    is_current         boolean       NOT NULL,
    verified_by_run_id text,
    merged_sha         text,
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

CREATE UNIQUE INDEX queue_items_current ON queue_items (queue_id, pr_number) WHERE is_current;
CREATE INDEX queue_items_pr ON queue_items (queue_id, pr_number);
CREATE INDEX queue_items_order ON queue_items (queue_id, position)
    WHERE is_current AND state NOT IN ('merged', 'failed', 'cancelled');
CREATE INDEX queue_items_author_live ON queue_items (pr_author_id)
    WHERE is_current AND state NOT IN ('merged', 'failed', 'cancelled');
CREATE INDEX queue_items_author_merged ON queue_items (pr_author_id, state_changed_at)
    WHERE is_current AND state = 'merged';
-- The counters of the last 24 hours, the last merge and the tabs of the ended items.
CREATE INDEX queue_items_ended ON queue_items (queue_id, state_changed_at)
    WHERE is_current AND state IN ('merged', 'failed', 'cancelled');
-- The lists of the PRs of an organization, the newest first.
CREATE INDEX queue_items_newest ON queue_items (org_id, submitted_at DESC, id DESC) WHERE is_current;

SELECT cihub_tenant_table('queue_items');
