// SPDX-FileCopyrightText: 2025 Olivier 'reivilibre'
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::BTreeMap, path::PathBuf};

use eyre::Context;
use maplit::btreeset;
use sqlparser::{
    ast::Statement,
    dialect::PostgreSqlDialect,
    parser::{Parser, ParserError},
};
use strum::EnumString;
use tracing::warn;
use walkdir::WalkDir;

use super::DiagnosticClassification;

pub mod gather_python;
pub mod gather_rust;
pub mod lint_donotdothis;

#[derive(Copy, Clone, Debug)]
pub struct FilePos {
    pub line: usize,
    pub col: usize,
}

/// SQL that was harvested from a source code file.
pub struct FoundSql {
    pub source_filename: String,
    pub source_span: (FilePos, FilePos),
    pub query: String,
}

/// SQL that has been parsed.
pub struct ParsedSql {
    pub source_filename: String,
    pub source_span: (FilePos, FilePos),
    pub text: String,
    pub statements: Vec<Statement>,
}

impl TryFrom<&FoundSql> for ParsedSql {
    type Error = ParserError;

    fn try_from(value: &FoundSql) -> Result<Self, Self::Error> {
        Parser::parse_sql(&PostgreSqlDialect {}, &value.query).map(|statements| ParsedSql {
            source_filename: value.source_filename.clone(),
            source_span: value.source_span,
            text: value.query.clone(),
            statements,
        })
    }
}

/// Given the Span from SQL parser and the FilePos start of the SQL query,
/// returns the in-file position of the Span.
fn span_within(
    span: sqlparser::tokenizer::Span,
    (within, _): (FilePos, FilePos),
) -> (FilePos, FilePos) {
    fn apply_one(span: sqlparser::tokenizer::Location, onto: FilePos) -> FilePos {
        if span.line == 1 {
            FilePos {
                line: onto.line,
                col: onto.col + span.column as usize - 1,
            }
        } else {
            FilePos {
                line: onto.line + span.line as usize - 1,
                col: span.column as usize,
            }
        }
    }

    (apply_one(span.start, within), apply_one(span.end, within))
}

#[derive(Debug)]
pub struct Diagnostic {
    pub span: (FilePos, FilePos),
    pub rule: DiagnosticRule,
}

#[derive(EnumString, strum::Display, Copy, Clone, Debug)]
#[strum(serialize_all = "snake_case")]
pub enum DiagnosticRule {
    DontUseCurrentTime,
    DontUseNotIn,
}

impl DiagnosticRule {
    pub fn describe(self) -> &'static str {
        match self {
            DiagnosticRule::DontUseCurrentTime => {
                "Don't use `CURRENT_TIME`
It returns a `timetz` which is only implemented for SQL compliance and virtually never appropriate.
See: <https://wiki.postgresql.org/wiki/Don't_Do_This#Don.27t_use_CURRENT_TIME>"
            }
            DiagnosticRule::DontUseNotIn => {
                "Don't use `NOT IN`
(Or any equivalent combination like `NOT (x IN ...)`)
`NOT IN` behaves in surprising ways if `NULL`s are present, which equivalently
causes `NOT IN (SELECT ...)` to produce slow query plans.
Most of the time, you will prefer `NOT EXISTS (SELECT ...)`.
There can be valid exceptions to this rule.
See: <https://wiki.postgresql.org/wiki/Don't_Do_This#Don.27t_use_NOT_IN>"
            }
        }
    }

    pub fn default_classification(self) -> DiagnosticClassification {
        use DiagnosticClassification::*;
        match self {
            DiagnosticRule::DontUseCurrentTime => Error,
            DiagnosticRule::DontUseNotIn => Warning,
        }
    }
}

fn gather_sql_from_code(paths: Vec<PathBuf>) -> eyre::Result<BTreeMap<String, Vec<ParsedSql>>> {
    let mut out = BTreeMap::new();
    for path in paths {
        for entry in WalkDir::new(&path) {
            let entry = entry.with_context(|| format!("failed to walk {path:?}"))?;
            let Some(path_str) = entry.path().to_str() else {
                continue;
            };

            let found_sqls = if path_str.ends_with(".py") {
                let source = std::fs::read_to_string(entry.path())
                    .with_context(|| format!("failed to read {path_str}"))?;
                gather_python::find_queries(
                    &source,
                    path_str.to_owned(),
                    btreeset! {
                        "txn.execute".to_owned(),
                        "query".to_owned(),
                        "db_pool.execute".to_owned()
                    },
                )?
            } else if path_str.ends_with(".rs") {
                let source = std::fs::read_to_string(entry.path())
                    .with_context(|| format!("failed to read {path_str}"))?;
                gather_rust::find_queries(&source, path_str.to_owned())?
            } else if path_str.ends_with(".sql")
                || path_str.ends_with(".sql.postgres")
                || path_str.ends_with(".sql.postgresql")
            {
                let source = std::fs::read_to_string(entry.path())
                    .with_context(|| format!("failed to read {path_str}"))?;
                vec![FoundSql {
                    source_filename: path_str.to_owned(),
                    source_span: (FilePos { line: 1, col: 1 }, FilePos { line: 1, col: 1 }),
                    query: source,
                }]
            } else {
                continue;
            };

            let mut parsed_sql = Vec::with_capacity(found_sqls.len());

            for sql in &found_sqls {
                match ParsedSql::try_from(sql) {
                    Ok(parsed) => {
                        parsed_sql.push(parsed);
                    }
                    Err(err) => {
                        warn!(
                            "could not parse query: {err:?}\nlocation: {} ({}:{})\ntext: {}",
                            sql.source_filename,
                            sql.source_span.0.line,
                            sql.source_span.0.col,
                            sql.query
                        );
                    }
                }
            }

            out.insert(path_str.to_owned(), parsed_sql);
        }
    }
    Ok(out)
}

pub fn lint_sql_in_code(paths: Vec<PathBuf>) -> eyre::Result<BTreeMap<String, Vec<Diagnostic>>> {
    let parsed_sqls = gather_sql_from_code(paths)?;

    let mut out = BTreeMap::new();

    for (file, sqls) in parsed_sqls {
        let mut diags = Vec::new();
        for sql in sqls {
            diags.extend(lint_all(&sql));
        }

        out.insert(file, diags);
    }

    Ok(out)
}

fn lint_all(sql: &ParsedSql) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for lintgroup in &[lint_donotdothis::LINTS] {
        for lint in *lintgroup {
            out.extend(lint(sql));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Diagnostic, FilePos, FoundSql, ParsedSql};

    fn parse(sql: &str) -> ParsedSql {
        ParsedSql::try_from(&FoundSql {
            source_filename: "test.sql".to_owned(),
            source_span: (FilePos { line: 1, col: 1 }, FilePos { line: 1, col: 1 }),
            query: sql.to_owned(),
        })
        .unwrap()
    }

    #[track_caller]
    pub(crate) fn assert_lint_fail(sql: &str, linter: fn(&ParsedSql) -> Vec<Diagnostic>) {
        let parsed = parse(sql);
        let diagnostics = linter(&parsed);
        assert!(
            !diagnostics.is_empty(),
            "expected lint fail, but no diagnostics produced"
        );
    }

    #[track_caller]
    pub(crate) fn assert_lint_ok(sql: &str, linter: fn(&ParsedSql) -> Vec<Diagnostic>) {
        let parsed = parse(sql);
        let diagnostics = linter(&parsed);
        assert!(
            diagnostics.is_empty(),
            "unexpected lint fail: {diagnostics:#?}"
        );
    }
}
