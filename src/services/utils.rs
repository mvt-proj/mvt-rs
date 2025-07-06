use regex::Regex;
use salvo::{Depot, Request};

use crate::{
    auth::JwtClaims,
    error::{AppError, AppResult},
    get_auth, get_jwt_secret,
    html::main::get_session_data,
    models::catalog::Layer,
};

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

fn is_inside_quotes(filter: &str, pos: usize) -> bool {
    let mut in_quotes = false;
    for (i, c) in filter.chars().enumerate() {
        if c == '\'' {
            in_quotes = !in_quotes;
        }
        if i == pos {
            return in_quotes;
        }
    }
    false
}

pub fn validate_filter(filter: &str) -> AppResult<()> {
    let dangerous_keywords = [
        "DELETE", "UPDATE", "INSERT", "DROP", "TRUNCATE", "CREATE", "EXEC", "EXECUTE",
    ];

    let pattern = format!(r"(?i)\b(?:{})\b", dangerous_keywords.join("|"));
    let re =
        Regex::new(&pattern).map_err(|e| AppError::InvalidInput(format!("Regex error: {e}")))?;

    let forbidden_patterns = vec![";", "--", "/*", "*/", "OR 1=1"];
    for pattern in forbidden_patterns {
        if filter.contains(pattern) {
            return Err(AppError::InvalidInput(format!(
                "Invalid filter: contains forbidden pattern '{pattern}'"
            )));
        }
    }

    for cap in re.find_iter(filter) {
        if !is_inside_quotes(filter, cap.start()) {
            return Err(AppError::InvalidInput(format!(
                "Invalid filter: contains dangerous keyword '{}'",
                cap.as_str()
            )));
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

        if let Ok(data) = token_data {
            if let Some(user) = auth.get_user_by_id(&data.claims.id) {
                let user_group_ids: std::collections::HashSet<_> =
                    user.groups.iter().map(|g| &g.id).collect();
                has_common_group = groups.iter().any(|g| user_group_ids.contains(&g.id));
            }
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
