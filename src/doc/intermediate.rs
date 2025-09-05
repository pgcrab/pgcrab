use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct IntermediateSchema {
    pub tables: BTreeMap<String, Table>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Table {
    pub name: String,
    pub comment: String,
    pub columns: Vec<Column>,
    pub indices: Vec<Index>,
    pub foreign_keys: Vec<ForeignKey>,
    pub foreign_key_backlinks: Vec<ForeignKey>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Column {
    pub name: String,
    pub r#type: String,
    pub not_null: bool,
    // TODO pub unique: bool,
    // TODO pub primary_key: bool,
    pub comment: String,
}

/// Represents an index on a table.
/// TODO the representation here is not very granular and could be improved.
#[derive(Clone, Debug, Serialize)]
pub struct Index {
    pub name: String,
    /// reconstructed SQL used to create it, from pg_get_constraintdef or pg_get_indexdef.
    pub def: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ForeignKey {
    pub name: String,
    pub referrer_table: String,
    pub referee_table: String,
    // definition from pg_get_constraintdef
    pub def: String,
    // pub referrer_columns: Vec<String>,
    // pub referee_columns: Vec<String>,
}
