#[cfg(test)]
mod tests {
    use crate::services::utils::{validate_filter, normalize_name};

    #[test]
    fn test_validate_filter_empty() {
        assert!(validate_filter("").is_ok());
        assert!(validate_filter("   ").is_ok());
    }

    #[test]
    fn test_validate_filter_valid_simple() {
        assert!(validate_filter("status = 'active'").is_ok());
        assert!(validate_filter("price > 100").is_ok());
        assert!(validate_filter("name LIKE 'John%'").is_ok());
    }

    #[test]
    fn test_validate_filter_valid_complex() {
        assert!(validate_filter("status = 'active' AND price > 100").is_ok());
        assert!(
            validate_filter("(status = 'active' OR status = 'pending') AND price > 100").is_ok()
        );
    }

    #[test]
    fn test_validate_filter_dangerous_keywords() {
        assert!(validate_filter("status = 'active'; DELETE FROM users").is_err());
        assert!(validate_filter("price > 100 DROP TABLE users").is_err());
        assert!(validate_filter("UNION SELECT * FROM users").is_err());
    }

    #[test]
    fn test_validate_filter_keywords_in_quotes_allowed() {
        assert!(validate_filter("name = 'DROP TABLE'").is_ok());
        assert!(validate_filter("description = 'This contains UNION and SELECT'").is_ok());
    }

    #[test]
    fn test_validate_filter_concat_injection() {
        assert!(validate_filter("name = '; DROP TABLE users'").is_ok());
        assert!(validate_filter("' OR 1=1; DROP TABLE users; --").is_err());
    }

    #[test]
    fn test_validate_filter_sql_injection_patterns() {
        assert!(validate_filter("1=1").is_err());
        assert!(validate_filter("1 = 1").is_err());
        assert!(validate_filter("OR 1=1").is_err());
        assert!(validate_filter("500=500").is_err());
    }

    #[test]
    fn test_validate_filter_tautology_variations() {
        assert!(validate_filter("OR 'x'='x'").is_err());
        assert!(validate_filter("AND 'admin'='admin'").is_err());
    }

    #[test]
    fn test_validate_filter_hex_literals() {
        assert!(validate_filter("name = 0x61646D696E").is_err());
    }

    #[test]
    fn test_validate_filter_system_procedures() {
        assert!(validate_filter("sp_executesql @query").is_err());
        assert!(validate_filter("EXEC xp_cmdshell 'dir'").is_err());
    }

    #[test]
    fn test_validate_filter_comment_injection() {
        assert!(validate_filter("admin' --").is_err());
        assert!(validate_filter("/* comment */").is_err());
    }

    #[test]
    fn test_validate_filter_unbalanced_quotes() {
        assert!(validate_filter("name = 'missing quote").is_err());
        assert!(validate_filter("name = \"missing quote").is_err());
    }

    #[test]
    fn test_normalize_name_spaces_and_case() {
        assert_eq!(
            normalize_name("departamentos Capital").unwrap(),
            "departamentos_capital"
        );
        assert_eq!(normalize_name("GRUPOS").unwrap(), "grupos");
    }

    #[test]
    fn test_normalize_name_accents() {
        assert_eq!(normalize_name("Categoría Ríos").unwrap(), "categoria_rios");
        assert_eq!(normalize_name("Ñandú Güemes").unwrap(), "nandu_guemes");
    }

    #[test]
    fn test_normalize_name_collapses_separators() {
        assert_eq!(normalize_name("  foo   bar  ").unwrap(), "foo_bar");
        assert_eq!(normalize_name("foo__bar").unwrap(), "foo_bar");
        assert_eq!(normalize_name("foo \t bar").unwrap(), "foo_bar");
    }

    #[test]
    fn test_normalize_name_drops_symbols() {
        assert_eq!(normalize_name("depto. (norte)").unwrap(), "depto_norte");
        assert_eq!(normalize_name("capa-2024!").unwrap(), "capa2024");
    }

    #[test]
    fn test_normalize_name_no_leading_or_trailing_underscore() {
        assert_eq!(normalize_name("!foo bar!").unwrap(), "foo_bar");
        assert_eq!(normalize_name(" _foo_ ").unwrap(), "foo");
    }

    #[test]
    fn test_normalize_name_already_normalized_passthrough() {
        assert_eq!(normalize_name("departamentos_capital").unwrap(), "departamentos_capital");
    }

    #[test]
    fn test_normalize_name_empty_result_is_error() {
        assert!(normalize_name("").is_err());
        assert!(normalize_name("   ").is_err());
        assert!(normalize_name("!!!").is_err());
        assert!(normalize_name("___").is_err());
    }
}
