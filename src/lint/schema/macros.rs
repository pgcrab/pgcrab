use postgres::Row;

use super::SchemaLoc;

pub(crate) fn loc_table(row: Row) -> SchemaLoc {
    SchemaLoc::Table { table: row.get(0) }
}

pub(crate) fn loc_object(row: Row) -> SchemaLoc {
    SchemaLoc::Object {
        object: row.get(0),
        kind: row.get(1),
    }
}

pub(crate) fn loc_column(row: Row) -> SchemaLoc {
    SchemaLoc::Column {
        table: row.get(0),
        column: row.get(1),
    }
}

pub(crate) fn loc_index(row: Row) -> SchemaLoc {
    SchemaLoc::Index {
        table: row.get(0),
        index: row.get(1),
    }
}

pub(crate) fn loc_indexes(row: Row) -> SchemaLoc {
    SchemaLoc::Indexes {
        table: row.get(0),
        indexes: row.get::<_, Vec<String>>(1).into_iter().collect(),
    }
}

pub(crate) fn loc_foreign_keys(row: Row) -> SchemaLoc {
    SchemaLoc::ForeignKeys {
        table: row.get(0),
        target_table: row.get(1),
        foreign_keys: row.get::<_, Vec<String>>(2).into_iter().collect(),
    }
}

pub(crate) fn loc_foreign_key(row: Row) -> SchemaLoc {
    SchemaLoc::ForeignKey {
        table: row.get(0),
        target_table: row.get(1),
        foreign_key: row.get(2),
    }
}

#[macro_export]
macro_rules! make_lint {
    ($lint_fn_name: ident, $lint_name: ident, $diagnostic_name: ident, $loc: expr) => {
        fn $lint_fn_name(txn: &mut Transaction) -> eyre::Result<Vec<SchemaDiagnostic>> {
            let mut out = Vec::new();
            for row in txn
                .query(
                    include_str!(concat!("rules/", stringify!($lint_name), ".sql")),
                    &[],
                )
                .context(concat!("failed to ", stringify!($lint_fn_name)))?
            {
                out.push(SchemaDiagnostic {
                    loc: $loc(row),
                    rule: SchemaDiagnosticRule::$diagnostic_name,
                });
            }
            Ok(out)
        }
    };
}
