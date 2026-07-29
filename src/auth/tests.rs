#[cfg(test)]
mod tests {
    use super::super::models::{
        Auth, Group, JwtClaims, User, count_group_references, resolve_updated_password,
    };
    use super::super::utils::decode_basic_auth;
    use base64::{Engine as _, engine::general_purpose};
    use crate::models::catalog::Layer;
    use crate::models::category::Category;

    fn create_test_auth() -> Auth {
        Auth {
            groups: vec![
                Group {
                    id: "1".to_string(),
                    name: "admin".to_string(),
                    description: "Admin group".to_string(),
                },
                Group {
                    id: "2".to_string(),
                    name: "users".to_string(),
                    description: "Regular users".to_string(),
                },
            ],
            users: vec![],
            config_dir: "/tmp/test".to_string(),
        }
    }

    fn create_test_user(
        auth: &Auth,
        username: &str,
        email: &str,
        password: &str,
        groups: Vec<Group>,
    ) -> User {
        let encrypted_password = auth.get_encrypt_psw(password.to_string()).unwrap();
        User {
            id: uuid::Uuid::new_v4().to_string(),
            username: username.to_string(),
            email: email.to_string(),
            first_name: Some("Test".to_string()),
            last_name: Some("User".to_string()),
            password: encrypted_password,
            groups,
        }
    }

    #[test]
    fn test_resolve_updated_password_none_keeps_existing_hash() {
        let auth = create_test_auth();
        let result = resolve_updated_password(&auth, "existing-hash", None).unwrap();
        assert_eq!(result, "existing-hash");
    }

    #[test]
    fn test_resolve_updated_password_some_hashes_the_new_password() {
        let auth = create_test_auth();
        let result =
            resolve_updated_password(&auth, "existing-hash", Some("new-password".to_string()))
                .unwrap();
        assert_ne!(result, "existing-hash");
        assert_ne!(result, "new-password"); // stored as an argon2 hash, not plaintext
    }

    fn test_layer(id: &str, groups: Option<Vec<Group>>) -> Layer {
        Layer {
            id: id.to_string(),
            category: Category {
                id: "cat-1".to_string(),
                name: "public".to_string(),
                description: String::new(),
            },
            geometry: "polygons".to_string(),
            name: "layer".to_string(),
            alias: "Layer".to_string(),
            description: String::new(),
            database_id: "default".to_string(),
            schema: "public".to_string(),
            table_name: "t".to_string(),
            fields: vec![],
            filter: None,
            srid: None,
            geom: None,
            sql_mode: None,
            buffer: None,
            extent: None,
            zmin: None,
            zmax: None,
            zmax_do_not_simplify: None,
            buffer_do_not_simplify: None,
            extent_do_not_simplify: None,
            clip_geom: None,
            delete_cache_on_start: None,
            max_cache_age: None,
            max_records: None,
            published: true,
            url: None,
            groups,
        }
    }

    #[test]
    fn test_count_group_references_none() {
        let auth = create_test_auth();
        let other_group = Group {
            id: "other".to_string(),
            name: "other".to_string(),
            description: String::new(),
        };
        let users = vec![create_test_user(
            &auth,
            "u1",
            "u1@test.com",
            "pw",
            vec![other_group.clone()],
        )];
        let layers = vec![test_layer("l1", Some(vec![other_group]))];
        assert_eq!(count_group_references("target", &users, &layers), (0, 0));
    }

    #[test]
    fn test_count_group_references_counts_users_and_layers() {
        let auth = create_test_auth();
        let target_group = Group {
            id: "target".to_string(),
            name: "target".to_string(),
            description: String::new(),
        };
        let other_group = Group {
            id: "other".to_string(),
            name: "other".to_string(),
            description: String::new(),
        };
        let users = vec![
            create_test_user(&auth, "u1", "u1@test.com", "pw", vec![target_group.clone()]),
            create_test_user(
                &auth,
                "u2",
                "u2@test.com",
                "pw",
                vec![target_group.clone(), other_group.clone()],
            ),
            create_test_user(&auth, "u3", "u3@test.com", "pw", vec![other_group]),
        ];
        let layers = vec![
            test_layer("l1", Some(vec![target_group])),
            test_layer("l2", None),
        ];
        assert_eq!(count_group_references("target", &users, &layers), (2, 1));
    }

