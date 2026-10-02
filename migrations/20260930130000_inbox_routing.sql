-- The routing of the deliveries (DECISIONS.md D84 to D86). The web role stores a
-- delivery without routing it; a loop translates it, finds its connection and
-- its queue, then sets `routed_at`. A delivery whose connection is unknown is
-- routed without an organization: only the system context sees it.

ALTER TABLE webhook_inbox ADD COLUMN routed_at timestamptz;

CREATE INDEX webhook_inbox_unrouted ON webhook_inbox (received_at)
    WHERE routed_at IS NULL;
