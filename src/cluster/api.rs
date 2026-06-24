use salvo::prelude::*;
use serde_json::json;

use crate::cluster::snapshot::build_snapshot;
use crate::config::system_settings::get_config_version;
use crate::error::{AppError, AppResult};
use crate::{get_cf_pool, get_cluster_secret, get_config_dir};

/// Constant-time byte comparison (avoids leaking secret length-prefix matches
/// via timing). Returns true only when both slices are equal.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Rejects requests whose `X-Cluster-Secret` header does not match the
/// configured cluster secret. The internal API ships config (incl. password
/// hashes), so it must never be reachable without the secret.
#[handler]
async fn cluster_secret_guard(req: &mut Request, res: &mut Response, ctrl: &mut FlowCtrl) {
    let provided = req
        .headers()
        .get("x-cluster-secret")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let expected = get_cluster_secret();
    if expected.is_empty() || !ct_eq(provided.as_bytes(), expected.as_bytes()) {
        res.status_code(StatusCode::UNAUTHORIZED);
        res.render(Json(json!({ "error": "unauthorized" })));
        ctrl.skip_rest();
    }
}

#[handler]
async fn version(res: &mut Response) -> AppResult<()> {
    let v = get_config_version(get_cf_pool())
        .await
        .map_err(AppError::from)?;
    res.render(Json(json!({ "version": v })));
    Ok(())
}

#[handler]
async fn snapshot(res: &mut Response) -> AppResult<()> {
    let snap = build_snapshot(get_config_dir(), get_cf_pool()).await?;
    res.render(Json(snap));
    Ok(())
}

/// `/internal/config/{version,snapshot}` behind the cluster-secret guard.
pub fn build_internal_routes() -> Router {
    Router::with_path("internal/config")
        .hoop(cluster_secret_guard)
        .push(Router::with_path("version").get(version))
        .push(Router::with_path("snapshot").get(snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ct_eq_matches_only_equal_slices() {
        assert!(ct_eq(b"secret", b"secret"));
        assert!(!ct_eq(b"secret", b"secres"));
        assert!(!ct_eq(b"secret", b"secre"));
    }

    #[tokio::test]
    async fn snapshot_without_secret_is_unauthorized() {
        use salvo::test::TestClient;
        let service = Service::new(build_internal_routes());
        let resp = TestClient::get("http://127.0.0.1:5800/internal/config/snapshot")
            .send(&service)
            .await;
        assert_eq!(resp.status_code.unwrap(), StatusCode::UNAUTHORIZED);
    }
}
