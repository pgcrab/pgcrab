// SPDX-FileCopyrightText: 2025 Olivier 'reivilibre'
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::BTreeMap, fs::File, path::PathBuf, process::ExitCode};

use clap::Parser;
use colored::Colorize;
use eyre::Context;
use fast_glob::glob_match;
use itertools::Itertools;
use pgcrab::{
    config::{find_and_load_optional_config, ConfigFileFind, CONFIG_RELATIVE_PATH},
    doc::{
        combine::combine_harvested_comments_into_schema,
        convert::filter_harvested_by_schema_comparison, gather_comments::harvest_from_paths,
        gather_database, render,
    },
    lint::{
        schema::{self, SchemaLoc},
        DiagnosticClassification,
    },
};
use postgres::NoTls;

#[derive(Parser)]
#[command(version = env!("CARGO_PKG_VERSION"), author = env!("CARGO_PKG_AUTHORS"), about = env!("CARGO_PKG_DESCRIPTION"))]
pub struct Options {
    #[clap(short = 'C', long = "conn", env = "DATABASE_URI")]
    postgres_connection_string: Option<String>,

    #[clap(subcommand)]
    subcommand: Subcommand,
}

#[derive(Parser)]
pub enum Subcommand {
    Doc {
        #[clap(subcommand)]
        cmd: DocCommand,
    },
    LintSchema {
        /// Add concession rules to ignore produced lints in the future.
        ///
        /// Will not work if a config file is not found and
        /// the current directory is not in a version controlled repository.
        #[clap(long = "add-concessions")]
        add_concessions: bool,
    },
}

#[derive(Parser)]
pub enum DocCommand {
    /// Generate documentation.
    Gen {
        #[clap(short = 't', long = "template", env = "TEMPLATE")]
        template: PathBuf,

        /// If specified, SQL comments (--) preceding some DDL statements will be
        /// harvested from SQL files at or in this path,
        /// in order to supplement missing comments in the database schema.
        #[clap(long = "harvest")]
        harvest: Vec<PathBuf>,
    },
    /// Show a list of uncommented items in the database.
    Uncommented {
        /// If specified, SQL comments (--) preceding some DDL statements will be
        /// harvested from SQL files at or in this path,
        /// in order to supplement missing comments in the database schema.
        #[clap(long = "harvest")]
        harvest: Vec<PathBuf>,
    },
    /// Emit `COMMENT ON` statements for SQL comments (--) present in SQL files but
    /// missing in the database.
    /// If running on historical SQL migrations, that have been invalidated by DROP
    /// or RENAME statements, `--compare` must be used.
    Convert {
        /// Connect to a database and only emit `COMMENT ON` statements for real
        /// database objects that do not already have the expected comment.
        #[clap(long = "compare")]
        compare: bool,
        /// SQL comments (--) preceding some DDL statements will be
        /// harvested from SQL files at or in this path.
        #[clap(required = true)]
        harvest: Vec<PathBuf>,
    },
}

