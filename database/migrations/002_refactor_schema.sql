-- Migration 002: Refactor schema
-- Drops aggregated cache tables and cleans up old dynamic tables

-- Drop the aggregated cache table
DROP TABLE IF EXISTS ohlcv_aggregated;

-- Drop any old dynamic bars_* tables that may have been created at runtime
DO $$
DECLARE
    table_name text;
BEGIN
    FOR table_name IN 
        SELECT tablename 
        FROM pg_tables 
        WHERE schemaname = 'public' 
        AND tablename LIKE 'bars_%'
    LOOP
        EXECUTE 'DROP TABLE IF EXISTS ' || quote_ident(table_name);
    END LOOP;
END $$;
