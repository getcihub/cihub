-- Lot 9. What each PR of a queue shows (DECISIONS.md D156): the rendering that the
-- reconciliation wants, and what the SCM shows. No event: like
-- check_observations, the comment of the SCM is not a state of the queue.
CREATE TABLE pr_surfaces (
    org_id              text        NOT NULL REFERENCES orgs (id),
    queue_id            text        NOT NULL,
    pr_number           bigint      NOT NULL,
    desired             jsonb       NOT NULL,
    desired_hash        text        NOT NULL,
    -- `NULL` until a job published a rendering.
    published_hash      text,
    PRIMARY KEY (queue_id, pr_number),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

CREATE INDEX pr_surfaces_unpublished ON pr_surfaces (queue_id)
    WHERE published_hash IS DISTINCT FROM desired_hash;

SELECT cihub_tenant_table('pr_surfaces');

-- The comment of CIHub on a PR belongs to the PR, not to a queue (D163): the
-- welcome comment of a PR becomes the comment of the state of its item.
CREATE TABLE pr_comments (
    org_id              text   NOT NULL REFERENCES orgs (id),
    repo_id             text   NOT NULL,
    pr_number           bigint NOT NULL,
    comment_external_id text   NOT NULL,
    PRIMARY KEY (repo_id, pr_number),
    FOREIGN KEY (org_id, repo_id) REFERENCES repos (org_id, id)
);

SELECT cihub_tenant_table('pr_comments');
