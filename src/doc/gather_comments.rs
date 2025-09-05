// SPDX-License-Identifier: GPL-3.0-or-later

//! Module to gather comments alongside tables and columns from SQL migration
//! files.
//!
//! # LLMs
//!
//! A substantial portion of this was generated with LLM.
//! It's considered 'good enough' but is not very robust,
//! given regular expressions are used for parsing.

use std::{
    collections::{btree_map, BTreeMap},
    path::PathBuf,
    sync::Arc,
};

use eyre::Context;
use maplit::btreemap;
use regex::Regex;
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
pub struct DocComment {
    pub comment: String,
    pub line_number: usize,
}

#[derive(Debug, Serialize)]
pub struct TableInfo {
    #[serde(skip)]
    pub file_path: Arc<String>,
    pub name: String,
    pub doc_comment: Option<DocComment>,
    pub columns: BTreeMap<String, ColumnInfo>,
}

#[derive(Debug, Serialize)]
pub struct ColumnInfo {
    #[serde(skip)]
    pub file_path: Arc<String>,
    pub name: String,
    pub doc_comment: Option<DocComment>,
    pub table_name: String,
}

#[derive(Debug, Serialize)]
pub struct HarvestedFile {
    #[serde(skip)]
    pub file_path: Arc<String>,
    pub tables: Vec<TableInfo>,
    pub alter_columns: Vec<ColumnInfo>,
}

#[derive(Debug, Serialize)]
pub struct TotalHarvest {
    pub tables: BTreeMap<String, TableInfo>,
}

pub fn harvest_from_paths(paths: &Vec<PathBuf>) -> eyre::Result<TotalHarvest> {
    fn harvest(paths: &Vec<PathBuf>) -> eyre::Result<BTreeMap<Arc<String>, HarvestedFile>> {
        let mut out = BTreeMap::new();
        for path in paths {
            for entry in WalkDir::new(&path) {
                let entry = entry.with_context(|| format!("failed to walk {path:?}"))?;
                let Some(path_str) = entry.path().to_str() else {
                    continue;
                };
                if !(path_str.ends_with(".sql")
                    || path_str.ends_with(".sql.postgres")
                    || path_str.ends_with(".sql.postgresql"))
                {
                    continue;
                }

                let content = std::fs::read_to_string(entry.path())
                    .with_context(|| format!("could not read {path_str}"))?;
                let path_str = Arc::new(path_str.to_owned());
                out.insert(path_str.clone(), harvest_sql_file(&content, path_str));
            }
        }
        Ok(out)
    }

    let all = harvest(paths)?;

    let mut combined = TotalHarvest {
        tables: BTreeMap::new(),
    };

    // We rely on BTreeMap ordering here to make it so that
    // lexicographically-later migrations override earlier
    // ones.
    for (filename, file) in all {
        for table in file.tables {
            combined.tables.insert(table.name.clone(), table);
        }

        for column in file.alter_columns {
            match combined.tables.entry(column.table_name.clone()) {
                btree_map::Entry::Vacant(ve) => {
                    ve.insert(TableInfo {
                        file_path: filename.clone(),
                        name: column.table_name.clone(),
                        doc_comment: None,
                        columns: btreemap![column.name.clone() => column],
                    });
                }
                btree_map::Entry::Occupied(mut oe) => {
                    oe.get_mut().columns.insert(column.name.to_owned(), column);
                }
            }
        }
    }

    Ok(combined)
}

// fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let directory = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());

//     let results = parse_sql_directory(&directory)?;

//     for result in results {
//         println!("File: {}", result.file_path);
//         println!("===========================================");

//         for table in result.tables {
//             if let Some(doc) = &table.doc_comment {
//                 println!(
//                     "Table '{}' (line {}): {}",
//                     table.name, doc.line_number, doc.comment
//                 );
//             } else {
//                 println!("Table '{}': No documentation", table.name);
//             }

//             for column in table.columns {
//                 if let Some(doc) = &column.doc_comment {
//                     println!(
//                         "  Column '{}' (line {}): {}",
//                         column.name, doc.line_number, doc.comment
//                     );
//                 } else {
//                     println!("  Column '{}': No documentation", column.name);
//                 }
//             }
//             println!();
//         }

//         if !result.alter_columns.is_empty() {
//             println!("ALTER TABLE ADD COLUMN statements:");
//             for column in result.alter_columns {
//                 if let Some(doc) = &column.doc_comment {
//                     println!(
//                         "  Column '{}' in table '{}' (line {}): {}",
//                         column.name, column.table_name, doc.line_number, doc.comment
//                     );
//                 } else {
//                     println!(
//                         "  Column '{}' in table '{}': No documentation",
//                         column.name, column.table_name
//                     );
//                 }
//             }
//             println!();
//         }