    #[test]
    fn test_count_group_references_empty_input() {
        assert_eq!(count_group_references("target", &[], &[]), (0, 0));
    }

    #[test]
    fn test_decode_basic_auth_valid() {
        let username = "testuser";
        let password = "testpass";
        let credentials = format!("{}:{}", username, password);
        let encoded = general_purpose::STANDARD.encode(credentials.as_bytes());
        let auth_header = format!("Basic {}", encoded);

        let result = decode_basic_auth(&auth_header);
        assert!(result.is_ok());
        let (decoded_username, decoded_password) = result.unwrap();
        assert_eq!(decoded_username, username);
        assert_eq!(decoded_password, password);
    }

    #[test]
    fn test_decode_basic_auth_invalid_format() {
        let result = decode_basic_auth("InvalidFormat");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_basic_auth_invalid_base64() {
        let result = decode_basic_auth("Basic !!!invalid!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_basic_auth_missing_colon() {
        let encoded = general_purpose::STANDARD.encode("usernameonly".as_bytes());
        let auth_header = format!("Basic {}", encoded);
        let result = decode_basic_auth(&auth_header);
        assert!(result.is_err());
    }

    #[test]
    fn test_user_is_admin_true() {
        let admin_group = Group {
            id: "1".to_string(),
            name: "admin".to_string(),
            description: "Admin group".to_string(),
        };

        let user = User {
            id: "1".to_string(),
            username: "admin".to_string(),
            email: "admin@test.com".to_string(),
            first_name: None,
            last_name: None,
            password: "hashedpassword".to_string(),
            groups: vec![admin_group],
        };

        assert!(user.is_admin());
    }

    #[test]
    fn test_user_is_admin_false() {
        let user_group = Group {
            id: "2".to_string(),
            name: "users".to_string(),
            description: "Regular users".to_string(),
        };

        let user = User {
            id: "2".to_string(),
            username: "regularuser".to_string(),
            email: "user@test.com".to_string(),
            first_name: None,
            last_name: None,
            password: "hashedpassword".to_string(),
            groups: vec![user_group],
        };

        assert!(!user.is_admin());
    }

    #[test]
    fn test_user_groups_as_string() {
        let groups = vec![
            Group {
                id: "1".to_string(),
                name: "admin".to_string(),
                description: "Admin group".to_string(),
            },
            Group {
                id: "2".to_string(),
                name: "users".to_string(),
                description: "Users group".to_string(),
            },
        ];

        let user = User {
            id: "1".to_string(),
            username: "test".to_string(),
            email: "test@test.com".to_string(),
            first_name: None,
            last_name: None,
            password: "password".to_string(),
            groups,
        };

        let groups_string = user.groups_as_string();
        assert_eq!(groups_string, "admin | users");
    }

    #[test]
    fn test_user_groups_as_vec_string() {
        let groups = vec![
            Group {
                id: "1".to_string(),
                name: "admin".to_string(),
                description: "Admin group".to_string(),
            },
            Group {
                id: "2".to_string(),
                name: "users".to_string(),
                description: "Users group".to_string(),
            },
        ];

        let user = User {
            id: "1".to_string(),
            username: "test".to_string(),
            email: "test@test.com".to_string(),
            first_name: None,
            last_name: None,
            password: "password".to_string(),
            groups: groups.clone(),
        };

        let groups_vec = user.groups_as_vec_string();
        assert_eq!(groups_vec, vec!["admin", "users"]);
    }

    #[test]
    fn test_auth_find_group_by_name() {
        let auth = create_test_auth();

        let admin_group = auth.find_group_by_name("admin");
        assert!(admin_group.is_some());
        assert_eq!(admin_group.unwrap().name, "admin");

        let nonexistent_group = auth.find_group_by_name("nonexistent");
        assert!(nonexistent_group.is_none());
    }

    #[test]
    fn test_auth_get_encrypt_psw() {
        let auth = create_test_auth();
        let password = "mypassword";

        let encrypted1 = auth.get_encrypt_psw(password.to_string()).unwrap();
        let encrypted2 = auth.get_encrypt_psw(password.to_string()).unwrap();

        assert_ne!(encrypted1, encrypted2);
        assert!(encrypted1.starts_with("$argon2"));
        assert!(encrypted2.starts_with("$argon2"));
    }

    #[test]
    fn test_validate_user_correct_password() {
        let mut auth = create_test_auth();
        let password = "correctpassword";
        let user = create_test_user(&auth, "testuser", "test@test.com", password, vec![]);

        auth.users.push(user);

        let is_valid = auth.validate_user("testuser", password);
        assert!(is_valid);
    }

    #[test]
    fn test_validate_user_incorrect_password() {
        let mut auth = create_test_auth();
        let password = "correctpassword";
        let user = create_test_user(&auth, "testuser", "test@test.com", password, vec![]);

        auth.users.push(user);

        let is_valid = auth.validate_user("testuser", "wrongpassword");
        assert!(!is_valid);
    }

    #[test]
    fn test_validate_user_nonexistent_user() {
        let auth = create_test_auth();

        let is_valid = auth.validate_user("nonexistent", "anypassword");
        assert!(!is_valid);
    }

    #[test]
    fn test_find_user_by_name() {
        let mut auth = create_test_auth();
        let user = create_test_user(&auth, "testuser", "test@test.com", "password", vec![]);

        auth.users.push(user);

        let found = auth.find_user_by_name("testuser");
        assert!(found.is_some());
        assert_eq!(found.unwrap().username, "testuser");

        let not_found = auth.find_user_by_name("nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_get_user_by_id() {
        let mut auth = create_test_auth();
        let user = create_test_user(&auth, "testuser", "test@test.com", "password", vec![]);
        let user_id = user.id.clone();

        auth.users.push(user);

        let found = auth.get_user_by_id(&user_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, user_id);

        let not_found = auth.get_user_by_id("nonexistent-id");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_find_user_position_by_name() {
        let mut auth = create_test_auth();
        let user1 = create_test_user(&auth, "user1", "user1@test.com", "password", vec![]);
        let user2 = create_test_user(&auth, "user2", "user2@test.com", "password", vec![]);

        auth.users.push(user1);
        auth.users.push(user2);

        let pos = auth.find_user_position_by_name("user2");
        assert_eq!(pos, Some(1));

        let no_pos = auth.find_user_position_by_name("nonexistent");
        assert_eq!(no_pos, None);
    }

    #[test]
    fn test_get_user_by_email_and_password_correct() {
        let auth = create_test_auth();
        let password = "correctpassword";
        let user = create_test_user(&auth, "testuser", "test@test.com", password, vec![]);

        let mut auth_with_user = auth.clone();
        auth_with_user.users.push(user);

        let result = auth_with_user.get_user_by_email_and_password("test@test.com", password);
        assert!(result.is_ok());
        let found_user = result.unwrap();
        assert_eq!(found_user.email, "test@test.com");
    }

    #[test]
    fn test_get_user_by_email_and_password_wrong_password() {
        let auth = create_test_auth();
        let password = "correctpassword";
        let user = create_test_user(&auth, "testuser", "test@test.com", password, vec![]);

        let mut auth_with_user = auth.clone();
        auth_with_user.users.push(user);

        let result =
            auth_with_user.get_user_by_email_and_password("test@test.com", "wrongpassword");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_user_by_email_and_password_nonexistent_email() {
        let auth = create_test_auth();

        let result = auth.get_user_by_email_and_password("nonexistent@test.com", "anypassword");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_current_username_and_password_valid() {
        let auth = create_test_auth();
        let username = "testuser";
        let password = "testpass";
        let credentials = format!("{}:{}", username, password);
        let encoded = general_purpose::STANDARD.encode(credentials.as_bytes());
        let auth_header = format!("Basic {}", encoded);

        let result = auth.get_current_username_and_password(&auth_header);
        assert!(result.is_ok());
        let (decoded_username, decoded_password) = result.unwrap();
        assert_eq!(decoded_username, username);
        assert_eq!(decoded_password, password);
    }

    #[test]
    fn test_get_current_username_and_password_invalid() {
        let auth = create_test_auth();

        let result = auth.get_current_username_and_password("Invalid");
        assert!(result.is_ok());
        let (username, password) = result.unwrap();
        assert_eq!(username, "");
        assert_eq!(password, "");
    }

    #[test]
    fn test_password_complexity() {
        let auth = create_test_auth();

        let passwords = vec![
            "simple",
            "with spaces",
            "with-symbols!@#$%",
            "UPPERCASE",
            "MiXeD_CaSe123",
            "emoji-🔒-test",
            "verylongpasswordthatexceedsnormallengthandhaslotsofcharacters",
        ];

        for password in passwords {
            let hash = auth.get_encrypt_psw(password.to_string());
            assert!(hash.is_ok(), "Failed to hash password: {}", password);
        }
    }

    #[test]
    fn test_multiple_users_with_same_password() {
        let mut auth = create_test_auth();
        let password = "sharedpassword";

        let user1 = create_test_user(&auth, "user1", "user1@test.com", password, vec![]);
        let user2 = create_test_user(&auth, "user2", "user2@test.com", password, vec![]);

        assert_ne!(user1.password, user2.password);

        auth.users.push(user1);
        auth.users.push(user2);

        assert!(auth.validate_user("user1", password));
        assert!(auth.validate_user("user2", password));
    }

    #[test]
    fn test_user_password_not_serialized() {
        let user = User {
            id: "1".to_string(),
            username: "test".to_string(),
            email: "test@test.com".to_string(),
            first_name: None,
            last_name: None,
            password: "supersecrethash".to_string(),
            groups: vec![],
        };

        let value = serde_json::to_value(&user).unwrap();
        assert!(value.get("password").is_none());
    }

    #[test]
    fn test_resolve_groups_by_name_matches_known_drops_unknown() {
        let auth = create_test_auth();
        let resolved = auth.resolve_groups_by_name(&[
            "admin".to_string(),
            "ghost".to_string(),
            "users".to_string(),
        ]);
        let mut names: Vec<String> = resolved.iter().map(|g| g.name.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["admin", "users"]);
    }

    #[test]
    fn test_resolve_groups_by_name_empty_input() {
        let auth = create_test_auth();
        assert!(auth.resolve_groups_by_name(&[]).is_empty());
    }

    #[test]
    fn test_jwtclaims_is_admin_true() {
        let claims = JwtClaims {
            id: "1".to_string(),
            username: "admin".to_string(),
            email: "admin@test.com".to_string(),
            groups: vec!["admin".to_string()],
            exp: 0,
        };
        assert!(claims.is_admin());
    }

    #[test]
    fn test_jwtclaims_is_admin_false() {
        let claims = JwtClaims {
            id: "1".to_string(),
            username: "regular".to_string(),
            email: "regular@test.com".to_string(),
            groups: vec!["users".to_string()],
            exp: 0,
        };
        assert!(!claims.is_admin());
    }

    #[test]
    fn test_jwtclaims_is_admin_empty_groups() {
        let claims = JwtClaims {
            id: "1".to_string(),
            username: "nogroups".to_string(),
            email: "nogroups@test.com".to_string(),
            groups: vec![],
            exp: 0,
        };
        assert!(!claims.is_admin());
    }
}
