use strum::EnumString;

pub mod schema;

pub mod sql;

#[derive(EnumString, strum::Display, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[strum(serialize_all = "snake_case")]
pub enum DiagnosticClassification {
    Note,
    Warning,
    Error,
}
