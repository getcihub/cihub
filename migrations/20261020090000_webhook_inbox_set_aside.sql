ALTER TABLE webhook_inbox
    ADD COLUMN attempts     integer     NOT NULL DEFAULT 0,
    ADD COLUMN set_aside_at timestamptz;

DROP INDEX webhook_inbox_pending;

CREATE INDEX webhook_inbox_pending ON webhook_inbox (queue_id, received_at)
    WHERE set_aside_at IS NULL;
