// SPDX-License-Identifier: GPL-3.0-or-later

//! Lint rules covering day-to-day SQL parts of the 'Don't Do This' wiki page
//! for Postgres.
//! See: <https://wiki.postgresql.org/wiki/Don't_Do_This>
//!
//! Entries on that page which are more relevant to schemas are instead
//! schema lints.
//!
//! TODO:
//! - Don't use BETWEEN (especially with timestamps)
//! - Don't use NOT IN

use std::ops::ControlFlow;

use sqlparser::ast::{visit_expressions, Expr, Spanned};

use super::{span_within, Diagnostic, DiagnosticRule, ParsedSql};

pub fn lint_no_current_time(sql: &ParsedSql) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for stmt in &sql.statements {
        let _ = visit_expressions(stmt, |expr| {
            match expr {
                Expr::Function(func) => {
                    let [ident] = func.name.0.as_slice() else {
                        return ControlFlow::Continue(());
                    };
                    let Some(ident) = ident.as_ident() else {
                        return ControlFlow::Continue(());
                    };
                    if ident.value.eq_ignore_ascii_case("CURRENT_TIME") {
                        diags.push(Diagnostic {
                            span: span_within(expr.span(), sql.source_span),
                            rule: DiagnosticRule::DontUseCurrentTime,
                        });
                    }
                }
                _ => {}
            }
            ControlFlow::Continue::<()>(())
        });
    }
    diags
}

pub fn lint_dont_use_not_in(sql: &ParsedSql) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for stmt in &sql.statements {
        let _ = visit_expressions(stmt, |expr| {
            match expr {
                Expr::InSubquery {
                    expr: _,
                    negated,
                    subquery: _,
                } => {
                    if *negated {
                        diags.push(Diagnostic {
                            span: span_within(expr.span(), sql.source_span),
                            rule: DiagnosticRule::DontUseNotIn,
                        });
                    }
                }
                _ => {}
            }
            ControlFlow::Continue::<()>(())
        });
    }
    diags
}

pub const LINTS: &[fn(&ParsedSql) -> Vec<Diagnostic>] =
    &[lint_no_current_time, lint_dont_use_not_in];

#[cfg(test)]
mod tests {
    use crate::lint::sql::tests::{assert_lint_fail, assert_lint_ok};

    use super::{lint_dont_use_not_in, lint_no_current_time};

    #[test]
    fn test_no_current_time() {
        assert_lint_ok(
            "SELECT CURRENT_TIMESTAMP, username FROM users;",
            lint_no_current_time,
        );
        assert_lint_fail(
            "SELECT CURRENT_TIME, username FROM users;",
            lint_no_current_time,
        );
        assert_lint_fail(
            "SELECT current_time, username FROM users;",
            lint_no_current_time,
        );
    }

    #[test]
    fn test_dont_use_not_in() {
        assert_lint_ok(
            "SELECT a FROM tbl WHERE a IN (SELECT a FROM tbl2)",
            lint_dont_use_not_in,
        );

        assert_lint_fail(
            "SELECT a FROM tbl WHERE a NOT IN (SELECT a FROM tbl2)",
            lint_dont_use_not_in,
        );

        // FUTURE ENHANCEMENT: catch this case
        assert_lint_ok(
            "SELECT a FROM tbl WHERE NOT (a IN (SELECT a FROM tbl2))",
            lint_dont_use_not_in,
        );
    }
}
