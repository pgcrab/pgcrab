// SPDX-License-Identifier: GPL-3.0-or-later

use super::{gather_comments::TotalHarvest, intermediate::IntermediateSchema};

/// Augment an `IntermediateSchema` with comments harvested from SQL files.
///
/// If `harvest_higher_priority` is true, then the comments harvested from SQL
/// files override those found in the schema itself.
pub fn combine_harvested_comments_into_schema(
    schema: &mut IntermediateSchema,
    harvest: &TotalHarvest,
    harvest_higher_priority: bool,
) {
    for (table_name, table) in &mut schema.tables {
        let Some(harvest_table) = harvest.tables.get(table_name) else {
            continue;
        };

        if let Some(harvest_tbl_comment) = &harvest_table.doc_comment {
            if table.comment.is_empty() || harvest_higher_priority {
                table.comment = harvest_tbl_comment.comment.clone();
            }
        }

        for column in &mut table.columns {
            let Some(harvest_column) = harvest_table.columns.get(&column.name) else {
                continue;
            };
            if let Some(harvest_col_comment) = &harvest_column.doc_comment {
                if column.comment.is_empty() || harvest_higher_priority {
                    column.comment = harvest_col_comment.comment.clone();
                }
            }
        }
    }
}
