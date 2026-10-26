CREATE TYPE device_code_state AS ENUM ('pending', 'approved', 'denied');

-- A sign-in of the command line in progress. It belongs to no organization
-- yet, so it is global, as `sessions`; the command line deletes the row when
-- it takes its session, and the maintenance deletes the expired ones.
CREATE TABLE device_codes (
    id               text              PRIMARY KEY,
    device_code_hash bytea             NOT NULL UNIQUE CHECK (length(device_code_hash) = 32),
    user_code        text              NOT NULL UNIQUE,
    state            device_code_state NOT NULL DEFAULT 'pending',
    user_id          text              REFERENCES users (id),
    created_at       timestamptz       NOT NULL,
    expires_at       timestamptz       NOT NULL,
    polled_at        timestamptz,
    CHECK ((state = 'pending') = (user_id IS NULL))
);

CREATE INDEX device_codes_expires_at ON device_codes (expires_at);