//         println!();
//     }

//     Ok(())
// }

// fn parse_sql_directory(dir_path: &str) -> Result<Vec<ParseResult>, Box<dyn std::error::Error>> {
//     let mut results = Vec::new();

//     for entry in fs::read_dir(dir_path)? {
//         let entry = entry?;
//         let path = entry.path();

//         if path.is_file() && path.extension().map_or(false, |ext| ext == "sql") {
//             let content = fs::read_to_string(&path)?;
//             let file_path = path.to_string_lossy().to_string();

//             let result = parse_sql_file(&content, &file_path);
//             results.push(result);
//         }
//     }

//     Ok(results)
// }

/// Harvests commentable items from a SQL migration file.
///
/// Handles DDL statements `CREATE TABLE` and `ALTER TABLE ... ADD COLUMN`.
///
/// The parser is not robust at all, so only common formattings of code will work.
/// Should probably use `sqlparser`.
pub fn harvest_sql_file(content: &str, file_path: Arc<String>) -> HarvestedFile {
    let lines: Vec<&str> = content.lines().collect();
    let mut tables = Vec::new();
    let mut alter_columns = Vec::new();

    // Regex patterns
    let doc_comment_re = Regex::new(r"^\s*--\s*(.*)$").unwrap();
    let create_table_re = Regex::new(r"(?i)^\s*CREATE\s+TABLE\s+(\w+)").unwrap();
    let alter_table_re = Regex::new(r"(?i)^\s*ALTER\s+TABLE\s+(\w+)").unwrap();
    let add_column_re = Regex::new(r"(?i)^\s*ADD\s+COLUMN\s+(\w+)").unwrap();
    let column_re = Regex::new(r"^\s*(\w+)\s+").unwrap();

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];

        // Check for CREATE TABLE
        if let Some(captures) = create_table_re.captures(line) {
            let table_name = captures.get(1).unwrap().as_str().to_string();

            // Look for doc comment before CREATE TABLE
            let table_doc = find_preceding_doc_comment(&lines, i, &doc_comment_re);

            // Parse columns within the CREATE TABLE
            let (columns, end_idx) = parse_create_table_columns(
                &lines,
                i + 1,
                &doc_comment_re,
                &column_re,
                &table_name,
                file_path.clone(),
            );

            tables.push(TableInfo {
                file_path: file_path.clone(),
                name: table_name,
                doc_comment: table_doc,
                columns,
            });

            i = end_idx;
        }
        // Check for single-line ALTER TABLE ADD COLUMN first
        else if line.to_uppercase().contains("ALTER TABLE")
            && line.to_uppercase().contains("ADD COLUMN")
        {
            // Use a more comprehensive regex for single-line format
            let single_line_alter_re =
                Regex::new(r"(?i)^\s*ALTER\s+TABLE\s+(\w+)\s+ADD\s+COLUMN\s+(\w+)").unwrap();
            if let Some(captures) = single_line_alter_re.captures(line) {
                let table_name = captures.get(1).unwrap().as_str().to_string();
                let column_name = captures.get(2).unwrap().as_str().to_string();

                // Look for doc comment before this ALTER TABLE line
                let column_doc = find_preceding_doc_comment(&lines, i, &doc_comment_re);

                alter_columns.push(ColumnInfo {
                    file_path: file_path.clone(),
                    name: column_name,
                    doc_comment: column_doc,
                    table_name,
                });
            }
            i += 1;
        }
        // Check for multi-line ALTER TABLE format
        else if let Some(captures) = alter_table_re.captures(line) {
            let table_name = captures.get(1).unwrap().as_str().to_string();

            // Look for ADD COLUMN in subsequent lines
            let mut j = i + 1;
            while j < lines.len() {
                let next_line = lines[j];

                if let Some(add_captures) = add_column_re.captures(next_line) {
                    let column_name = add_captures.get(1).unwrap().as_str().to_string();

                    // Look for doc comment before ADD COLUMN line
                    let column_doc = find_preceding_doc_comment(&lines, j, &doc_comment_re);

                    alter_columns.push(ColumnInfo {
                        file_path: file_path.clone(),
                        name: column_name,
                        doc_comment: column_doc,
                        table_name: table_name.clone(),
                    });

                    i = j;
                    break;
                }

                // Skip empty lines and comments, but stop at non-empty SQL lines
                let trimmed = next_line.trim();
                if !trimmed.is_empty() && !doc_comment_re.is_match(trimmed) {
                    break;
                }

                j += 1;
            }

            i += 1;
        } else {
            i += 1;
        }
    }

    HarvestedFile {
        file_path: file_path.clone(),
        tables,
        alter_columns,
    }
}

