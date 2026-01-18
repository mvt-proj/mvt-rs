// parser.rs
use crate::filters::types::{FilterCondition, LogicalOp, Operator};
use std::collections::HashMap;

fn strip_single_quotes(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].replace("''", "'")
    } else {
        trimmed.to_string()
    }
}

pub fn parse_query_params(query: &HashMap<String, String>) -> Vec<FilterCondition> {
    query
        .iter()
        .filter_map(|(key, value)| {
            let (logic, key_clean) = if key.starts_with("or__") {
                (LogicalOp::Or, key.trim_start_matches("or__"))
            } else if key.starts_with("not__") {
                (LogicalOp::Not, key.trim_start_matches("not__"))
            } else {
                (LogicalOp::And, key.as_str())
            };

            let parts: Vec<&str> = key_clean.split("__").collect();
            let (field, operator_enum) = match parts.len() {
                1 => (parts[0].to_string(), Operator::Eq),
                2 => {
                    let op = Operator::from_str(parts[1])?; // Retorna None si el op es inválido
                    (parts[0].to_string(), op)
                }
                _ => return None,
            };

            let clean_value = match operator_enum {
                Operator::Eq => strip_single_quotes(value),
                _ => value.to_string(),
            };

            Some(FilterCondition {
                field,
                operator: operator_enum,
                value: clean_value,
                logic,
            })
        })
        .collect()
}
