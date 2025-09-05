//! Module for pgCrab-original schema lint rules.

use eyre::WrapErr;
use postgres::Transaction;

use crate::make_lint;

use super::{macros::loc_table, SchemaDiagnostic, SchemaDiagnosticRule};

make_lint!(
    lint_table_without_replica_identity,
    table_without_replica_identity,
    TableWithoutReplicaIdentity,
    loc_table
);

pub const LINTS: &[fn(&mut Transaction) -> eyre::Result<Vec<SchemaDiagnostic>>] =
    &[lint_table_without_replica_identity];
