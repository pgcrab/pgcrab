//! Schema lint rules taken from [`pg-index-health-sql`].
//! Not all rules were adopted. Some are instead part of the 'Do Not Do This' set.
//!
//! [`pg-index-health-sql`]: https://github.com/mfvanek/pg-index-health-sql

use crate::make_lint;

use super::{
    macros::{loc_foreign_key, loc_foreign_keys, loc_index, loc_indexes, loc_object, loc_table},
    SchemaDiagnostic, SchemaDiagnosticRule,
};
use eyre::WrapErr;
use postgres::Transaction;

make_lint!(
    lint_possible_object_name_truncation,
    possible_object_name_truncation,
    PossibleObjectNameTruncation,
    loc_object
);

make_lint!(
    lint_duplicate_indexes,
    duplicate_indexes,
    DuplicateIndexes,
    loc_indexes
);

make_lint!(
    lint_duplicate_foreign_keys,
    duplicate_foreign_keys,
    DuplicateForeignKeys,
    loc_foreign_keys
);

make_lint!(
    lint_foreign_key_without_index,
    foreign_key_without_index,
    ForeignKeyWithoutIndex,
    loc_foreign_key
);

make_lint!(
    lint_foreign_key_with_unmatched_column_type,
    foreign_key_with_unmatched_column_type,
    ForeignKeyWithUnmatchedColumnType,
    loc_foreign_key
);

make_lint!(
    lint_index_with_redundant_where_clause,
    index_with_redundant_where_clause,
    IndexWithRedundantWhereClause,
    loc_index
);

make_lint!(
    lint_overlapping_indexes,
    overlapping_indexes,
    OverlappingIndexes,
    loc_indexes
);

make_lint!(
    lint_overlapping_foreign_keys,
    overlapping_foreign_keys,
    OverlappingForeignKeys,
    loc_foreign_keys
);

make_lint!(
    lint_table_without_primary_key,
    table_without_primary_key,
    TableWithoutPrimaryKey,
    loc_table
);

make_lint!(
    lint_btree_index_on_array_column,
    btree_index_on_array_column,
    BtreeIndexOnArrayColumn,
    loc_index
);

make_lint!(
    lint_index_with_boolean,
    index_with_boolean,
    IndexWithBoolean,
    loc_index
);

// TODO:
// indexes_with_timestamp_in_the_middle.sql NOT SURE

pub const LINTS: &[fn(&mut Transaction) -> eyre::Result<Vec<SchemaDiagnostic>>] = &[
    lint_possible_object_name_truncation,
    lint_duplicate_indexes,
    lint_duplicate_foreign_keys,
    lint_foreign_key_without_index,
    lint_foreign_key_with_unmatched_column_type,
    lint_index_with_redundant_where_clause,
    lint_overlapping_indexes,
    lint_overlapping_foreign_keys,
    lint_table_without_primary_key,
    lint_btree_index_on_array_column,
    lint_index_with_boolean,
];
