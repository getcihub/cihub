-- Lot 8. A queue has a name, unique in its repository, so that several queues can
-- target one branch (AGENTS.md D135). It keeps the last valid configuration file
-- read on the head of its target (D136).
ALTER TABLE queues
    DROP CONSTRAINT queues_repo_id_target_branch_key,
    ADD COLUMN name text NOT NULL,
    -- The text of the last valid file; NULL when the target has no file.
    ADD COLUMN config_text text,
    -- The head of the target at the last read, valid or not.
    ADD COLUMN config_sha text,
    -- Why the file read at config_sha is not valid; NULL when it is.
    ADD COLUMN config_error text,
    ADD UNIQUE (repo_id, name);
