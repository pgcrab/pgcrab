//! Module to find SQL queries embedded in Python source code.
//!
//! # LLMs
//!
//! This module was mostly written by LLM as an isolated module,
//! once fed a repomix of the rustpython-ast package,
//! with human tweaks only to fix obvious issues, fix test cheating
//! and simplify the implementation.
//! Be cautious!

use std::collections::BTreeSet;

use eyre::{Context, Result};
use rustpython_ast::{
    source_code::{LineIndex, SourceCode, SourceLocation},
    ExceptHandler, Expr, Mod, Stmt,
};
use rustpython_parser::{parse, Mode};

use super::{FilePos, FoundSql};

impl From<SourceLocation> for FilePos {
    fn from(value: SourceLocation) -> Self {
        FilePos {
            line: value.row.to_usize(),
            col: value.column.to_usize(),
        }
    }
}

/// Find SQL queries within a module of Python source code.
///
/// `expected_query_functions` contains the names of functions
/// that are likely to contain SQL as the first argument, like `txn.execute`.
///
/// This function parses Python source code and traverses the abstract syntax tree (AST)
/// to find function calls that match the patterns provided in `expected_query_functions`.
/// When such a call is found, it extracts the SQL query from the first string argument.
///
/// Limitations:
/// - Only detects queries passed as string literals in the first argument
/// - Dynamically-generated SQL queries will not be found
/// - Complex string formatting (f-strings, etc.) is not supported
pub fn find_queries(
    source: &str,
    source_filename: String,
    expected_query_functions: BTreeSet<String>,
) -> Result<Vec<FoundSql>> {
    // Parse the Python source code into an Abstract Syntax Tree (AST)
    let ast: Mod =
        parse(&source, Mode::Module, &source_filename).context("failed to parse Python")?;

    let line_index = LineIndex::from_source_text(source);
    let source_code = SourceCode::new(source, &line_index);

    let mut visitor = SqlQueryVisitor::new(expected_query_functions, source_filename, source_code);
    visitor.visit_mod(&ast);

    Ok(visitor.queries)
}

/// A visitor that traverses the Python AST to find SQL queries
///
/// This visitor implements a depth-first traversal of the AST,
/// checking for function calls that match the expected patterns
/// and collecting SQL queries found in string literals.
struct SqlQueryVisitor<'sc> {
    /// Set of function name patterns to look for (e.g., "txn.execute")
    expected_functions: BTreeSet<String>,
    /// Name of the source file being analyzed
    source_filename: String,
    /// Collection of SQL queries found during traversal
    queries: Vec<FoundSql>,
    /// Helper to map text positions to line numbers
    source: SourceCode<'sc, 'sc>,
}

impl<'sc> SqlQueryVisitor<'sc> {
    /// Creates a new SqlQueryVisitor with the specified function patterns to search for
    fn new(
        expected_functions: BTreeSet<String>,
        source_filename: String,
        source: SourceCode<'sc, 'sc>,
    ) -> Self {
        SqlQueryVisitor {
            expected_functions,
            source_filename,
            queries: Vec::new(),
            source,
        }
    }

