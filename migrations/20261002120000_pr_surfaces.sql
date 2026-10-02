-- Lot 9. What each PR of a queue shows (AGENTS.md D156): the rendering that the
-- reconciliation wants, and what the SCM shows. No event: like
-- check_observations, the comment of the SCM is not a state of the queue.
CREATE TABLE pr_surfaces (
    org_id              text        NOT NULL REFERENCES orgs (id),
    queue_id            text        NOT NULL,
    pr_number           bigint      NOT NULL,
    -- The item that the rendering shows.
    item_id             text        NOT NULL,
    desired             jsonb       NOT NULL,
    desired_hash        text        NOT NULL,
    -- `NULL` until a job published a rendering.
    published_hash      text,
    published_at        timestamptz,
    -- The comment of the queue on the PR, once created.
    comment_external_id text,
    PRIMARY KEY (queue_id, pr_number),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

CREATE INDEX pr_surfaces_unpublished ON pr_surfaces (queue_id)
    WHERE published_hash IS DISTINCT FROM desired_hash;

ALTER TABLE pr_surfaces ENABLE ROW LEVEL SECURITY;
ALTER TABLE pr_surfaces FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON pr_surfaces
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());
