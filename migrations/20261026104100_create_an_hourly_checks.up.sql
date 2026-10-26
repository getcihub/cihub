CREATE TABLE an_hourly_checks (
    org_id          text        NOT NULL REFERENCES orgs (id),
    queue_id        text        NOT NULL,
    hour            timestamptz NOT NULL,
    name            text        NOT NULL,
    kind            check_kind  NOT NULL,
    required        boolean     NOT NULL,
    runs            bigint      NOT NULL,
    failed          bigint      NOT NULL,
    passed_on_retry bigint      NOT NULL,
    PRIMARY KEY (queue_id, hour, name, kind),
    FOREIGN KEY (org_id, queue_id) REFERENCES queues (org_id, id)
);

SELECT cihub_tenant_table('an_hourly_checks');
