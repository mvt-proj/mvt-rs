use super::builder::SqlQueryBuilder;
use super::parser::parse_query_params;
use super::types::{FilterCondition, LogicalOp, Operator};
use std::collections::HashMap;

fn build_where_clause(filters: &[FilterCondition], start: usize) -> (String, Vec<String>) {
    let mut builder = SqlQueryBuilder::new(start);
    builder.build(filters)
}

#[test]
fn test_parse_filters_mixed() {
    let mut query = HashMap::new();
    query.insert("date__gte".to_string(), "2017-01-01".to_string());
    query.insert("date__lte".to_string(), "2017-04-05".to_string());
    query.insert("or__hour__lt".to_string(), "18".to_string());
    query.insert("not__status".to_string(), "inactive".to_string());
    query.insert("status".to_string(), "active".to_string());

    let filters = parse_query_params(&query);

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

#[test]
fn test_build_where_clause_only_and() {
    let filters = vec![
        FilterCondition {
            field: "date".to_string(),
            operator: Operator::Gte,
            value: "2017-01-01".to_string(),
            logic: LogicalOp::And,
        },
        FilterCondition {
            field: "date".to_string(),
            operator: Operator::Lte,
            value: "2017-04-05".to_string(),
            logic: LogicalOp::And,
        },
    ];
    let (clause, bindings) = build_where_clause(&filters, 1);
    assert_eq!(clause, "date >= $1 AND date <= $2");
    assert_eq!(
        bindings,
        vec!["2017-01-01".to_string(), "2017-04-05".to_string()]
    );
}

#[test]
fn test_build_where_clause_and_or_not() {
    let filters = vec![
        FilterCondition {
            field: "date".to_string(),
            operator: Operator::Gte,
            value: "2017-01-01".to_string(),
            logic: LogicalOp::And,
        },
        FilterCondition {
            field: "status".to_string(),
            operator: Operator::Eq,
            value: "active".to_string(),
            logic: LogicalOp::And,
        },
        FilterCondition {
            field: "hour".to_string(),
            operator: Operator::Lt,
            value: "18".to_string(),
            logic: LogicalOp::Or,
        },
        FilterCondition {
            field: "status".to_string(),
            operator: Operator::Eq,
            value: "inactive".to_string(),
            logic: LogicalOp::Not,
        },
    ];
    let (clause, bindings) = build_where_clause(&filters, 1);
    assert_eq!(
        clause,
        "date >= $1 AND status = $2 AND (hour < $3) AND NOT (status = $4)"
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

#[test]
fn test_build_where_clause_only_or() {
    let filters = vec![
        FilterCondition {
            field: "hour".to_string(),
            operator: Operator::Lt,
            value: "18".to_string(),
            logic: LogicalOp::Or,
        },
        FilterCondition {
            field: "minute".to_string(),
            operator: Operator::Gt,
            value: "30".to_string(),
            logic: LogicalOp::Or,
        },
    ];
    let (clause, bindings) = build_where_clause(&filters, 1);
    assert_eq!(clause, "(hour < $1 OR minute > $2)");
    assert_eq!(bindings, vec!["18".to_string(), "30".to_string()]);
}

#[test]
fn test_build_where_clause_only_not() {
    let filters = vec![
        FilterCondition {
            field: "status".to_string(),
            operator: Operator::Eq,
            value: "inactive".to_string(),
            logic: LogicalOp::Not,
        },
        FilterCondition {
            field: "hour".to_string(),
            operator: Operator::Gt,
            value: "18".to_string(),
            logic: LogicalOp::Not,
        },
    ];
    let (clause, bindings) = build_where_clause(&filters, 1);
    assert_eq!(clause, "NOT (status = $1) AND NOT (hour > $2)");
    assert_eq!(bindings, vec!["inactive".to_string(), "18".to_string()]);
}

#[test]
fn test_parse_filters_string_and_float() {
    let mut query = HashMap::new();
    query.insert("name".to_string(), "Alice".to_string());
    query.insert("score__gt".to_string(), "4.2".to_string());
    query.insert("or__description__ne".to_string(), "poor".to_string());

    let filters = parse_query_params(&query);
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
            .any(|f| f.field == "name" && f.operator == Operator::Eq && f.value == "Alice")
    );
    assert!(
        and_filters
            .iter()
            .any(|f| f.field == "score" && f.operator == Operator::Gt && f.value == "4.2")
    );

    assert_eq!(or_filters[0].field, "description");
    assert_eq!(or_filters[0].operator, Operator::Ne);
    assert_eq!(or_filters[0].value, "poor");
}

#[test]
fn test_build_where_clause_string_and_float() {
    let filters = vec![
        FilterCondition {
            field: "name".to_string(),
            operator: Operator::Eq,
            value: "John".to_string(),
            logic: LogicalOp::And,
        },
        FilterCondition {
            field: "score".to_string(),
            operator: Operator::Gt,
            value: "3.5".to_string(),
            logic: LogicalOp::And,
        },
        FilterCondition {
            field: "description".to_string(),
            operator: Operator::Ne,
            value: "bad".to_string(),
            logic: LogicalOp::Or,
        },
    ];

    let (clause, bindings) = build_where_clause(&filters, 1);
    assert_eq!(clause, "name = $1 AND score > $2 AND (description <> $3)");
    assert_eq!(
        bindings,
        vec!["John".to_string(), "3.5".to_string(), "bad".to_string()]
    );
}

#[test]
fn test_parse_filters_like_and_in() {
    let mut query = HashMap::new();
    query.insert("name__like".to_string(), "%Alice%".to_string());
    query.insert("id__in".to_string(), "1,2,3".to_string());

    let filters = parse_query_params(&query);
    let and_filters: Vec<&FilterCondition> = filters
        .iter()
        .filter(|f| matches!(f.logic, LogicalOp::And))
        .collect();

    assert_eq!(and_filters.len(), 2);

    assert!(
        and_filters
            .iter()
            .any(|f| f.field == "name" && f.operator == Operator::Like && f.value == "%Alice%")
    );
    assert!(
        and_filters
            .iter()
            .any(|f| f.field == "id" && f.operator == Operator::In && f.value == "1,2,3")
    );
}

#[test]
fn test_build_where_clause_with_in_operator() {
    let filters = vec![
        FilterCondition {
            field: "id".to_string(),
            operator: Operator::In,
            value: "6,9,22".to_string(),
            logic: LogicalOp::Or,
        },
        FilterCondition {
            field: "name".to_string(),
            operator: Operator::Eq,
            value: "Foo".to_string(),
            logic: LogicalOp::Or,
        },
    ];

    let (clause, bindings) = build_where_clause(&filters, 1);

    assert!(clause.contains("id IN ($1::int, $2::int, $3::int)"));
    assert!(clause.contains("name = $4"));

    assert_eq!(bindings[0], "6");
    assert_eq!(bindings[1], "9");
    assert_eq!(bindings[2], "22");
    assert_eq!(bindings[3], "Foo");
}

#[test]
fn test_build_where_clause_like_and_in() {
    let filters = vec![
        FilterCondition {
            field: "name".to_string(),
            operator: Operator::Like,
            value: "%John%".to_string(),
            logic: LogicalOp::And,
        },
        FilterCondition {
            field: "id".to_string(),
            operator: Operator::In,
            value: "1,2,3".to_string(),
            logic: LogicalOp::And,
        },
    ];

    let (clause, bindings) = build_where_clause(&filters, 1);

    assert_eq!(clause, "name LIKE $1 AND id IN ($2::int, $3::int, $4::int)");

    assert_eq!(
        bindings,
        vec![
            "%John%".to_string(),
            "1".to_string(),
            "2".to_string(),
            "3".to_string()
        ]
    );
}

#[test]
fn test_build_where_clause_ilike_and_in() {
    let filters = vec![
        FilterCondition {
            field: "name".to_string(),
            operator: Operator::Ilike,
            value: "%John%".to_string(),
            logic: LogicalOp::And,
        },
        FilterCondition {
            field: "id".to_string(),
            operator: Operator::In,
            value: "1,2,3".to_string(),
            logic: LogicalOp::And,
        },
    ];

    let (clause, bindings) = build_where_clause(&filters, 1);

    assert_eq!(
        clause,
        "name ILIKE $1 AND id IN ($2::int, $3::int, $4::int)"
    );
    assert_eq!(
        bindings,
        vec![
            "%John%".to_string(),
            "1".to_string(),
            "2".to_string(),
            "3".to_string()
        ]
    );
}

#[test]
fn test_build_where_clause_complex() {
    let filters = vec![
        FilterCondition {
            field: "name".to_string(),
            operator: Operator::Eq,
            value: "John".to_string(),
            logic: LogicalOp::And,
        },
        FilterCondition {
            field: "age".to_string(),
            operator: Operator::Gt,
            value: "25".to_string(),
            logic: LogicalOp::And,
        },
        FilterCondition {
            field: "status".to_string(),
            operator: Operator::Eq,
            value: "active".to_string(),
            logic: LogicalOp::Or,
        },
        FilterCondition {
            field: "score".to_string(),
            operator: Operator::Lt,
            value: "100".to_string(),
            logic: LogicalOp::Or,
        },
        FilterCondition {
            field: "status".to_string(),
            operator: Operator::Eq,
            value: "inactive".to_string(),
            logic: LogicalOp::Not,
        },
    ];

    let (clause, bindings) = build_where_clause(&filters, 1);
    assert_eq!(
        clause,
        "name = $1 AND age > $2 AND (status = $3 OR score < $4) AND NOT (status = $5)"
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

    let filters = parse_query_params(&query);

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

// SQL injection validation tests
#[cfg(test)]
mod sql_injection_tests {
    use crate::services::utils::validate_filter;

    #[test]
    fn valid_filter_passes() {
        assert!(validate_filter("name = 'Buenos Aires'").is_ok());
        assert!(validate_filter("population > 1000000").is_ok());
        assert!(validate_filter("").is_ok());
        assert!(validate_filter("active = true AND region = 'Norte'").is_ok());
    }

    #[test]
    fn sql_injection_tautology_rejected() {
        assert!(validate_filter("1=1").is_err());
    }

    #[test]
    fn sql_injection_comment_rejected() {
        assert!(validate_filter("name = 'x' -- DROP TABLE").is_err());
        assert!(validate_filter("name = 'x' /* comment */").is_err());
    }

    #[test]
    fn sql_injection_dangerous_keywords_rejected() {
        assert!(validate_filter("name = 'x' DROP TABLE users").is_err());
        assert!(validate_filter("UNION SELECT * FROM passwords").is_err());
    }

    #[test]
    fn sql_injection_hex_literal_rejected() {
        assert!(validate_filter("id = 0x41414141").is_err());
    }

    #[test]
    fn unbalanced_quotes_rejected() {
        assert!(validate_filter("name = 'unclosed").is_err());
    }
}
