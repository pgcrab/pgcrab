use std::path::PathBuf;

use clap::Parser;
use eyre::Context;
use postgres::NoTls;
use postgres_schema_documenter::{gather, render};

#[derive(Parser)]
#[command(version = env!("CARGO_PKG_VERSION"), author = env!("CARGO_PKG_AUTHORS"), about = env!("CARGO_PKG_DESCRIPTION"))]
pub struct Options {
    #[clap(short = 'C', long = "postgres-conn", env = "POSTGRES_CONN")]
    postgres_connection_string: Option<String>,

    #[clap(subcommand)]
    subcommand: Subcommand,
}

#[derive(Parser)]
pub enum Subcommand {
    Doc {
        #[clap(short = 't', long = "template", env = "TEMPLATE")]
        template: PathBuf,
    },
    ListUncommented {},
}

fn main() -> eyre::Result<()> {
    let opts = Options::parse();

    let mut db_conn = match opts.postgres_connection_string {
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
    };

    let schema = gather::gather_schema(&mut db_conn).context("failed to gather schema")?;
    // eprintln!("{schema:#?}");

    match &opts.subcommand {
        Subcommand::Doc { template } => {
            let rendered = render::render_schema(&schema, template)?;

            print!("{rendered}");
        }
        Subcommand::ListUncommented {} => {
            for (table_name, table) in &schema.tables {
                if table.comment.is_empty() {
                    println!("TABLE {table_name}");
                }

                for column in &table.columns {
                    if column.comment.is_empty() {
                        println!("COLUMN {} ON TABLE {table_name}", column.name);
                    }
                }
            }
        }
    }

    Ok(())
}
