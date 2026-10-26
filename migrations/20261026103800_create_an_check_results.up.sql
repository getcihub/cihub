-- The last report of each check on the test commit of an ended run;
-- `passed_on_retry` when it passed after it failed on the attempt before.
CREATE TABLE an_check_results (
    org_id          text             NOT NULL REFERENCES orgs (id),
    run_id          text             NOT NULL REFERENCES an_test_run_results (run_id),
    name            text             NOT NULL,
    kind            check_kind       NOT NULL,
    required        boolean          NOT NULL,
    conclusion      check_conclusion,
    passed_on_retry boolean          NOT NULL,
    PRIMARY KEY (run_id, name, kind)
);

SELECT cihub_tenant_table('an_check_results');