    fn visit_mod(&mut self, node: &Mod) {
        match node {
            Mod::Module(module) => {
                for stmt in &module.body {
                    self.visit_stmt(stmt);
                }
            }
            Mod::Expression(expr) => {
                // Handle single expression modules
                self.visit_expr(&expr.body);
            }
            // Ignore other module types
            _ => {}
        }
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::FunctionDef(func) => {
                for stmt in &func.body {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::AsyncFunctionDef(func) => {
                for stmt in &func.body {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::ClassDef(cls) => {
                for stmt in &cls.body {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::If(if_stmt) => {
                for stmt in &if_stmt.body {
                    self.visit_stmt(stmt);
                }
                for stmt in &if_stmt.orelse {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::For(for_stmt) => {
                for stmt in &for_stmt.body {
                    self.visit_stmt(stmt);
                }
                for stmt in &for_stmt.orelse {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::AsyncFor(for_stmt) => {
                for stmt in &for_stmt.body {
                    self.visit_stmt(stmt);
                }
                for stmt in &for_stmt.orelse {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::While(while_stmt) => {
                for stmt in &while_stmt.body {
                    self.visit_stmt(stmt);
                }
                for stmt in &while_stmt.orelse {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::With(with_stmt) => {
                for stmt in &with_stmt.body {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::AsyncWith(with_stmt) => {
                for stmt in &with_stmt.body {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::Try(try_stmt) => {
                for stmt in &try_stmt.body {
                    self.visit_stmt(stmt);
                }
                for ExceptHandler::ExceptHandler(handler) in &try_stmt.handlers {
                    for stmt in &handler.body {
                        self.visit_stmt(stmt);
                    }
                }
                for stmt in &try_stmt.orelse {
                    self.visit_stmt(stmt);
                }
                for stmt in &try_stmt.finalbody {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::TryStar(try_stmt) => {
                for stmt in &try_stmt.body {
                    self.visit_stmt(stmt);
                }
                for ExceptHandler::ExceptHandler(handler) in &try_stmt.handlers {
                    for stmt in &handler.body {
                        self.visit_stmt(stmt);
                    }
                }
                for stmt in &try_stmt.orelse {
                    self.visit_stmt(stmt);
                }
                for stmt in &try_stmt.finalbody {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::Expr(expr_stmt) => {
                self.visit_expr(&expr_stmt.value);
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Call(call) => {
                // First recursively check arguments for nested expressions
                // This ensures we process nested function calls in depth-first order
                for arg in &call.args {
                    self.visit_expr(arg);
                }
                for kw in &call.keywords {
                    self.visit_expr(&kw.value);
                }

                // Then check if this call matches one of our expected function patterns
                if let Some(func_name) = self.extract_function_name(&call.func) {
                    if self.expected_functions.contains(&func_name) {
                        // Check if the first argument is a string literal containing SQL
                        if let Some(arg) = call.args.first() {
                            // Extract SQL string from first argument if it's a constant string
                            match arg {
                                Expr::Constant(constant) => {
                                    if let Some(sql) = constant.value.as_str() {
                                        let mut start =
                                            self.source.source_location(constant.range.start());
                                        // TODO +1 for single-quoted strings. Not correct for `"""` strings :-(
                                        start.column = start.column.saturating_add(1);
                                        let end = self.source.source_location(constant.range.end());

                                        self.queries.push(FoundSql {
                                            source_filename: self.source_filename.clone(),
                                            source_span: (start.into(), end.into()),
                                            query: sql.to_string(),
                                        });
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // Arguments already processed above
            }
            Expr::BoolOp(bool_op) => {
                for value in &bool_op.values {
                    self.visit_expr(value);
                }
            }
            Expr::BinOp(bin_op) => {
                self.visit_expr(&bin_op.left);
                self.visit_expr(&bin_op.right);
            }
            Expr::UnaryOp(unary_op) => {
                self.visit_expr(&unary_op.operand);
            }
            Expr::Lambda(lambda) => {
                self.visit_expr(&lambda.body);
            }
            Expr::IfExp(if_exp) => {
                self.visit_expr(&if_exp.test);
                self.visit_expr(&if_exp.body);
                self.visit_expr(&if_exp.orelse);
            }
            Expr::Dict(dict) => {
                for key in dict.keys.iter().flatten() {
                    self.visit_expr(key);
                }
                for value in &dict.values {
                    self.visit_expr(value);
                }
            }
            Expr::List(list) => {
                for elt in &list.elts {
                    self.visit_expr(elt);
                }
            }
            Expr::Tuple(tuple) => {
                for elt in &tuple.elts {
                    self.visit_expr(elt);
                }
            }
            _ => {}
        }
    }

    /// Helper method to extract a qualified function name from an expression
    ///
    /// For example, from the expression `txn.execute`, this would extract "txn.execute"
    /// Currently only handles simple object.method patterns
    fn extract_function_name(&self, expr: &Expr) -> Option<String> {
        match expr {
            // Handle attribute access like "txn.execute"
            Expr::Attribute(attr) => {
                if let Expr::Name(obj) = attr.value.as_ref() {
                    return Some(format!("{}.{}", obj.id, attr.attr));
                } else if let Expr::Attribute(_) = attr.value.as_ref() {
                    // Handle nested attributes like "db.conn.execute"
                    if let Some(prefix) = self.extract_function_name(&*attr.value) {
                        return Some(format!("{}.{}", prefix, attr.attr));
                    }
                }
            }
            // Could be extended to handle more complex patterns in the future
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::lint::sql::FoundSql;

    fn default_expected() -> BTreeSet<String> {
        let mut expected_functions = BTreeSet::new();
        expected_functions.insert("txn.execute".to_string());
        expected_functions
    }
    fn find_queries(source: &str, expected_functions: BTreeSet<String>) -> Vec<FoundSql> {
        let cleaned_source = source.trim_start().replace("\n    ", "\n");

        super::find_queries(&cleaned_source, "test.py".to_owned(), expected_functions).unwrap()
    }

    #[test]
    fn test_basic_query() {
        let source = r#"
    def test_function():
        txn.execute("SELECT * FROM users WHERE id = 1")
        return True
    "#;

        let queries = find_queries(source, default_expected());

        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].query, "SELECT * FROM users WHERE id = 1");
        assert_eq!(queries[0].source_filename, "test.py");
        assert_eq!(queries[0].source_span.0.line, 2);
    }

    #[test]
    fn test_multiple_queries() {
        let source = r#"
    def test_function():
        txn.execute("SELECT * FROM users")

        other_function()

        db.query("SELECT * FROM products")

        # This one shouldn't be picked up
        not_db.query("NOT SQL")
    "#;

        let mut expected_functions = BTreeSet::new();
        expected_functions.insert("txn.execute".to_string());
        expected_functions.insert("db.query".to_string());

        let queries = find_queries(source, expected_functions);

        assert_eq!(queries.len(), 2);
        assert_eq!(queries[0].query, "SELECT * FROM users");
        assert_eq!(queries[0].source_span.0.line, 2);
        assert_eq!(queries[1].query, "SELECT * FROM products");
        assert_eq!(queries[1].source_span.0.line, 6);
    }

    #[test]
    fn test_nested_queries() {
        let source = r#"
    def outer_function():
        def inner_function():
            txn.execute("SELECT * FROM inner_table")

        txn.execute("SELECT * FROM outer_table")
        inner_function()
    "#;

        let queries = find_queries(source, default_expected());

        assert_eq!(queries.len(), 2);
        assert_eq!(queries[0].query, "SELECT * FROM inner_table");
        assert_eq!(queries[1].query, "SELECT * FROM outer_table");
    }

    #[test]
    fn test_complex_structures() {
        let source = r#"
    def test_function():
        if condition:
            txn.execute("SELECT * FROM users WHERE condition = TRUE")
        else:
            txn.execute("SELECT * FROM users WHERE condition = FALSE")

        for item in items:
            txn.execute("SELECT * FROM items WHERE id = 1")

        with context_manager:
            txn.execute("SELECT * FROM contexts")

        try:
            txn.execute("SELECT * FROM try_table")
        except Exception:
            txn.execute("SELECT * FROM except_table")
        finally:
            txn.execute("SELECT * FROM finally_table")
    "#;

        let queries = find_queries(source, default_expected());

        assert_eq!(queries.len(), 7);
    }
}
