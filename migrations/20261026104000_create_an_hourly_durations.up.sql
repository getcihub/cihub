-- A histogram of the durations of the merged PRs: the time to merge, and the
-- time in each live state, 0 for a state that a PR skipped. Bucket 0 holds no
-- time; bucket `b` the durations from 1.1^(b - 1) to 1.1^b seconds, so that a
-- percentile read from it is within 5 % of the exact one.
CREATE TABLE an_hourly_durations (
    org_id   text        NOT NULL REFERENCES orgs (id),
    queue_id text        NOT NULL,
    hour     timestamptz NOT NULL,
    measure  text        NOT NULL CHECK (measure IN ('time_to_merge', 'not_ready', 'pending',
                                                     'testing', 'pending_failure',
                                                     'tests_passed')),
    bucket   smallint    NOT NULL CHECK (bucket >= 0),
    count    bigint      NOT NULL,
    PRIMARY KEY (queue_id, hour, measure, bucket),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

SELECT cihub_tenant_table('an_hourly_durations');
