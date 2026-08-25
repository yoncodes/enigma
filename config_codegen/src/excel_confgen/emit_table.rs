use serde_json::Value;
use std::collections::{HashMap, HashSet};

use crate::excel_confgen::{
    rust_ident::{camel_to_snake, snake_to_pascal},
    schema_inspect::{
        check_if_optional, detect_group_field, detect_id_field, get_id_type, has_vec_group_type,
    },
};

pub fn emit_table_store(
    table_name: &str,
    field_types: &HashMap<String, HashSet<String>>,
    records: &[Value],
) -> String {
    let struct_name = snake_to_pascal(table_name);

    let id_field = detect_id_field(field_types);
    let group_field = detect_group_field(field_types);

    // Act128 reward ids repeat per activity, so a global id index would be lossy.
    let has_id_index = id_field.is_some() && table_name != "activity128_rewards";
    let has_group_index =
        group_field.is_some() && !has_vec_group_type(field_types, group_field.as_ref());

    let id_type = id_field
        .as_ref()
        .map(|f| get_id_type(field_types, f))
        .unwrap_or_else(|| "i32".to_string());

    let group_type = group_field
        .as_ref()
        .and_then(|f| field_types.get(f))
        .and_then(|types| types.iter().next())
        .cloned()
        .unwrap_or_else(|| "i32".to_string());

    let mut out = Vec::new();

    if has_id_index || has_group_index {
        out.push("use std::collections::HashMap;".into());
        out.push("".into());
    }

    out.push(format!("pub struct {}Table {{", struct_name));
    out.push(format!("    records: Vec<{}>,", struct_name));

    if has_id_index {
        out.push(format!("    by_id: HashMap<{}, usize>,", id_type));
    }

    if has_group_index {
        out.push(format!(
            "    by_group: HashMap<{}, Vec<usize>>,",
            group_type
        ));
    }

    out.push("}".into());
    out.push("".into());

    out.push(format!("impl {}Table {{", struct_name));
    out.push("    pub fn load(path: &str) -> anyhow::Result<Self> {".into());
    out.push(format!(
        "        let records: Vec<{}> = crate::load_rows(path)?;",
        struct_name
    ));
    out.push("".into());

    if has_id_index || has_group_index {
        if has_id_index {
            out.push("        let mut by_id = HashMap::with_capacity(records.len());".into());
        }
        if has_group_index {
            out.push(format!(
                "        let mut by_group: HashMap<{}, Vec<usize>> = HashMap::new();",
                group_type
            ));
        }

        out.push("".into());
        out.push("        for (idx, record) in records.iter().enumerate() {".into());

        if let Some(id_field) = &id_field {
            let id_snake = camel_to_snake(id_field);
            let optional = check_if_optional(records, id_field);

            if optional {
                out.push(format!(
                    "            if let Some(id) = &record.{} {{",
                    id_snake
                ));
                if id_type == "String" {
                    out.push("                by_id.insert(id.clone(), idx);".into());
                } else {
                    out.push("                by_id.insert(*id, idx);".into());
                }
                out.push("            }".into());
            } else if id_type == "String" {
                out.push(format!(
                    "            by_id.insert(record.{}.clone(), idx);",
                    id_snake
                ));
            } else {
                out.push(format!(
                    "            by_id.insert(record.{}, idx);",
                    id_snake
                ));
            }
        }

        if has_group_index && let Some(group_field) = &group_field {
            let group_snake = camel_to_snake(group_field);
            let optional = check_if_optional(records, group_field);

            if optional {
                out.push(format!(
                    "            if let Some(group_id) = &record.{} {{",
                    group_snake
                ));
                if group_type == "String" {
                    out.push(
                        "                by_group.entry(group_id.clone()).or_default().push(idx);"
                            .into(),
                    );
                } else {
                    out.push(
                        "                by_group.entry(*group_id).or_default().push(idx);".into(),
                    );
                }
                out.push("            }".into());
            } else if group_type == "String" {
                out.push(format!(
                    "            by_group.entry(record.{}.clone()).or_default().push(idx);",
                    group_snake
                ));
            } else {
                out.push(format!(
                    "            by_group.entry(record.{}).or_default().push(idx);",
                    group_snake
                ));
            }
        }

        out.push("        }".into());
        out.push("".into());
    }

    out.push("        Ok(Self {".into());
    out.push("            records,".into());

    if has_id_index {
        out.push("            by_id,".into());
    }
    if has_group_index {
        out.push("            by_group,".into());
    }

    out.push("        })".into());
    out.push("    }".into());
    out.push("".into());

    if has_id_index {
        out.push("    #[inline]".into());
        out.push(format!(
            "    pub fn get(&self, id: {}) -> Option<&{}> {{",
            id_type, struct_name
        ));
        out.push("        self.by_id.get(&id).map(|&i| &self.records[i])".into());
        out.push("    }".into());
        out.push("".into());
    }

    if has_group_index {
        out.push(format!(
            "    pub fn by_group(&self, group_id: {}) -> impl Iterator<Item = &'_ {}> + '_ {{",
            group_type, struct_name
        ));
        out.push("        self.by_group".into());
        out.push("            .get(&group_id)".into());
        out.push("            .into_iter()".into());
        out.push("            .flat_map(|idxs| idxs.iter())".into());
        out.push("            .map(|&i| &self.records[i])".into());
        out.push("    }".into());
        out.push("".into());
    }

    out.push("    #[inline]".into());
    out.push(format!("    pub fn all(&self) -> &[{}] {{", struct_name));
    out.push("        &self.records".into());
    out.push("    }".into());
    out.push("".into());

    out.push("    #[inline]".into());
    out.push(format!(
        "    pub fn iter(&self) -> std::slice::Iter<'_, {}> {{",
        struct_name
    ));
    out.push("        self.records.iter()".into());
    out.push("    }".into());
    out.push("".into());

    out.push("    pub fn len(&self) -> usize { self.records.len() }".into());
    out.push("    pub fn is_empty(&self) -> bool { self.records.is_empty() }".into());

    out.push("}".into());

    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::excel_confgen::schema_infer::analyze_field_types;
    use serde_json::json;

    #[test]
    fn act128_rewards_do_not_expose_a_lossy_global_id_lookup() {
        let records = vec![
            json!({"activityId": 1, "id": 1}),
            json!({"activityId": 2, "id": 1}),
        ];
        let output = emit_table_store(
            "activity128_rewards",
            &analyze_field_types(&records),
            &records,
        );

        assert!(!output.contains("by_id"));
        assert!(!output.contains("pub fn get("));
    }
}
