#[cfg(test)]
mod tests {
    use crate::services::utils::validate_filter;

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
        assert!(validate_filter("(status = 'active' OR status = 'pending') AND price > 100").is_ok());
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
}
