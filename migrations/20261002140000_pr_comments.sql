-- Lot 9. The comment of CIHub on a PR belongs to the PR, not to a queue
-- (AGENTS.md D163): the welcome comment of a PR becomes the comment of the state
-- of its item.
CREATE TABLE pr_comments (
    org_id              text   NOT NULL REFERENCES orgs (id),
    repo_id             text   NOT NULL,
    pr_number           bigint NOT NULL,
    comment_external_id text   NOT NULL,
    PRIMARY KEY (repo_id, pr_number),
    FOREIGN KEY (org_id, repo_id) REFERENCES repos (org_id, id)
);

INSERT INTO pr_comments (org_id, repo_id, pr_number, comment_external_id)
SELECT s.org_id, q.repo_id, s.pr_number, s.comment_external_id
FROM pr_surfaces s JOIN queues q ON q.id = s.queue_id
WHERE s.comment_external_id IS NOT NULL
ON CONFLICT DO NOTHING;

ALTER TABLE pr_surfaces DROP COLUMN comment_external_id;

ALTER TABLE pr_comments ENABLE ROW LEVEL SECURITY;
ALTER TABLE pr_comments FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant ON pr_comments
    USING (cihub_system() OR org_id = cihub_org_id())
    WITH CHECK (cihub_system() OR org_id = cihub_org_id());
