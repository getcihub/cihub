-- The cihub role and database of the development Postgres (DECISIONS.md D38,
-- D52), brought to the wanted state. compose.yaml and the CI run this file.
SELECT 'CREATE ROLE cihub'
WHERE NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'cihub') \gexec
ALTER ROLE cihub LOGIN PASSWORD 'cihub' CREATEDB NOSUPERUSER NOBYPASSRLS;
SELECT 'CREATE DATABASE cihub OWNER cihub'
WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'cihub') \gexec
ALTER DATABASE cihub OWNER TO cihub;
