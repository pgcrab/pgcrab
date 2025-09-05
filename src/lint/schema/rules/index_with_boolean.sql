/*
 * Copyright (c) 2019-2025. Ivan Vakhrushev and others.
 * https://github.com/mfvanek/pg-index-health-sql
 *
 * Licensed under the Apache License 2.0
 *
 * Modified for the pgCrab project:
 * - Only flag boolean columns if they are the only
 *   column in the index, because there can be good
 *   reasons to include them in composite indices.
 */

-- Finds indexes that contains boolean values.
select
    pi.indrelid::regclass::text as table_name,
    pi.indexrelid::regclass::text as index_name,
    col.attnotnull as column_not_null,
    quote_ident(col.attname) as column_name,
    pg_relation_size(pi.indexrelid) as index_size
from
    pg_catalog.pg_index pi
    inner join pg_catalog.pg_class pc on pc.oid = pi.indexrelid
    inner join pg_catalog.pg_namespace nsp on nsp.oid = pc.relnamespace
    inner join pg_catalog.pg_attribute col on col.attrelid = pi.indrelid
        -- original
        -- and col.attnum = any(pi.indkey)
        -- modified:
        and col.attnum = pi.indkey[0]
where
    nsp.nspname = 'public' /* TODO */ and
    not pi.indisunique and
    pi.indisready and
    pi.indisvalid and
    not pc.relispartition and
    array_length(pi.indkey, 1) = 1 and -- only one column in index
    col.atttypid = 'boolean'::regtype
order by table_name, index_name;
