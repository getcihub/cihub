CREATE TABLE sessions (
    id         text        PRIMARY KEY,
    user_id    text        NOT NULL REFERENCES users (id),
    token_hash bytea       NOT NULL UNIQUE CHECK (length(token_hash) = 32),
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL
);

CREATE INDEX sessions_expires_at ON sessions (expires_at);
