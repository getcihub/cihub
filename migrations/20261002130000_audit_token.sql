-- Lot 9. The API token that made a change (AGENTS.md D160): an action of a
-- token has no user.
ALTER TABLE audit_log ADD COLUMN actor_token_id text REFERENCES api_tokens (id);
