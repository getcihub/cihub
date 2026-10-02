-- Lot 8. The permission of the author of a command, read at the submission, so
-- that a read of the PR evaluates `sender-permission` (DECISIONS.md D144, D145).
CREATE TYPE permission AS ENUM ('none', 'read', 'write', 'admin');

ALTER TABLE queue_items
    -- NULL for a submission by label.
    ADD COLUMN sender_permission permission;
