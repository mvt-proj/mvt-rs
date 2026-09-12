use askama::Template;
use salvo::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use salvo::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    error::AppResult,
    get_cf_pool, get_sqlite_path,
    html::utils::{BaseTemplateData, make_base},
};

#[derive(Template)]
#[template(path = "admin/system.html")]
struct SystemTemplate {
    base: BaseTemplateData,
    sqlite_path: String,
}

#[handler]
pub async fn index(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;
    let template = SystemTemplate { base, sqlite_path: get_sqlite_path().to_string() };
    res.render(Text::Html(template.render()?));
    Ok(())
}

/// Builds the downloadable filename from the current time, e.g.
/// `mvtrs-backup-20260912-113917.db`. Extracted as a pure function since
/// `backup` itself touches `get_cf_pool()` — a process-global `OnceLock`
/// only initialized in `main()` — and can't be driven end-to-end in this
/// crate's unit-test binary (same precedent as `html::admin::metadata`).
fn backup_filename(now: OffsetDateTime) -> String {
    format!(
        "mvtrs-backup-{:04}{:02}{:02}-{:02}{:02}{:02}.db",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

#[handler]
pub async fn backup(res: &mut Response) -> AppResult<()> {
    let tmp_path = std::env::temp_dir().join(format!("mvtrs-backup-{}.db", Uuid::new_v4()));

    // The target path is server-generated (UUID, no user input), so
    // interpolating it into the SQL literal is safe — VACUUM INTO doesn't
    // support a bound parameter for its target filename.
    sqlx::query(sqlx::AssertSqlSafe(format!("VACUUM INTO '{}'", tmp_path.display())))
        .execute(get_cf_pool())
        .await?;

    let bytes = tokio::fs::read(&tmp_path).await?;
    let _ = tokio::fs::remove_file(&tmp_path).await;

    let filename = backup_filename(OffsetDateTime::now_utc());

    if let Ok(value) = "application/octet-stream".parse() {
        res.headers_mut().insert(CONTENT_TYPE, value);
    }
    if let Ok(value) = format!("attachment; filename=\"{filename}\"").parse() {
        res.headers_mut().insert(CONTENT_DISPOSITION, value);
    }
    let _ = res.write_body(bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn backup_filename_formats_as_expected() {
        let now = datetime!(2026-09-12 11:39:17 UTC);
        assert_eq!(backup_filename(now), "mvtrs-backup-20260912-113917.db");
    }
}
