use std::collections::BTreeMap;

use eyre::Context;

use crate::doc::intermediate::{Column, ForeignKey, Index, IntermediateSchema, Table};

// The only Postgres schema we will export. TODO on making it configurable.
const SCHEMA_NAME: &str = "public";

pub fn gather_schema(db: &mut postgres::Client) -> eyre::Result<IntermediateSchema> {
    let mut out = IntermediateSchema {
        tables: BTreeMap::new(),
    };

    for row in db
        .query(
            "SELECT quote_ident(tablename) FROM pg_catalog.pg_tables WHERE schemaname = $1",
            &[&SCHEMA_NAME],
        )
        .context("failed to list tables")?
    {
        let table_name: String = row.get(0);
        out.tables.insert(
            table_name.clone(),
            gather_table(db, &table_name)
                .with_context(|| format!("failed to gather table {table_name:?}"))?,
        );
    }

    Ok(out)
}

pub fn gather_table(db: &mut postgres::Client, table_name: &str) -> eyre::Result<Table> {
    // Calculate the schema-qualified name, just so we can cast it to a regclass without any risk of name clashes.
    let qual_name = format!("{SCHEMA_NAME}.{table_name}");
    let comment: String = db
        .query_one(
            "SELECT COALESCE(pg_catalog.obj_description($1::text::regclass::oid, 'pg_class'), '')",
            &[&qual_name],
        )
        .context("failed to get table comment")?
        .get(0);

    let mut columns = Vec::new();
    for row in db
        .query(
            "
            SELECT column_name, data_type, is_nullable, COALESCE(pg_catalog.col_description($3::text::regclass::oid, ordinal_position), '')
            FROM information_schema.columns
            WHERE table_schema = $1 AND table_name = $2
            ORDER BY ordinal_position
        ",
            &[&SCHEMA_NAME, &table_name, &qual_name],
        )
        .context("failed to list columns")?
    {
        columns.push(Column {
            name: row.get(0),
            r#type: row.get(1),
            not_null: row.get::<_, String>(2).as_str() == "NO",
            comment: row.get(3),
        });
    }

    let mut indices = Vec::new();
    for row in db
        // .query("SELECT indexname, indexdef FROM pg_catalog.pg_indexes WHERE schemaname = $1 AND tablename = $2", &[&SCHEMA_NAME, &table_name])
        .query("SELECT c2.relname, COALESCE(pg_catalog.pg_get_constraintdef(con.oid, true), pg_catalog.pg_get_indexdef(i.indexrelid, 0, true))
FROM pg_catalog.pg_class c, pg_catalog.pg_class c2, pg_catalog.pg_index i
  LEFT JOIN pg_catalog.pg_constraint con ON (conrelid = i.indrelid AND conindid = i.indexrelid AND contype IN ('p','u','x'))
WHERE c.oid = $1::text::regclass AND c.oid = i.indrelid AND i.indexrelid = c2.oid
ORDER BY i.indisprimary DESC, c2.relname;", &[&qual_name])
        .context("failed to list indices")?
    {
        indices.push(Index {
            name: row.get(0),
            def: row.get(1),
        });
    }

    let mut foreign_keys = Vec::new();
    for row in db
        .query(
            "SELECT conname, pg_catalog.pg_get_constraintdef(r.oid, true) AS condef, foreigntbl.relname AS foreign_name
        FROM pg_catalog.pg_constraint r
        INNER JOIN pg_class AS foreigntbl ON foreigntbl.oid = r.confrelid
        WHERE r.conrelid = $1::text::regclass AND r.contype = 'f' ORDER BY conname",
            &[&qual_name],
        )
        .context("failed to query for forward foreign key links")?
    {
        foreign_keys.push(ForeignKey {
            name: row.get(0),
            referrer_table: table_name.to_owned(),
            referee_table: row.get(2),
            def: row.get(1),
        });
    }

    let mut foreign_key_backlinks = Vec::new();
    for row in db
        .query(
            "SELECT conname, pg_catalog.pg_get_constraintdef(r.oid, true) AS condef, origin.relname AS origin_name
        FROM pg_catalog.pg_constraint r
        INNER JOIN pg_class AS origin ON origin.oid = r.conrelid
        WHERE r.confrelid = $1::text::regclass AND r.contype = 'f' ORDER BY origin_name, conname",
            &[&qual_name],
        )
        .context("failed to query for forward foreign key links")?
    {
        foreign_key_backlinks.push(ForeignKey {
            name: row.get(0),
            referrer_table: row.get(2),
            referee_table: table_name.to_owned(),
            def: row.get(1),
        });
    }

    Ok(Table {
        name: table_name.to_owned(),
        comment,
        columns,
        indices,
        foreign_keys,
        foreign_key_backlinks,
    })
}
