use regex::Regex;
use salvo::{Depot, Request};
use std::sync::OnceLock;
use tracing::warn;

use crate::{
    auth::JwtClaims,
    error::{AppError, AppResult},
    get_auth, get_jwt_secret,
    html::main::get_session_data,
    models::catalog::Layer,
};

fn regex_numeric_comparison() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| Regex::new(r"(?i)\b(\d+)\s*=\s*(\d+)\b").unwrap())
}

fn regex_hex() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| Regex::new(r"(?i)0x[0-9a-fA-F]+").unwrap())
}

fn regex_sys_proc() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| Regex::new(r"(?i)\b(sp_|xp_)\w+").unwrap())
}

fn regex_comment() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| Regex::new(r"(--|/\*|\*/)").unwrap())
}

fn regex_string_tautology_candidates() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| Regex::new(r"(?i)(?:OR|AND)\s+'([^']+)'\s*=\s*'([^']+)'").unwrap())
}

const DANGEROUS_KEYWORDS: &[&str] = &[
    "DROP", "DELETE", "INSERT", "UPDATE", "ALTER", "TRUNCATE", "GRANT", "REVOKE",
    "UNION", "EXEC", "EXECUTE", "DECLARE", "CAST", "Char", "NCHAR", "VARCHAR",
    "NVARCHAR", "SUSER_SNAME", "SESSION_USER", "xp_cmdshell"
];

pub fn convert_fields(fields: Vec<String>) -> String {
    let vec_fields: Vec<String> = if fields.len() == 1 {
        fields[0]
            .split(',')
            .map(|s| format!("\"{}\"", s.trim()))
            .collect()
    } else {
        fields
            .iter()
            .map(|field| format!("\"{field}\""))
            .collect::<Vec<_>>()
    };
    vec_fields.join(", ")
}

// fn is_inside_quotes(filter: &str, pos: usize) -> bool {
//     let mut in_single = false;
//     let mut in_double = false;
//     let chars: Vec<char> = filter.chars().collect();
//
//     for i in 0..=pos {
//         if i >= chars.len() { break; }
//         let c = chars[i];
//
//         if c == '\'' && !in_double {
//             if i > 0 && chars[i-1] == '\'' {
//             } else {
//                 in_single = !in_single;
//             }
//         } else if c == '"' && !in_single {
//              in_double = !in_double;
//         }
//     }
//     in_single || in_double
// }

pub fn validate_filter(filter: &str) -> AppResult<()> {
    if filter.trim().is_empty() {
        return Ok(());
    }

    for cap in regex_numeric_comparison().captures_iter(filter) {
        if cap[1] == cap[2] {
            warn!(filter, "SQL Injection attempt detected: Tautology ({}={})", &cap[1], &cap[2]);
            return Err(AppError::SqlInjectionError("Tautology detected".into()));
        }
    }

    if regex_hex().is_match(filter) {
        warn!(filter, "SQL Injection attempt detected: Hex Literal");
        return Err(AppError::SqlInjectionError("Hex literal detected".into()));
    }

    if regex_sys_proc().is_match(filter) {
        warn!(filter, "SQL Injection attempt detected: System Procedure");
        return Err(AppError::SqlInjectionError("System procedure detected".into()));
    }

    if regex_comment().is_match(filter) {
        warn!(filter, "SQL Injection attempt detected: Comment characters");
        return Err(AppError::SqlInjectionError("SQL comments detected".into()));
    }

    for cap in regex_string_tautology_candidates().captures_iter(filter) {
        if cap[1] == cap[2] {
            warn!(filter, "SQL Injection attempt detected: String Tautology");
            return Err(AppError::SqlInjectionError("String tautology detected".into()));
        }
    }

    let mut buffer = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    let chars: Vec<char> = filter.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c == '\'' && !in_double_quote {
            if i + 1 < chars.len() && chars[i+1] == '\'' {
                i += 1;
            } else {
                in_single_quote = !in_single_quote;
            }
        } else if c == '"' && !in_single_quote {
            if i + 1 < chars.len() && chars[i+1] == '"' {
                 i += 1;
            } else {
                in_double_quote = !in_double_quote;
            }
        }

        if !in_single_quote && !in_double_quote {
            buffer.push(c);
        } else {
            buffer.push(' ');
        }
        i += 1;
    }

    if in_single_quote || in_double_quote {
         return Err(AppError::SqlInjectionError("Unbalanced quotes".into()));
    }

    let upper_buffer = buffer.to_uppercase();

    for keyword in DANGEROUS_KEYWORDS {
        if let Some(idx) = upper_buffer.find(keyword) {
            let before = if idx == 0 { ' ' } else { upper_buffer.chars().nth(idx - 1).unwrap() };
            let after_idx = idx + keyword.len();
            let after = if after_idx >= upper_buffer.len() { ' ' } else { upper_buffer.chars().nth(after_idx).unwrap() };

            let is_word_start = !before.is_alphanumeric() && before != '_';
            let is_word_end = !after.is_alphanumeric() && after != '_';

            if is_word_start && is_word_end {
                warn!(filter, keyword, "Dangerous keyword detected");
                return Err(AppError::SqlInjectionError(format!("Dangerous keyword detected: {}", keyword)));
            }
        }
    }

    Ok(())
}


pub async fn validate_user_groups(
    req: &Request,
    layer: &Layer,
    depot: &mut Depot,
) -> AppResult<bool> {
    let Some(groups) = layer.groups.as_ref() else {
        return Ok(true);
    };
    if groups.is_empty() {
        return Ok(true);
    }

    let authorization = req
        .headers()
        .get("authorization")
        .and_then(|ah| ah.to_str().ok())
        .unwrap_or("");

    let mut has_common_group = false;
    let mut auth = get_auth().await.write().await;

    if authorization.starts_with("Bearer ") {
        let token = authorization.trim_start_matches("Bearer ").trim();
        let jwt_secret = get_jwt_secret();
        let token_data = jsonwebtoken::decode::<JwtClaims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(jwt_secret.as_bytes()),
            &jsonwebtoken::Validation::default(),
        );

        if let Ok(data) = token_data
            && let Some(user) = auth.get_user_by_id(&data.claims.id)
        {
            let user_group_ids: std::collections::HashSet<_> =
                user.groups.iter().map(|g| &g.id).collect();
            has_common_group = groups.iter().any(|g| user_group_ids.contains(&g.id));
        }
    } else if !authorization.is_empty() {
        let user = auth.get_user_by_authorization(authorization)?.cloned();
        if let Some(user) = user {
            let user_group_ids: std::collections::HashSet<_> =
                user.groups.iter().map(|g| &g.id).collect();
            has_common_group = groups.iter().any(|g| user_group_ids.contains(&g.id));
        }
    }

    let (is_auth, _) = get_session_data(depot).await;
    Ok(has_common_group || is_auth)
}
