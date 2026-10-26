CREATE FUNCTION cihub_org_id() RETURNS text
LANGUAGE sql STABLE
AS $$ SELECT nullif(current_setting('cihub.org_id', true), '') $$;

CREATE FUNCTION cihub_system() RETURNS boolean
LANGUAGE sql STABLE
AS $$ SELECT coalesce(current_setting('cihub.system', true), '') = 'on' $$;

CREATE FUNCTION cihub_tenant_table(tenant_table regclass) RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
    tenant_column text := CASE
        WHEN EXISTS (
            SELECT 1 FROM pg_attribute
            WHERE attrelid = tenant_table AND attname = 'org_id' AND NOT attisdropped
        ) THEN 'org_id'
        ELSE 'id'
    END;
BEGIN
    EXECUTE format('ALTER TABLE %s ENABLE ROW LEVEL SECURITY', tenant_table);
    EXECUTE format('ALTER TABLE %s FORCE ROW LEVEL SECURITY', tenant_table);
    EXECUTE format(
        'CREATE POLICY tenant ON %s
            USING (cihub_system() OR %I = cihub_org_id())
            WITH CHECK (cihub_system() OR %I = cihub_org_id())',
        tenant_table, tenant_column, tenant_column
    );
END
$$;
