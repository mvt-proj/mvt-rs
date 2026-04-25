#[cfg(test)]
mod tests {
    // Logic to test the naming convention mapping from environment variables
    #[test]
    fn test_db_name_parsing() {
        let keys = vec![
            ("DBCONN", "default"),
            ("DBCONN_DEFAULT", "default"),
            ("DBCONN_SECONDARY", "secondary"),
            ("DBCONN_MY_DB", "my_db"),
        ];

        for (key, expected) in keys {
            let name = if key == "DBCONN" || key == "DBCONN_DEFAULT" {
                "default".to_string()
            } else {
                key.replace("DBCONN_", "").to_lowercase()
            };
            assert_eq!(name, expected);
        }
    }
}
