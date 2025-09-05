-- SPDX-FileCopyrightText: 2025 Olivier 'reivilibre'
--
-- SPDX-License-Identifier: Apache-2.0

-- This lint rule is part of the pgCrab project.

-- Find tables that don't have a specific replica identity set,
-- and also don't have a primary key.

WITH tables_no_pkey AS (
    SELECT tbl.table_schema, tbl.table_name
    FROM information_schema.tables tbl
    WHERE table_type = 'BASE TABLE'
        AND table_schema = 'public' /* TODO */
        AND NOT EXISTS (
            SELECT 1
            FROM information_schema.key_column_usage kcu
            WHERE kcu.table_name = tbl.table_name
                AND kcu.table_schema = tbl.table_schema
        )
)
SELECT oid::regclass::text FROM tables_no_pkey INNER JOIN pg_class ON oid::regclass = quote_ident(table_name)::regclass
-- d = default
WHERE relreplident = 'd';
