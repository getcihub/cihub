CREATE TABLE queue_inbox (
    id            text        PRIMARY KEY,
    org_id        text        NOT NULL REFERENCES orgs (id),
    queue_id      text        NOT NULL,
    actor_kind    actor_kind  NOT NULL,
    actor_user_id text        REFERENCES users (id),
    type          event_type  NOT NULL,
    payload       jsonb       NOT NULL,
    received_at   timestamptz NOT NULL,
    arrival       bigint      GENERATED ALWAYS AS IDENTITY,
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id),
    CHECK ((actor_kind = 'user') = (actor_user_id IS NOT NULL))
);

CREATE INDEX queue_inbox_pending ON queue_inbox (queue_id, arrival);

SELECT cihub_tenant_table('queue_inbox');
