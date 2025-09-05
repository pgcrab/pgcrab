// SPDX-License-Identifier: GPL-3.0-or-later

//! Module to find SQL queries embedded in Rust source code.
//!
//! # LLMs
//!
//! This module was mostly written by LLM as an isolated module,
//! with human tweaks only to fix obvious issues, add more test cases
//! and simplify the implementation.
//! Be cautious!

use itertools::Itertools;
use proc_macro2::{LineColumn, Span};
use syn::parse::Parser;
use syn::{visit::Visit, Expr, ExprLit, ExprMacro, Lit, MacroDelimiter};

use super::{FilePos, FoundSql};

impl From<LineColumn> for FilePos {
    fn from(value: LineColumn) -> Self {
        FilePos {
            line: value.line,
            // +1 because 0-indexed to 1-indexed
            col: value.column + 1,
        }
    }
}

/// Find SQL queries within a unit of Rust source code.
///
/// SQL queries are expected to be found within:
/// - `query!` or `sqlx::query!` macros
/// - `query_as!` or `sqlx::query_as!` macros
/// - `query_scalar!` or `sqlx::query_scalar!` macros
pub fn find_queries(source: &str, source_filename: String) -> eyre::Result<Vec<FoundSql>> {
    // Parse the Rust source code
    let file =
        syn::parse_file(source).map_err(|e| eyre::eyre!("Failed to parse Rust source: {}", e))?;

    let mut visitor = SqlMacroVisitor {
        queries: Vec::new(),
        source_filename,
    };

    visitor.visit_file(&file);
    Ok(visitor.queries)
}

struct SqlMacroVisitor {
    queries: Vec<FoundSql>,
    source_filename: String,
}

impl<'ast> Visit<'ast> for SqlMacroVisitor {
    fn visit_expr_macro(&mut self, node: &'ast ExprMacro) {
        // Check if this is one of the target SQL macros
        let path = &node
            .mac
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .join("::");

        let is_sql_macro = match path.as_str() {
            "query" | "sqlx::query" | "query_as" | "sqlx::query_as" | "query_scalar"
            | "sqlx::query_scalar" => true,
            _ => false,
        };

        if is_sql_macro {
            if let MacroDelimiter::Paren(_) = node.mac.delimiter {
                // Try to extract the SQL query from the arguments
                // // First try to parse as a single expression
                if let Ok(exprs) =
                    syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                        .parse2(node.mac.tokens.clone())
                {
                    if !exprs.is_empty() {
                        if let Some((query, span)) = extract_query_from_expr(&exprs[0]) {
                            let mut span_start = span.start();
                            let mut span_end = span.end();
                            // TODO assume single-quoted string, inset by the size of the delimiter
                            span_start.column += 1;
                            span_end.column -= 1;
                            self.queries.push(FoundSql {
                                source_filename: self.source_filename.clone(),
                                source_span: (span_start.into(), span_end.into()),
                                query,
                            });
                        }
                    }
                }
            }
        }

        // Continue visiting the AST
        syn::visit::visit_expr_macro(self, node);
    }
}

fn extract_query_from_expr(expr: &Expr) -> Option<(String, Span)> {
    match expr {
        // For a direct string literal: query!("SELECT * FROM table")
        Expr::Lit(ExprLit {
            lit: Lit::Str(lit_str),
            ..
        }) => Some((lit_str.value(), lit_str.span())),

        // Other expression types
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_queries(rust_src: &str) -> Vec<FoundSql> {
        super::find_queries(rust_src, "test".to_owned()).unwrap()
    }

    #[test]
    fn test_find_queries_simple() {
        let source = r#"
            fn example() {
                let q = query!("SELECT * FROM users");
            }
        "#;

        let result = find_queries(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].query, "SELECT * FROM users");
    }

    #[test]
    fn test_find_queries_raw_strings() {
        let source = r##"
            fn example() {
                let q = query!(r#"SELECT * FROM\nusers"#);
            }
        "##;

        let result = find_queries(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].query, "SELECT * FROM\\nusers");
    }

    #[test]
    fn test_find_queries_escaped_strings() {
        let source = r#"
            fn example() {
                let q = query!("SELECT * FROM\nusers");
            }
        "#;

        let result = find_queries(source);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].query,
            "SELECT * FROM
users"
        );
    }

    #[test]
    fn test_find_queries_with_namespace() {
        let source = r#"
            fn example() {
                let q = sqlx::query!("SELECT * FROM users");
            }
        "#;

        let result = find_queries(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].query, "SELECT * FROM users");
    }

    #[test]
    fn test_find_queries_as_and_scalar() {
        let source = r#"
            fn example() {
                let q1 = query_as!("SELECT id, name FROM users");
                let q2 = sqlx::query_scalar!("SELECT COUNT(*) FROM users");
            }
        "#;

        let result = find_queries(source);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].query, "SELECT id, name FROM users");
        assert_eq!(result[1].query, "SELECT COUNT(*) FROM users");
    }

    #[test]
    fn test_find_queries_with_params() {
        let source = r#"
            fn example() {
                let id = 42;
                let q = query!("SELECT * FROM users WHERE id = $1", id);
            }
        "#;

        let result = find_queries(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].query, "SELECT * FROM users WHERE id = $1");
    }

    #[test]
    fn test_find_queries_multiline() {
        let source = r#"
            fn example() {
                let q = query!(
                    "SELECT *
                    FROM users
                    WHERE active = true"
                );
            }
        "#;

        let result = find_queries(source);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].query,
            "SELECT *
                    FROM users
                    WHERE active = true"
        );
    }

    #[test]
    fn test_parse_comma_separated_args() {
        let source = r#"
            fn example() {
                let q = query!("SELECT * FROM users", id, name);
            }
        "#;

        let result = find_queries(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].query, "SELECT * FROM users");
    }

    #[test]
    fn test_extract_query_from_expr() {
        // This test is more complex as it requires creating Expr instances
        // Just test a few simplified cases
        let expr = syn::parse_str::<Expr>(r#""SELECT * FROM users""#).unwrap();
        assert_eq!(
            extract_query_from_expr(&expr).unwrap().0.as_str(),
            "SELECT * FROM users"
        );
    }

    #[test]
    fn test_escaped_quotes_in_query() {
        let source = r#"
            fn example() {
                let q = query!("SELECT * FROM \"table\" WHERE id = $1", id);
            }
        "#;

        let result = find_queries(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].query, r#"SELECT * FROM "table" WHERE id = $1"#);
    }
}
