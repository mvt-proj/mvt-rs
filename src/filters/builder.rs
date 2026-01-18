// builder.rs
use crate::filters::types::{FilterCondition, LogicalOp, Operator};

pub struct SqlQueryBuilder {
    param_index: usize,
    bindings: Vec<String>,
}

impl SqlQueryBuilder {
    pub fn new(start_index: usize) -> Self {
        Self {
            param_index: start_index,
            bindings: Vec::new(),
        }
    }

    pub fn build(&mut self, filters: &[FilterCondition]) -> (String, Vec<String>) {
        let mut and_parts = Vec::new();
        let mut or_parts = Vec::new();
        let mut not_parts = Vec::new();

        for filter in filters {
            let condition = self.create_condition(filter);
            match filter.logic {
                LogicalOp::And => and_parts.push(condition),
                LogicalOp::Or => or_parts.push(condition),
                LogicalOp::Not => not_parts.push(condition),
            }
        }

        let mut final_clause = String::new();

        let mut append = |parts: Vec<String>, joiner: &str, wrapper: Option<(&str, &str)>| {
            if !parts.is_empty() {
                if !final_clause.is_empty() {
                    final_clause.push_str(" AND ");
                }
                if let Some((prefix, suffix)) = wrapper {
                    final_clause.push_str(prefix);
                    final_clause.push_str(&parts.join(joiner));
                    final_clause.push_str(suffix);
                } else {
                    final_clause.push_str(&parts.join(joiner));
                }
            }
        };

        append(and_parts, " AND ", None);
        append(or_parts, " OR ", Some(("(", ")")));

        for not_cond in not_parts {
            if !final_clause.is_empty() {
                final_clause.push_str(" AND ");
            }
            final_clause.push_str(&format!("NOT ({})", not_cond));
        }

        (final_clause, self.bindings.clone())
    }

    fn create_condition(&mut self, filter: &FilterCondition) -> String {
        match filter.operator {
            Operator::In => {
                let values: Vec<&str> = filter
                    .value
                    .split(',')
                    .map(|s| s.trim())
                    .map(|s| s.trim_matches('\''))
                    .filter(|s| !s.is_empty())
                    .collect();

                if values.is_empty() {
                    return "1=0".to_string();
                }

                let is_numeric = values
                    .first()
                    .map_or(false, |v| v.chars().all(|c| c.is_numeric()));

                let mut placeholders = Vec::new();
                for v in values {
                    if is_numeric {
                        placeholders.push(format!("${}::int", self.param_index));
                    } else {
                        placeholders.push(format!("${}", self.param_index));
                    }

                    self.bindings.push(v.to_string());
                    self.param_index += 1;
                }

                format!("{} IN ({})", filter.field, placeholders.join(", "))
            }
            _ => {
                let sql = format!(
                    "{} {} ${}",
                    filter.field,
                    filter.operator.as_sql(),
                    self.param_index
                );
                self.bindings.push(filter.value.clone());
                self.param_index += 1;
                sql
            }
        }
    }
}
