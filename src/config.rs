use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use eyre::Context;
use fast_glob::glob_match;
use serde::Deserialize;
use strum::IntoEnumIterator;

use crate::lint::schema::SchemaDiagnosticRule;

/// Where to find the pgCrab config file, relative to a directory it applies within.
/// `.config` convention inspired by the XDG config convention and https://dot-config.github.io/.
pub const CONFIG_RELATIVE_PATH: &str = ".config/pgcrab.toml";

/// Known file and directory names that identify the root of a version-controlled repository.
const VERSION_CONTROL_ROOT_RELATIVE_PATHS: &[&str] =
    &[".git", ".svn", ".hg", ".bzr", "_darcs", ".pijul"];

pub enum ConfigFileFind {
    /// Found a config file at the given path!
    Found { config_path: PathBuf },
    /// We kept searching for config files but hit the repository root
    /// before finding one.
    StoppedAtVersionControl {
        /// Root of the version-controlled repository
        repository_root: PathBuf,
    },
    /// Stopped at the root directory or user's $HOME.
    /// Did not find a version-controlled repository.
    StoppedAtHardBoundary,
}

pub fn find_config_file(
    starting_at_dir: &Path,
    home_dir: Option<&Path>,
) -> eyre::Result<ConfigFileFind> {
    let mut currently_at = starting_at_dir;
    loop {
        // See if the directory contains a config file
        let try_config_path = currently_at.join(".config/pgcrab.toml");
        if std::fs::exists(&try_config_path)? {
            return Ok(ConfigFileFind::Found {
                config_path: try_config_path,
            });
        }

        // See if we should stop because this directory is a repo root
        for repo_root_relative_path in VERSION_CONTROL_ROOT_RELATIVE_PATHS {
            if std::fs::exists(currently_at.join(repo_root_relative_path))? {
                return Ok(ConfigFileFind::StoppedAtVersionControl {
                    repository_root: currently_at.to_owned(),
                });
            }
        }

        // See if we should stop because we would now be ascending out of the user's $HOME
        if Some(currently_at) == home_dir {
            return Ok(ConfigFileFind::StoppedAtHardBoundary);
        }

        // Ascend, unless this is the root in which case we've exhausted the search.
        let Some(new_currently_at) = currently_at.parent() else {
            return Ok(ConfigFileFind::StoppedAtHardBoundary);
        };
        currently_at = new_currently_at;
    }
}

pub fn find_config_from_cwd_and_env() -> eyre::Result<ConfigFileFind> {
    let home_dir = std::env::var("HOME")
        .ok()
        .and_then(|v| PathBuf::try_from(v).ok())
        .filter(|p| p.exists());

    find_config_file(
        &std::env::current_dir().context("could not get current working dir")?,
        home_dir.as_ref().map(|p| p.as_ref()),
    )
}

pub fn find_and_load_optional_config() -> eyre::Result<(Config, ConfigFileFind)> {
    let config_file_find = find_config_from_cwd_and_env().context("failed to look for config")?;

    let ConfigFileFind::Found { config_path } = &config_file_find else {
        return Ok((Config::default(), config_file_find));
    };

    let config_bytes = std::fs::read(config_path)
        .with_context(|| format!("could not read config at {config_path:?}"))?;
    let config: Config = toml_edit::de::from_slice(&config_bytes)
        .with_context(|| format!("could not deserialise config at {config_path:?}"))?;

    for warning in config.check_for_warnings() {
        let Warning { key, message } = &warning;
        eprintln!("[config warning] at {key}: {message}");
    }

    Ok((config, config_file_find))
}

#[derive(Default, Deserialize)]
pub struct Config {
    pub schema: SchemaConfig,
}

impl Config {
    pub fn check_for_warnings(&self) -> Vec<Warning> {
        self.schema.check_for_warnings()
    }
}

#[derive(Default, Deserialize)]
pub struct SchemaConfig {
    /// Rule ID to list of patterns matching objects that are exempt from the rule.
    #[serde(default)]
    pub concessions: BTreeMap<String, Vec<String>>,
}

impl SchemaConfig {
    pub fn check_for_warnings(&self) -> Vec<Warning> {
        let mut out = Vec::new();

        'next_concession: for concession_rule_pattern in self.concessions.keys() {
            for rule in SchemaDiagnosticRule::iter() {
                if glob_match(
                    concession_rule_pattern.as_bytes(),
                    <&'static str>::from(rule).as_bytes(),
                ) {
                    continue 'next_concession;
                }
            }

            out.push(Warning {
                key: format!("schema.concessions.{concession_rule_pattern:?}"),
                message:
                    "Concession rule does not match any schema rules in this version of pgCrab."
                        .to_owned(),
            });
        }

        out
    }
}

#[derive(Debug)]
pub struct Warning {
    pub key: String,
    pub message: String,
}