fn main() -> eyre::Result<ExitCode> {
    let opts = Options::parse();

    let make_db_conn = || {
        Ok::<_, eyre::Error>(match opts.postgres_connection_string {
            Some(ref conn_str) => postgres::Client::connect(conn_str.as_str(), NoTls)
                .context("failed to connect using provided connection string")?,
            None => {
                // connect using defaults and PGxxx env vars
                let mut config = postgres::Client::configure();

                if let Some(host) = std::env::var("PGHOST").ok() {
                    config.host(&host);
                }
                if let Some(user) = std::env::var("PGUSER").ok() {
                    config.user(&user);
                }
                if let Some(db) = std::env::var("PGDATABASE").ok() {
                    config.dbname(&db);
                }
                config
                    .connect(NoTls)
                    .context("failed to connect using env vars")?
            }
        })
    };

    match &opts.subcommand {
        Subcommand::Doc { cmd } => match cmd {
            DocCommand::Gen { template, harvest } => {
                let mut db_conn = make_db_conn()?;
                let mut schema = gather_database::gather_schema(&mut db_conn)
                    .context("failed to gather schema")?;
                let harvested = harvest_from_paths(harvest)
                    .context("failed to harvest comments from SQL migrations")?;

                combine_harvested_comments_into_schema(&mut schema, &harvested, false);

                let rendered = render::render_schema(&schema, template)?;

                print!("{rendered}");

                Ok(ExitCode::SUCCESS)
            }
            DocCommand::Uncommented { harvest } => {
                let mut db_conn = make_db_conn()?;
                let mut schema = gather_database::gather_schema(&mut db_conn)
                    .context("failed to gather schema")?;
                let harvested = harvest_from_paths(harvest)
                    .context("failed to harvest comments from SQL migrations")?;

                combine_harvested_comments_into_schema(&mut schema, &harvested, false);

                for (table_name, table) in &schema.tables {
                    if table.comment.is_empty() {
                        println!("TABLE {table_name}");
                    }

                    for column in &table.columns {
                        if column.comment.is_empty() {
                            println!("COLUMN {table_name}.{}", column.name);
                        }
                    }
                }

                Ok(ExitCode::SUCCESS)
            }
            DocCommand::Convert { compare, harvest } => {
                let mut harvested = harvest_from_paths(harvest)
                    .context("failed to harvest comments from SQL migrations")?;

                if *compare {
                    let mut db_conn = make_db_conn()?;
                    let schema = gather_database::gather_schema(&mut db_conn)
                        .context("failed to gather schema")?;
                    filter_harvested_by_schema_comparison(&mut harvested, &schema);
                }

                fn emit_comment(comment: &str) {
                    let mut tag = String::new();
                    let mut tag_num = 0;
                    while comment.contains(&format!("${tag}$")) {
                        tag_num += 1;
                        tag = tag_num.to_string();
                    }
                    println!("    ${tag}${comment}${tag}$;");
                }

                for (table_name, table) in &harvested.tables {
                    let mut emitted_table = false;
                    if let Some(comment) = &table.doc_comment {
                        println!("COMMENT ON TABLE {table_name} IS");
                        emit_comment(&comment.comment);
                        emitted_table = true;
                    }
                    for (column_name, column) in &table.columns {
                        if let Some(comment) = &column.doc_comment {
                            println!("COMMENT ON COLUMN {table_name}.{column_name} IS");
                            emit_comment(&comment.comment);
                            emitted_table = true;
                        }
                    }
                    if emitted_table {
                        println!();
                    }
                }
                Ok(ExitCode::SUCCESS)
            }
        },
        Subcommand::LintSchema { add_concessions } => {
            let mut db_conn = make_db_conn()?;
            let mut txn = db_conn
                .transaction()
                .context("could not start transaction")?;
            let mut diagnostics = schema::lint_all(&mut txn).context("could not lint schema")?;
            diagnostics.sort_by_key(|d| <&'static str>::from(d.rule));
            txn.rollback()?;

            let (config, config_find) = find_and_load_optional_config()?;

            if !config.schema.concessions.is_empty() {
                diagnostics = diagnostics
                    .into_iter()
                    .filter(|diagnostic| {
                        let diagnostic_loc_matchable =
                            diagnostic.loc.to_concession_matchable_string();
                        for (concession_rule_pattern, object_patterns) in &config.schema.concessions
                        {
                            if glob_match(
                                concession_rule_pattern.as_bytes(),
                                <&'static str>::from(diagnostic.rule).as_bytes(),
                            ) {
                                for object_pattern in object_patterns {
                                    // Replace `.` with `/` to satisfy glob rules for * vs **
                                    let converted_object_pattern = object_pattern.replace('.', "/");
                                    if glob_match(
                                        converted_object_pattern.as_bytes(),
                                        diagnostic_loc_matchable.as_bytes(),
                                    ) {
                                        // This diagnostic should be ignored
                                        return false;
                                    }
                                }
                            }
                        }

                        // This diagnostic was not filtered out
                        true
                    })
                    .collect();
            }

            let mut counts: BTreeMap<DiagnosticClassification, u32> = BTreeMap::new();

            for diagnostic in &diagnostics {
                let classification = diagnostic.rule.default_classification();
                let description = diagnostic.rule.describe();
                *counts.entry(classification).or_default() += 1;

                let mut lines = description.split("\n");
                let firstline = lines.next().unwrap();

                println!(
                    "{}",
                    format!(
                        "{} [{}]: {}",
                        classification.to_string(),
                        diagnostic.rule.to_string(),
                        firstline.custom_color(c(catppuccin::PALETTE.mocha.colors.text))
                    )
                    .custom_color(c(match classification {
                        DiagnosticClassification::Note => catppuccin::PALETTE.mocha.colors.mauve,
                        DiagnosticClassification::Warning =>
                            catppuccin::PALETTE.mocha.colors.yellow,
                        DiagnosticClassification::Error => catppuccin::PALETTE.mocha.colors.red,
                    })),
                );

                match &diagnostic.loc {
                    SchemaLoc::Table { table } => {
                        println!(
                            "  {}{}",
                            "--> on table ".custom_color(c(catppuccin::PALETTE.mocha.colors.teal)),
                            table
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.pink))
                                .bold()
                        )
                    }
                    SchemaLoc::Column { table, column } => {
                        println!(
                            "  {}{}{}{}",
                            "--> on table ".custom_color(c(catppuccin::PALETTE.mocha.colors.teal)),
                            table
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.pink))
                                .bold(),
                            ", column ".custom_color(c(catppuccin::PALETTE.mocha.colors.teal)),
                            column
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.pink))
                                .bold(),
                        )
                    }
                    SchemaLoc::Object { object, kind } => {
                        println!(
                            "  {} {} {}",
                            "--> on".custom_color(c(catppuccin::PALETTE.mocha.colors.teal)),
                            kind.custom_color(c(catppuccin::PALETTE.mocha.colors.teal)),
                            object
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.pink))
                                .bold()
                        )
                    }
                    SchemaLoc::Index { table, index } => {
                        println!(
                            "  {}{}{}{}",
                            "--> on table ".custom_color(c(catppuccin::PALETTE.mocha.colors.teal)),
                            table
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.pink))
                                .bold(),
                            ", index ".custom_color(c(catppuccin::PALETTE.mocha.colors.teal)),
                            index
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.pink))
                                .bold(),
                        )
                    }
                    SchemaLoc::Indexes { table, indexes } => {
                        println!(
                            "  {}{}{}{{{}}}",
                            "--> on table ".custom_color(c(catppuccin::PALETTE.mocha.colors.teal)),
                            table
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.pink))
                                .bold(),
                            ", indexes ".custom_color(c(catppuccin::PALETTE.mocha.colors.teal)),
                            indexes
                                .iter()
                                .join(", ")
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.pink))
                                .bold(),
                        )
                    }
                    SchemaLoc::ForeignKey {
                        table,
                        target_table,
                        foreign_key,
                    } => {
                        println!(
                            "  {}{}{}{} {} {}",
                            "--> on table ".custom_color(c(catppuccin::PALETTE.mocha.colors.teal)),
                            table
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.pink))
                                .bold(),
                            ", foreign key ".custom_color(c(catppuccin::PALETTE.mocha.colors.teal)),
                            foreign_key
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.pink))
                                .bold(),
                            "with target table"
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.teal)),
                            target_table
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.pink))
                                .bold()
                        )
                    }
                    SchemaLoc::ForeignKeys {
                        table,
                        target_table,
                        foreign_keys,
                    } => {
                        println!(
                            "  {}{}{}{{{}}} {} {}",
                            "--> on table ".custom_color(c(catppuccin::PALETTE.mocha.colors.teal)),
                            table
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.pink))
                                .bold(),
                            ", foreign keys "
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.teal)),
                            foreign_keys
                                .iter()
                                .join(", ")
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.pink))
                                .bold(),
                            "with target table"
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.teal)),
                            target_table
                                .custom_color(c(catppuccin::PALETTE.mocha.colors.pink))
                                .bold()
                        )
                    }
                }

                for line in lines {
                    println!(
                        "  {}",
                        line.custom_color(c(catppuccin::PALETTE.mocha.colors.subtext0))
                    );
                }
                println!();
            }

            if !counts.is_empty() {
                for (classification, count) in counts {
                    println!(
                        "{}",
                        format!(
                            "{}: schema linting generated {} {}{}",
                            classification.to_string(),
                            count,
                            classification.to_string(),
                            if count > 1 { "s" } else { "" }
                        )
                        .custom_color(c(match classification {
                            DiagnosticClassification::Note =>
                                catppuccin::PALETTE.mocha.colors.mauve,
                            DiagnosticClassification::Warning =>
                                catppuccin::PALETTE.mocha.colors.yellow,
                            DiagnosticClassification::Error => catppuccin::PALETTE.mocha.colors.red,
                        })),
                    );
                }

                if *add_concessions {
                    match config_find {
                        ConfigFileFind::Found { config_path } => {
                            schema::add_concessions(&diagnostics, &config_path)
                                .context("failed to add concessions")?;
                        }
                        ConfigFileFind::StoppedAtVersionControl { repository_root } => {
                            let target_config_path = repository_root.join(CONFIG_RELATIVE_PATH);
                            let target_config_dir = target_config_path.parent().unwrap();
                            std::fs::create_dir_all(target_config_dir).with_context(|| {
                                format!("could not make dir at {target_config_dir:?}!")
                            })?;
                            drop(File::create(&target_config_path).with_context(|| {
                                format!("could not create config at {target_config_path:?}")
                            })?);
                            schema::add_concessions(&diagnostics, &target_config_path)
                                .context("failed to add concessions")?;
                        }
                        ConfigFileFind::StoppedAtHardBoundary => {
                            println!("Could not add concessions because there is no config file");
                            println!("and the current directory is not in a version controlled repository.");
                            println!(
                                "Either move to a repository or add a `{}` file!",
                                ".config/pgcrab.toml"
                                    .custom_color(c(catppuccin::PALETTE.mocha.colors.flamingo))
                            );
                        }
                    }
                } else {
                    println!(
                        "Use {} to add concession rules to suppress these lints in the future.",
                        "--add-concessions"
                            .custom_color(c(catppuccin::PALETTE.mocha.colors.flamingo))
                    );
                }
            }

            // TODO Allow more granularity in how different levels are exposed
            Ok(if diagnostics.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
    }
}

fn c(c: catppuccin::Color) -> (u8, u8, u8) {
    (c.rgb.r, c.rgb.g, c.rgb.b)
}
