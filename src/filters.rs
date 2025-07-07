use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
enum LogicalOp {
    And,
    Or,
    Not,
}

#[derive(Debug)]
pub struct FilterCondition {
    field: String,
    operator: String,
    value: String,
    logic: LogicalOp,
}

impl fmt::Display for FilterCondition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} '{}'", self.field, self.operator, self.value)
    }
}

/// Parses filters from query parameters.
///
/// - If the key starts with "or__", it is marked as an OR condition.
/// - If the key starts with "not__", it is marked as a NOT condition.
/// - The expected format is:
///   - `field=value` (interpreted as equality, AND operator by default)
///   - `field__operator=value` (AND operator by default)
///   - `or__field=value` or `or__field__operator=value` for OR conditions.
///   - `not__field=value` or `not__field__operator=value` for NOT conditions.
pub fn parse_filters(query: &HashMap<String, String>) -> Vec<FilterCondition> {
    query
        .iter()
        .filter_map(|(key, value)| {
            // Detect whether the condition is OR or NOT
            let (logic, key_clean) = if key.starts_with("or__") {
                (LogicalOp::Or, key.trim_start_matches("or__"))
            } else if key.starts_with("not__") {
                (LogicalOp::Not, key.trim_start_matches("not__"))
            } else {
                (LogicalOp::And, key.as_str())
            };

            // Split by "__" to identify field and operator
            let parts: Vec<&str> = key_clean.split("__").collect();
            let (field, operator) = match parts.len() {
                1 => (parts[0].to_string(), "=".to_string()),
                2 => {
                    let op = match parts[1] {
                        "gte" => ">=",
                        "lte" => "<=",
                        "gt" => ">",
                        "lt" => "<",
                        "ne" => "<>",
                        "eq" => "=",
                        "like" => "LIKE",
                        "ilike" => "ILIKE",
                        "in" => "IN",
                        _ => return None, // Unsupported operator
                    };
                    (parts[0].to_string(), op.to_string())
                }
                _ => return None, // Malformed parameter
            };

            Some(FilterCondition {
                field,
                operator,
                value: value.to_string(),
                logic,
            })
        })
        .collect()
}

/// Builds the WHERE clause from parsed filters,
/// grouping separately AND, OR, and NOT conditions.
///
/// `start_param` indicates the starting index for query parameters
/// (e.g., if you already have 8 fixed parameters, start at 9).
///
/// Returns a tuple with:
/// - The string representing the WHERE clause (or part of it).
/// - A vector with the values to bind to the query.
pub fn build_where_clause(
    filters: &[FilterCondition],
    start_param: usize,
) -> (String, Vec<String>) {
    let mut and_conditions = Vec::new();
    let mut or_conditions = Vec::new();
    let mut not_conditions = Vec::new();
    let mut bindings = Vec::new();
    let mut param_index = start_param;

    // Iterate over each filter and group them according to their logic (AND, OR, NOT)
    for filter in filters {
        let condition = match filter.operator.as_str() {
            "LIKE" => format!("{} {} ${}", filter.field, filter.operator, param_index),
            "ILIKE" => format!("{} ILIKE ${}", filter.field, param_index),
            "IN" => {
                let array_values = filter.value.split(',').collect::<Vec<_>>().join(",");
                format!("{} = ANY(ARRAY[{}])", filter.field, array_values)
            }
            _ => format!("{} {} ${}", filter.field, filter.operator, param_index),
        };
        match filter.logic {
            LogicalOp::And => and_conditions.push(condition),
            LogicalOp::Or => or_conditions.push(condition),
            LogicalOp::Not => not_conditions.push(condition),
        }
        bindings.push(filter.value.clone());
        param_index += 1;
    }

    let mut clause = String::new();

    // Add AND conditions
    if !and_conditions.is_empty() {
        clause.push_str(&and_conditions.join(" AND "));
    }

    // Add OR conditions, wrapped in parentheses
    if !or_conditions.is_empty() {
        let or_clause = or_conditions.join(" OR ");
        if !clause.is_empty() {
            clause.push_str(" AND (");
            clause.push_str(&or_clause);
            clause.push(')');
        } else {
            clause.push('(');
            clause.push_str(&or_clause);
            clause.push(')');
        }
    }

    // Add NOT conditions, wrapped in parentheses and prefixed with NOT
    if !not_conditions.is_empty() {
        for not_condition in not_conditions {
            if !clause.is_empty() {
                clause.push_str(" AND ");
            }
            clause.push_str("NOT (");
            clause.push_str(&not_condition);
            clause.push(')');
        }
    }

    if clause.is_empty() {
        (String::new(), bindings)
    } else {
        (format!(" {clause}"), bindings)
    }
}