fn find_preceding_doc_comment(
    lines: &[&str],
    current_idx: usize,
    doc_comment_re: &Regex,
) -> Option<DocComment> {
    if current_idx == 0 {
        return None;
    }

    let mut comment_lines = Vec::new();
    let mut first_comment_line = None;

    // Look backwards for consecutive doc comments (skip empty lines)
    for i in (0..current_idx).rev() {
        let line = lines[i].trim();

        if line.is_empty() {
            continue;
        }

        if let Some(captures) = doc_comment_re.captures(line) {
            let comment_text = captures.get(1).unwrap().as_str().trim().to_string();
            comment_lines.push(comment_text);
            first_comment_line = Some(i + 1); // Store line number (1-indexed)
        } else {
            // If we hit a non-empty, non-doc-comment line, stop looking
            break;
        }
    }

    if comment_lines.is_empty() {
        return None;
    }

    // Reverse the comment lines since we collected them backwards
    comment_lines.reverse();

    // Join multiple comment lines with newlines
    let combined_comment = comment_lines.join("\n");

    Some(DocComment {
        comment: combined_comment,
        line_number: first_comment_line.unwrap(),
    })
}

fn parse_create_table_columns(
    lines: &[&str],
    start_idx: usize,
    doc_comment_re: &Regex,
    column_re: &Regex,
    table_name: &str,
    file_path: Arc<String>,
) -> (BTreeMap<String, ColumnInfo>, usize) {
    let mut columns = BTreeMap::new();
    let mut i = start_idx;

    while i < lines.len() {
        let line = lines[i].trim();

        // End of CREATE TABLE statement
        if line.ends_with(");") {
            break;
        }

        // Skip empty lines and lines that start with constraints
        if line.is_empty()
            || line.to_uppercase().starts_with("PRIMARY KEY")
            || line.to_uppercase().starts_with("FOREIGN KEY")
            || line.to_uppercase().starts_with("UNIQUE")
            || line.to_uppercase().starts_with("CHECK")
            || line.to_uppercase().starts_with("CONSTRAINT")
        {
            i += 1;
            continue;
        }

        // Check if this looks like a column definition
        if let Some(captures) = column_re.captures(line) {
            let column_name = captures.get(1).unwrap().as_str().to_string();

            // Look for doc comment before this column
            let column_doc = find_preceding_doc_comment(lines, i, doc_comment_re);

            columns.insert(
                column_name.clone(),
                ColumnInfo {
                    file_path: file_path.clone(),
                    name: column_name,
                    doc_comment: column_doc,
                    table_name: table_name.to_string(),
                },
            );
        }

        i += 1;
    }

    (columns, i)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use insta::assert_yaml_snapshot;

    use super::HarvestedFile;

    fn harvest_sql(sql: &str) -> HarvestedFile {
        super::harvest_sql_file(sql, Arc::new("test.sql".to_owned()))
    }

    #[test]
    fn test_harvest() {
        let h = harvest_sql(
            r#"
-- Stores users
-- Does not include magic users.
CREATE TABLE users (
    -- ID from the auth system
    sub_id UUID NOT NULL,

    -- Hash of user's password,
    -- or null if no password is set.
    -- We use Argon2 in the PHC format
    password_hash TEXT
);

ALTER TABLE users
    -- whether the user is barred from logging in
    ADD COLUMN locked BOOLEAN DEFAULT FALSE;

-- whether the user is all powerful
-- and able to do anything they want
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
        "#,
        );

        assert_yaml_snapshot!(h, @r#"
        tables:
          - name: users
            doc_comment:
              comment: "Stores users\nDoes not include magic users."
              line_number: 2
            columns:
              password_hash:
                name: password_hash
                doc_comment:
                  comment: "Hash of user's password,\nor null if no password is set.\nWe use Argon2 in the PHC format"
                  line_number: 8
                table_name: users
              sub_id:
                name: sub_id
                doc_comment:
                  comment: ID from the auth system
                  line_number: 5
                table_name: users
        alter_columns:
          - name: locked
            doc_comment:
              comment: whether the user is barred from logging in
              line_number: 15
            table_name: users
          - name: admin
            doc_comment:
              comment: "whether the user is all powerful\nand able to do anything they want"
              line_number: 18
            table_name: users
        "#);
    }
}
