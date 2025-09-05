/*
 * Copyright (c) 2019-2025. Ivan Vakhrushev and others.
 * https://github.com/mfvanek/pg-index-health-sql
 *
 * Licensed under the Apache License 2.0
 *
 * Modified for `timestamp without time zone` type.
 */

select
    t.oid::regclass::text as table_name,
    quote_ident(col.attname) as column_name
from
    pg_catalog.pg_class t
    inner join pg_catalog.pg_namespace nsp on nsp.oid = t.relnamespace
    inner join pg_catalog.pg_attribute col on col.attrelid = t.oid
where
    t.relkind in ('r', 'p') and
    not t.relispartition and
    col.attnum > 0 and /* to filter out system columns */
    not col.attisdropped and
    col.atttypid = 'timestamp without time zone'::regtype and
    nsp.nspname = 'public' -- TODO
order by table_name, column_name;