// fn validate_and_escape_values(values: &[String]) -> Vec<String> {
//     values
//         .iter()
//         .map(|value| {
//             value.replace('\'', "''")
//         })
//         .collect()
// }

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Test for the filter parser, verifying that it correctly detects
    // AND, OR, and NOT conditions based on their prefixes.
    #[test]
    fn test_parse_filters_mixed() {
        let mut query = HashMap::new();
        query.insert("date__gte".to_string(), "2017-01-01".to_string());
        query.insert("date__lte".to_string(), "2017-04-05".to_string());
        query.insert("or__hour__lt".to_string(), "18".to_string());
        query.insert("not__status".to_string(), "inactive".to_string());
        query.insert("status".to_string(), "active".to_string());

        let filters = parse_filters(&query);
        let and_filters: Vec<&FilterCondition> = filters
            .iter()
            .filter(|f| matches!(f.logic, LogicalOp::And))
            .collect();
        let or_filters: Vec<&FilterCondition> = filters
            .iter()
            .filter(|f| matches!(f.logic, LogicalOp::Or))
            .collect();
        let not_filters: Vec<&FilterCondition> = filters
            .iter()
            .filter(|f| matches!(f.logic, LogicalOp::Not))
            .collect();

        assert_eq!(and_filters.len(), 3);
        assert_eq!(or_filters.len(), 1);
        assert_eq!(not_filters.len(), 1);

        let mut and_fields: Vec<String> = and_filters.iter().map(|f| f.field.clone()).collect();
        and_fields.sort();
        assert_eq!(
            and_fields,
            vec!["date".to_string(), "date".to_string(), "status".to_string()]
        );
        assert_eq!(or_filters[0].field, "hour");
        assert_eq!(not_filters[0].field, "status");
    }

    // Test build_where_clause with only AND conditions.
    #[test]
    fn test_build_where_clause_only_and() {
        let filters = vec![
            FilterCondition {
                field: "date".to_string(),
                operator: ">=".to_string(),
                value: "2017-01-01".to_string(),
                logic: LogicalOp::And,
            },
            FilterCondition {
                field: "date".to_string(),
                operator: "<=".to_string(),
                value: "2017-04-05".to_string(),
                logic: LogicalOp::And,
            },
        ];
        let (clause, bindings) = build_where_clause(&filters, 1);
        assert_eq!(clause, " date >= $1 AND date <= $2");
        assert_eq!(
            bindings,
            vec!["2017-01-01".to_string(), "2017-04-05".to_string()]
        );
    }

    // Test build_where_clause combining AND, OR, and NOT conditions.
    #[test]
    fn test_build_where_clause_and_or_not() {
        let filters = vec![
            FilterCondition {
                field: "date".to_string(),
                operator: ">=".to_string(),
                value: "2017-01-01".to_string(),
                logic: LogicalOp::And,
            },
            FilterCondition {
                field: "status".to_string(),
                operator: "=".to_string(),
                value: "active".to_string(),
                logic: LogicalOp::And,
            },
            FilterCondition {
                field: "hour".to_string(),
                operator: "<".to_string(),
                value: "18".to_string(),
                logic: LogicalOp::Or,
            },
            FilterCondition {
                field: "status".to_string(),
                operator: "=".to_string(),
                value: "inactive".to_string(),
                logic: LogicalOp::Not,
            },
        ];
        let (clause, bindings) = build_where_clause(&filters, 1);
        assert_eq!(
            clause,
            " date >= $1 AND status = $2 AND (hour < $3) AND NOT (status = $4)"
        );
        assert_eq!(
            bindings,
            vec![
                "2017-01-01".to_string(),
                "active".to_string(),
                "18".to_string(),
                "inactive".to_string()
            ]
        );
    }

    // Test build_where_clause with only OR conditions.
    #[test]
    fn test_build_where_clause_only_or() {
        let filters = vec![
            FilterCondition {
                field: "hour".to_string(),
                operator: "<".to_string(),
                value: "18".to_string(),
                logic: LogicalOp::Or,
            },
            FilterCondition {
                field: "minute".to_string(),
                operator: ">".to_string(),
                value: "30".to_string(),
                logic: LogicalOp::Or,
            },
        ];
        let (clause, bindings) = build_where_clause(&filters, 1);
        assert_eq!(clause, " (hour < $1 OR minute > $2)");
        assert_eq!(bindings, vec!["18".to_string(), "30".to_string()]);
    }

    #[test]
    fn test_build_where_clause_only_not() {
        let filters = vec![
            FilterCondition {
                field: "status".to_string(),
                operator: "=".to_string(),
                value: "inactive".to_string(),
                logic: LogicalOp::Not,
            },
            FilterCondition {
                field: "hour".to_string(),
                operator: ">".to_string(),
                value: "18".to_string(),
                logic: LogicalOp::Not,
            },
        ];
        let (clause, bindings) = build_where_clause(&filters, 1);
        assert_eq!(clause, " NOT (status = $1) AND NOT (hour > $2)");
        assert_eq!(bindings, vec!["inactive".to_string(), "18".to_string()]);
    }

    // Test parsing filters that include string and float values.
    #[test]
    fn test_parse_filters_string_and_float() {
        let mut query = HashMap::new();
        query.insert("name".to_string(), "Alice".to_string()); // string, equality
        query.insert("score__gt".to_string(), "4.2".to_string()); // float, >
        query.insert("or__description__ne".to_string(), "poor".to_string()); // string, <>, OR condition

        let filters = parse_filters(&query);
        let and_filters: Vec<&FilterCondition> = filters
            .iter()
            .filter(|f| matches!(f.logic, LogicalOp::And))
            .collect();
        let or_filters: Vec<&FilterCondition> = filters
            .iter()
            .filter(|f| matches!(f.logic, LogicalOp::Or))
            .collect();

        assert_eq!(and_filters.len(), 2);
        assert_eq!(or_filters.len(), 1);

        assert!(
            and_filters
                .iter()
                .any(|f| f.field == "name" && f.operator == "=" && f.value == "Alice")
        );
        assert!(
            and_filters
                .iter()
                .any(|f| f.field == "score" && f.operator == ">" && f.value == "4.2")
        );

        assert_eq!(or_filters[0].field, "description");
        assert_eq!(or_filters[0].operator, "<>");
        assert_eq!(or_filters[0].value, "poor");
    }

    // Test building WHERE clause with string and float conditions.
    #[test]
    fn test_build_where_clause_string_and_float() {
        let filters = vec![
            FilterCondition {
                field: "name".to_string(),
                operator: "=".to_string(),
                value: "John".to_string(),
                logic: LogicalOp::And,
            },
            FilterCondition {
                field: "score".to_string(),
                operator: ">".to_string(),
                value: "3.5".to_string(),
                logic: LogicalOp::And,
            },
            FilterCondition {
                field: "description".to_string(),
                operator: "<>".to_string(),
                value: "bad".to_string(),
                logic: LogicalOp::Or,
            },
        ];

        let (clause, bindings) = build_where_clause(&filters, 1);
        assert_eq!(clause, " name = $1 AND score > $2 AND (description <> $3)");
        assert_eq!(
            bindings,
            vec!["John".to_string(), "3.5".to_string(), "bad".to_string()]
        );
    }

    // Test parsing filters that include LIKE and IN conditions.
    #[test]
    fn test_parse_filters_like_and_in() {
        let mut query = HashMap::new();
        query.insert("name__like".to_string(), "%Alice%".to_string()); // string, LIKE
        query.insert("id__in".to_string(), "1,2,3".to_string()); // IN

        let filters = parse_filters(&query);
        let and_filters: Vec<&FilterCondition> = filters
            .iter()
            .filter(|f| matches!(f.logic, LogicalOp::And))
            .collect();

        assert_eq!(and_filters.len(), 2);

        assert!(
            and_filters
                .iter()
                .any(|f| f.field == "name" && f.operator == "LIKE" && f.value == "%Alice%")
        );
        assert!(
            and_filters
                .iter()
                .any(|f| f.field == "id" && f.operator == "IN" && f.value == "1,2,3")
        );
    }

    #[test]
    fn test_build_where_clause_with_in_operator() {
        let filters = vec![
            FilterCondition {
                field: "id".to_string(),
                operator: "IN".to_string(),
                value: "6,9,22".to_string(),
                logic: LogicalOp::Or,
            },
            FilterCondition {
                field: "name".to_string(),
                operator: "=".to_string(),
                value: "Foo".to_string(),
                logic: LogicalOp::Or,
            },
        ];

        let (clause, bindings) = build_where_clause(&filters, 1);

        assert!(clause.contains("id = ANY(ARRAY[6,9,22])"));
        assert!(clause.contains("name = $"));
        assert!(clause.starts_with(" ("));
        assert!(clause.ends_with(")"));
        assert!(bindings.contains(&"Foo".to_string()));
        assert!(bindings.contains(&"6,9,22".to_string()));
    }

    // Test building WHERE clause with LIKE and IN conditions.
    #[test]
    fn test_build_where_clause_like_and_in() {
        let filters = vec![
            FilterCondition {
                field: "name".to_string(),
                operator: "LIKE".to_string(),
                value: "%John%".to_string(),
                logic: LogicalOp::And,
            },
            FilterCondition {
                field: "id".to_string(),
                operator: "IN".to_string(),
                value: "1,2,3".to_string(),
                logic: LogicalOp::And,
            },
        ];

        let (clause, bindings) = build_where_clause(&filters, 1);
        assert_eq!(clause, " name LIKE $1 AND id = ANY(ARRAY[1,2,3])");
        assert_eq!(bindings, vec!["%John%".to_string(), "1,2,3".to_string()]);
    }

    // Test building WHERE clause with ILIKE and IN conditions.
    #[test]
    fn test_build_where_clause_ilike_and_in() {
        let filters = vec![
            FilterCondition {
                field: "name".to_string(),
                operator: "ILIKE".to_string(),
                value: "%John%".to_string(),
                logic: LogicalOp::And,
            },
            FilterCondition {
                field: "id".to_string(),
                operator: "IN".to_string(),
                value: "1,2,3".to_string(),
                logic: LogicalOp::And,
            },
        ];

        let (clause, bindings) = build_where_clause(&filters, 1);
        assert_eq!(clause, " name ILIKE $1 AND id = ANY(ARRAY[1,2,3])");
        assert_eq!(bindings, vec!["%John%".to_string(), "1,2,3".to_string()]);
    }

    // Test building a complex WHERE clause with multiple AND, OR, and NOT conditions.
    #[test]
    fn test_build_where_clause_complex() {
        let filters = vec![
            FilterCondition {
                field: "name".to_string(),
                operator: "=".to_string(),
                value: "John".to_string(),
                logic: LogicalOp::And,
            },
            FilterCondition {
                field: "age".to_string(),
                operator: ">".to_string(),
                value: "25".to_string(),
                logic: LogicalOp::And,
            },
            FilterCondition {
                field: "status".to_string(),
                operator: "=".to_string(),
                value: "active".to_string(),
                logic: LogicalOp::Or,
            },
            FilterCondition {
                field: "score".to_string(),
                operator: "<".to_string(),
                value: "100".to_string(),
                logic: LogicalOp::Or,
            },
            FilterCondition {
                field: "status".to_string(),
                operator: "=".to_string(),
                value: "inactive".to_string(),
                logic: LogicalOp::Not,
            },
        ];

        let (clause, bindings) = build_where_clause(&filters, 1);
        assert_eq!(
            clause,
            " name = $1 AND age > $2 AND (status = $3 OR score < $4) AND NOT (status = $5)"
        );
        assert_eq!(
            bindings,
            vec![
                "John".to_string(),
                "25".to_string(),
                "active".to_string(),
                "100".to_string(),
                "inactive".to_string()
            ]
        );
    }

    #[test]
    fn test_build_where_clause_or_conditions() {
        let mut query = HashMap::new();
        query.insert("or__vur_dolar__gte".to_string(), "600".to_string());
        query.insert("or__vur_pesos__gte".to_string(), "700000".to_string());

        let filters = parse_filters(&query);

        assert_eq!(filters.len(), 2);

        for filter in &filters {
            assert!(matches!(filter.logic, LogicalOp::Or));
        }

        let (clause, bindings) = build_where_clause(&filters, 1);

        assert!(clause.contains("vur_dolar >= $"));
        assert!(clause.contains("vur_pesos >= $"));

        assert!(bindings.contains(&"600".to_string()));
        assert!(bindings.contains(&"700000".to_string()));
    }

    #[test]
    fn test_build_where_clause_not_conditions() {
        let filters = vec![
            FilterCondition {
                field: "status".to_string(),
                operator: "=".to_string(),
                value: "inactive".to_string(),
                logic: LogicalOp::Not,
            },
            FilterCondition {
                field: "hour".to_string(),
                operator: ">".to_string(),
                value: "18".to_string(),
                logic: LogicalOp::Not,
            },
        ];
        let (clause, bindings) = build_where_clause(&filters, 1);
        assert_eq!(clause, " NOT (status = $1) AND NOT (hour > $2)");
        assert_eq!(bindings, vec!["inactive".to_string(), "18".to_string()]);
    }
}
