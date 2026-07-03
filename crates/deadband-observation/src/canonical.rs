use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;

pub fn strip_volatile_fields(value: &mut Value, paths: &[String]) {
    for path_str in paths {
        let segments = parse_json_path(path_str);
        if segments.is_empty() {
            continue;
        }
        let (last, parent_segs) = segments.split_last().unwrap();
        let parent_path = if parent_segs.is_empty() {
            String::new()
        } else {
            format!("/{}", parent_segs.join("/"))
        };

        let target = if parent_segs.is_empty() {
            Some(&mut *value)
        } else {
            value.pointer_mut(&parent_path)
        };

        if let Some(Value::Object(map)) = target {
            map.remove(last);
        }
    }
}

fn parse_json_path(path: &str) -> Vec<String> {
    let s = if let Some(stripped) = path.strip_prefix("$.") {
        stripped
    } else {
        path
    };
    s.split('.').map(|p| p.to_string()).collect()
}

pub fn auto_infer_volatile_fields(
    current_args: &Value,
    history: &[(&str, &Value)],  // (tool_name, arguments) pairs from history
    tool_name: &str,
    min_occurrences: usize,
) -> Vec<String> {
    // Only process JSON objects
    let current_obj = match current_args {
        Value::Object(map) => map,
        _ => return Vec::new(),
    };

    // Filter history to same tool
    let same_tool: Vec<&Value> = history
        .iter()
        .filter(|(t, _)| *t == tool_name)
        .map(|(_, a)| *a)
        .collect();

    if same_tool.len() < min_occurrences {
        return Vec::new();
    }

    let mut field_diff_counts: HashMap<String, usize> = HashMap::new();
    let mut auto_inferred: HashSet<String> = HashSet::new();

    for prior_args in &same_tool {
        let prior_obj = match prior_args {
            Value::Object(map) => map,
            _ => continue,
        };

        let mut diffs = Vec::new();
        for (k, v) in current_obj.iter() {
            if prior_obj.get(k) != Some(v) {
                diffs.push(k.clone());
            }
        }
        for (k, _) in prior_obj.iter() {
            if !current_obj.contains_key(k) && !diffs.contains(k) {
                diffs.push(k.clone());
            }
        }

        // Track which fields differ
        for diff in &diffs {
            *field_diff_counts.entry(diff.clone()).or_insert(0) += 1;
        }

        // If exactly one field differs, it's a candidate for auto-inference
        if diffs.len() == 1 {
            auto_inferred.insert(diffs[0].clone());
        }
    }

    // Only keep fields that differ in at least `min_occurrences` prior calls
    auto_inferred.retain(|field| {
        let count = field_diff_counts.get(field).copied().unwrap_or(0);
        count >= min_occurrences
    });

    auto_inferred.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_strip_simple_field() {
        let mut val = json!({"query": "hello", "req_id": 123});
        strip_volatile_fields(&mut val, &["$.req_id".to_string()]);
        assert_eq!(val, json!({"query": "hello"}));
    }

    #[test]
    fn test_strip_nested_field() {
        let mut val = json!({"meta": {"timestamp": 1000, "data": "x"}});
        strip_volatile_fields(&mut val, &["$.meta.timestamp".to_string()]);
        assert_eq!(val, json!({"meta": {"data": "x"}}));
    }

    #[test]
    fn test_strip_multiple_fields() {
        let mut val = json!({"query": "hello", "req_id": 1, "ts": 100});
        strip_volatile_fields(&mut val, &["$.req_id".to_string(), "$.ts".to_string()]);
        assert_eq!(val, json!({"query": "hello"}));
    }

    #[test]
    fn test_auto_infer_detects_volatile_req_id() {
        let current = json!({"query": "python", "req_id": 3});
        let h1 = json!({"query": "python", "req_id": 1});
        let h2 = json!({"query": "python", "req_id": 2});
        let history = vec![
            ("search", &h1),
            ("search", &h2),
        ];
        let inferred = auto_infer_volatile_fields(&current, &history, "search", 2);
        assert_eq!(inferred, vec!["req_id"]);
    }

    #[test]
    fn test_auto_infer_requires_min_occurrences() {
        let current = json!({"query": "python", "req_id": 2});
        let h1 = json!({"query": "python", "req_id": 1});
        let history = vec![
            ("search", &h1),
        ];
        let inferred = auto_infer_volatile_fields(&current, &history, "search", 2);
        assert!(inferred.is_empty());
    }

    #[test]
    fn test_auto_infer_no_false_positive_when_query_changes() {
        let current = json!({"query": "rust", "req_id": 3});
        let h1 = json!({"query": "python", "req_id": 1});
        let h2 = json!({"query": "java", "req_id": 2});
        let history = vec![
            ("search", &h1),
            ("search", &h2),
        ];
        let inferred = auto_infer_volatile_fields(&current, &history, "search", 2);
        assert!(inferred.is_empty());
    }

    #[test]
    fn test_auto_infer_requires_field_diff_in_multiple_prior_calls() {
        let current = json!({"query": "python", "req_id": 3});
        let h1 = json!({"query": "python", "req_id": 1});
        let h2 = json!({"query": "python", "req_id": 2});
        let history = vec![
            ("search", &h1),
            ("search", &h2),
        ];
        let inferred = auto_infer_volatile_fields(&current, &history, "search", 2);
        assert_eq!(inferred, vec!["req_id"]);
    }

    #[test]
    fn test_strip_field_not_present() {
        let mut val = json!({"query": "hello"});
        strip_volatile_fields(&mut val, &["$.nonexistent".to_string()]);
        assert_eq!(val, json!({"query": "hello"}));
    }

    #[test]
    fn test_strip_non_object() {
        let mut val = json!("string_value");
        strip_volatile_fields(&mut val, &["$.field".to_string()]);
        assert_eq!(val, json!("string_value"));
    }
}
