// SPDX-License-Identifier: GPL-3.0-or-later

use eyre::Context;
use postgres::Transaction;

use crate::make_lint;

use super::{
    macros::{loc_column, loc_object},
    SchemaDiagnostic, SchemaDiagnosticRule,
};

// fn lint_no_timestamp(txn: &mut Transaction) -> eyre::Result<Vec<SchemaDiagnostic>> {
//     let mut out = Vec::new();
//     for row in txn
//         .query(include_str!("rules/columns_with_timestamp_type.sql"), &[])
//         .context("failed to look for TIMESTAMP columns")?
//     {
//         out.push(SchemaDiagnostic {
//             loc: SchemaLoc::Column {
//                 table: row.get(0),
//                 column: row.get(1),
//             },
//             rule: SchemaDiagnosticRule::DontUseTimestampWithoutTimeZone,
//         });
//     }
//     Ok(out)
// }

make_lint!(
    lint_no_timestamp,
    column_with_timestamp_type,
    DontUseTimestampWithoutTimeZone,
    loc_column
);

make_lint!(
    lint_no_money,
    column_with_money_type,
    DontUseMoney,
    loc_column
);

make_lint!(
    lint_no_serial,
    column_with_serial_type,
    DontUseSerial,
    loc_column
);

make_lint!(
    lint_no_varchar_n,
    column_with_fixed_length_varchar,
    DontUseVarcharNByDefault,
    loc_column
);

make_lint!(
    lint_column_requires_quotation,
    column_requires_quotation,
    ColumnRequiresQuotation,
    loc_column
);

make_lint!(
    lint_object_requires_quotation,
    object_requires_quotation,
    ObjectRequiresQuotation,
    loc_object
);

pub const LINTS: &[fn(&mut Transaction) -> eyre::Result<Vec<SchemaDiagnostic>>] = &[
    lint_no_timestamp,
    lint_no_money,
    lint_no_serial,
    lint_no_varchar_n,
    lint_column_requires_quotation,
    lint_object_requires_quotation,
];
