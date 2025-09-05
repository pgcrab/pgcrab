// SPDX-FileCopyrightText: 2025 Olivier 'reivilibre'
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{path::Path, sync::Arc};

use crate::doc::intermediate::IntermediateSchema;
use eyre::Context;
use lazy_static::lazy_static;
use minijinja::Environment;
use regex::{Captures, Regex};

pub fn render_schema(schema: &IntermediateSchema, template_path: &Path) -> eyre::Result<String> {
    let mut jinja = Environment::new();
    let template = std::fs::read_to_string(template_path).context("failed to read template")?;
    jinja
        .add_template("schema.md", &template)
        .context("failed to parse template")?;

    let arc_schema = Arc::new(schema.to_owned());
    jinja.add_filter("linkify_schema_elements_markdown", move |text| {
        linkify_schema_elements_markdown(text, &arc_schema)
    });

    jinja
        .get_template("schema.md")?
        .render(schema)
        .context("failed to render template")
}

lazy_static! {
    static ref LINKIFY_MD_REGEX: Regex =
        Regex::new(r#"\b{start-half}`([a-zA-Z0-9-_]+)`\b{end-half}"#).unwrap();
}

// TODO it's a bit nasty that we imply that Jinja will copy `list_of_tables` to pass it to us...
fn linkify_schema_elements_markdown(text: String, schema: &IntermediateSchema) -> String {
    LINKIFY_MD_REGEX
        .replace_all(&text, |captures: &Captures| {
            let object_name = &captures[1];

            if schema.tables.contains_key(object_name) {
                // this looks like a table name; insert a link
                // TODO get the anchor logic exactly right
                format!("[{}](#{})", &captures[0], object_name.replace("_", "-"))
            } else {
                // don't linkify; leave it alone
                captures[0].to_owned()
            }
        })
        .into_owned()
}
