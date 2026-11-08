use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub fn check_if_optional(records: &[Value], field: &str) -> bool {
    records.iter().any(|r| {
        r.as_object()
            .map(|o| !o.contains_key(field) || o[field].is_null())
            .unwrap_or(true)
    })
}

pub fn detect_id_field(fields: &HashMap<String, HashSet<String>>) -> Option<String> {
    fields.keys().find(|k| k.to_lowercase() == "id").cloned()
}

pub fn detect_group_field(fields: &HashMap<String, HashSet<String>>) -> Option<String> {
    fields
        .keys()
        .filter(|k| {
            let l = k.to_lowercase();
            l.contains("group") && l.contains("id")
        })
        .min()
        .cloned()
}

pub fn has_vec_group_type(
    fields: &HashMap<String, HashSet<String>>,
    group: Option<&String>,
) -> bool {
    group
        .and_then(|g| fields.get(g))
        .map(|types| types.iter().any(|t| t.starts_with("Vec<")))
        .unwrap_or(false)
}

pub fn get_id_type(fields: &HashMap<String, HashSet<String>>, id: &str) -> String {
    fields
        .get(id)
        .and_then(|t| t.iter().next())
        .map(|t| {
            t.strip_prefix("Option<")
                .and_then(|s| s.strip_suffix('>'))
                .unwrap_or(t)
                .to_string()
        })
        .unwrap_or_else(|| "i32".to_string())
}
