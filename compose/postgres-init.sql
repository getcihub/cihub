-- Development only. CIHub connects as `cihub`, which is not a superuser and
-- cannot bypass row level security (AGENTS.md D38). CREATEDB lets
-- `#[sqlx::test]` create one database per test.
CREATE ROLE cihub LOGIN PASSWORD 'cihub' CREATEDB;

CREATE DATABASE cihub OWNER cihub;
