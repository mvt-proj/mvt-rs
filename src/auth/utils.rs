use crate::error::{AppError, AppResult};
use base64::{Engine as _, engine::general_purpose};

pub fn decode_basic_auth(base64_string: &str) -> AppResult<(String, String)> {
    let parts: Vec<&str> = base64_string.splitn(2, ' ').collect();

    if parts.len() != 2 || parts[0] != "Basic" {
        return Err(AppError::BasicAuthError(
            "Invalid Basic Authentication format".to_string(),
        ));
    }

    let decoded_bytes = general_purpose::STANDARD
        .decode(parts[1])
        .map_err(|_| AppError::BasicAuthError("Failed to decode Base64".to_string()))?;

    let decoded_str = String::from_utf8(decoded_bytes)
        .map_err(|_| AppError::BasicAuthError("Failed to convert to UTF-8".to_string()))?;

    let auth_parts: Vec<&str> = decoded_str.splitn(2, ':').collect();

    if auth_parts.len() != 2 {
        return Err(AppError::BasicAuthError(
            "Invalid username:password format".to_string(),
        ));
    }

    Ok((auth_parts[0].to_string(), auth_parts[1].to_string()))
}
