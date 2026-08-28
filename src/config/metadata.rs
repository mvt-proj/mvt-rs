// SQLite CRUD for `metadata_records`/`metadata_links` (Phase 1, work unit 1
// of the ISO 19115 metadata integration). Consumed by `services/metadata/*`
// and `api/metadata.rs` (Phase 2/3, not yet wired into the module tree) plus
// `config::layers::delete_layer` (already wired, see `delete_metadata_for_layer`).
// Mirrors the `#[allow(dead_code)]` convention in `config/system_settings.rs`
// for ahead-of-time CRUD; remove once Phase 2/3 mount their callers.
#![allow(dead_code)]

use crate::config::system_settings::bump_config_version;
use crate::error::{AppError, AppResult};
use crate::get_cf_pool;
use crate::models::metadata::{MetadataLink, MetadataRecord};
use sqlx::{Row, sqlite::SqlitePool};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

fn format_rfc3339(value: OffsetDateTime) -> Result<String, sqlx::Error> {
    value
        .format(&Rfc3339)
        .map_err(|e| sqlx::Error::Decode(Box::new(e)))
}

fn parse_rfc3339(value: &str) -> Result<OffsetDateTime, sqlx::Error> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|e| sqlx::Error::Decode(Box::new(e)))
}

async fn insert_links(
    pool: &SqlitePool,
    record_id: &str,
    links: &[MetadataLink],
) -> Result<(), sqlx::Error> {
    for link in links {
        sqlx::query(
            "INSERT INTO metadata_links (id, record_id, protocol, url, label) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&link.id)
        .bind(record_id)
        .bind(&link.protocol)
        .bind(&link.url)
        .bind(&link.label)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn fetch_links(pool: &SqlitePool, record_id: &str) -> Result<Vec<MetadataLink>, sqlx::Error> {
    let rows = sqlx::query("SELECT * FROM metadata_links WHERE record_id = ? ORDER BY id")
        .bind(record_id)
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| MetadataLink {
            id: row.get("id"),
            protocol: row.get("protocol"),
            url: row.get("url"),
            label: row.get("label"),
        })
        .collect())
}

fn row_to_record(row: &sqlx::sqlite::SqliteRow) -> Result<MetadataRecord, sqlx::Error> {
    let keywords: String = row.get("keywords");
    let reference_date: Option<String> = row.get("reference_date");
    let metadata_date: String = row.get("metadata_date");

    Ok(MetadataRecord {
        id: row.get("id"),
        layer_id: row.get("layer_id"),
        file_identifier: row.get("file_identifier"),
        language: row.get("language"),
        character_set: row.get("character_set"),
        topic_category: row.get("topic_category"),
        keywords: if keywords.is_empty() {
            Vec::new()
        } else {
            keywords.split(',').map(|s| s.to_string()).collect()
        },
        data_creator_contact: row.get("data_creator_contact"),
        metadata_contact: row.get("metadata_contact"),
        maintenance_frequency: row.get("maintenance_frequency"),
        restrictions: row.get("restrictions"),
        lineage: row.get("lineage"),
        scale: row.get("scale"),
        spatial_resolution: row.get("spatial_resolution"),
        status: row.get("status"),
        edition: row.get("edition"),
        reference_date: reference_date.map(|d| parse_rfc3339(&d)).transpose()?,
        metadata_date: parse_rfc3339(&metadata_date)?,
        links: Vec::new(),
    })
}

/// Insert a new metadata record and its links. Rejects a second record for
/// the same `layer_id` (storage-level UNIQUE constraint, mapped to a typed
/// `AppError::Conflict`).
pub async fn create_metadata_record(
    pool: Option<&SqlitePool>,
    record: &MetadataRecord,
) -> AppResult<()> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let keywords = record.keywords.join(",");
    let reference_date = record.reference_date.map(format_rfc3339).transpose()?;
    let metadata_date = format_rfc3339(record.metadata_date)?;

    let result = sqlx::query(
        "INSERT INTO metadata_records (
            id, layer_id, file_identifier, language, character_set, topic_category, keywords,
            data_creator_contact, metadata_contact, maintenance_frequency, restrictions, lineage,
            scale, spatial_resolution, status, edition, reference_date, metadata_date
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&record.id)
    .bind(&record.layer_id)
    .bind(&record.file_identifier)
    .bind(&record.language)
    .bind(&record.character_set)
    .bind(&record.topic_category)
    .bind(keywords)
    .bind(&record.data_creator_contact)
    .bind(&record.metadata_contact)
    .bind(&record.maintenance_frequency)
    .bind(&record.restrictions)
    .bind(&record.lineage)
    .bind(&record.scale)
    .bind(&record.spatial_resolution)
    .bind(&record.status)
    .bind(&record.edition)
    .bind(&reference_date)
    .bind(&metadata_date)
    .execute(pool)
    .await;

    match result {
        Ok(_) => {}
        Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
            return Err(AppError::Conflict(format!(
                "A metadata record already exists for layer '{}'",
                record.layer_id
            )));
        }
        Err(e) => return Err(AppError::from(e)),
    }

    insert_links(pool, &record.id, &record.links).await?;
    bump_config_version(pool).await?;

    Ok(())
}

pub async fn get_metadata_record_by_layer_id(
    pool: Option<&SqlitePool>,
    layer_id: &str,
) -> Result<Option<MetadataRecord>, sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let row = sqlx::query("SELECT * FROM metadata_records WHERE layer_id = ?")
        .bind(layer_id)
        .fetch_optional(pool)
        .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let mut record = row_to_record(&row)?;
    record.links = fetch_links(pool, &record.id).await?;

    Ok(Some(record))
}

pub async fn update_metadata_record(
    pool: Option<&SqlitePool>,
    record: &MetadataRecord,
) -> AppResult<()> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let keywords = record.keywords.join(",");
    let reference_date = record.reference_date.map(format_rfc3339).transpose()?;
    let metadata_date = format_rfc3339(record.metadata_date)?;

    sqlx::query(
        "UPDATE metadata_records SET
            file_identifier = ?, language = ?, character_set = ?, topic_category = ?, keywords = ?,
            data_creator_contact = ?, metadata_contact = ?, maintenance_frequency = ?, restrictions = ?,
            lineage = ?, scale = ?, spatial_resolution = ?, status = ?, edition = ?,
            reference_date = ?, metadata_date = ? WHERE layer_id = ?",
    )
    .bind(&record.file_identifier)
    .bind(&record.language)
    .bind(&record.character_set)
    .bind(&record.topic_category)
    .bind(keywords)
    .bind(&record.data_creator_contact)
    .bind(&record.metadata_contact)
    .bind(&record.maintenance_frequency)
    .bind(&record.restrictions)
    .bind(&record.lineage)
    .bind(&record.scale)
    .bind(&record.spatial_resolution)
    .bind(&record.status)
    .bind(&record.edition)
    .bind(&reference_date)
    .bind(&metadata_date)
    .bind(&record.layer_id)
    .execute(pool)
    .await?;

    sqlx::query("DELETE FROM metadata_links WHERE record_id = ?")
        .bind(&record.id)
        .execute(pool)
        .await?;
    insert_links(pool, &record.id, &record.links).await?;

    bump_config_version(pool).await?;

    Ok(())
}

/// Delete the metadata record (and its links) for `layer_id`, if any, and
/// bump the config version once for the whole operation.
pub async fn delete_metadata_record(
    pool: Option<&SqlitePool>,
    layer_id: &str,
) -> Result<(), sqlx::Error> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    delete_metadata_for_layer(pool, layer_id).await?;
    bump_config_version(pool).await?;

    Ok(())
}

/// Cascade-delete helper for `config::layers::delete_layer` (design decision
/// #2). Deliberately does NOT bump the config version — the caller already
/// bumps once for the whole layer-delete operation.
pub(crate) async fn delete_metadata_for_layer(
    pool: &SqlitePool,
    layer_id: &str,
) -> Result<(), sqlx::Error> {
    let row = sqlx::query("SELECT id FROM metadata_records WHERE layer_id = ?")
        .bind(layer_id)
        .fetch_optional(pool)
        .await?;

    let Some(row) = row else {
        return Ok(());
    };

    let record_id: String = row.get("id");

    sqlx::query("DELETE FROM metadata_links WHERE record_id = ?")
        .bind(&record_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM metadata_records WHERE id = ?")
        .bind(&record_id)
        .execute(pool)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::system_settings::get_config_version;
    use crate::config::test_support::in_memory_pool;
    use crate::models::metadata::{MetadataLink, MetadataRecord};
    use time::macros::datetime;

    fn sample_record(layer_id: &str) -> MetadataRecord {
        MetadataRecord {
            id: format!("rec-{layer_id}"),
            layer_id: layer_id.to_string(),
            file_identifier: format!("file-{layer_id}"),
            language: "spa".to_string(),
            character_set: Some("utf8".to_string()),
            topic_category: Some("boundaries".to_string()),
            keywords: vec!["catastro".to_string(), "limites".to_string()],
            data_creator_contact: None,
            metadata_contact: None,
            maintenance_frequency: None,
            restrictions: None,
            lineage: None,
            scale: None,
            spatial_resolution: None,
            status: None,
            edition: None,
            reference_date: Some(datetime!(2026-01-15 00:00:00 UTC)),
            metadata_date: datetime!(2026-08-27 12:00:00 UTC),
            links: vec![MetadataLink {
                id: format!("link-{layer_id}"),
                protocol: "OGC:WMS".to_string(),
                url: "https://example.com/wms".to_string(),
                label: Some("WMS service".to_string()),
            }],
        }
    }

    #[tokio::test]
    async fn create_metadata_record_persists_row_and_bumps_version() {
        let pool = in_memory_pool().await;
        create_metadata_record(Some(&pool), &sample_record("layer-1"))
            .await
            .unwrap();

        assert_eq!(get_config_version(&pool).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn get_metadata_record_by_layer_id_returns_none_when_absent() {
        let pool = in_memory_pool().await;
        let found = get_metadata_record_by_layer_id(Some(&pool), "nonexistent")
            .await
            .unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn get_metadata_record_by_layer_id_returns_created_record_with_links() {
        let pool = in_memory_pool().await;
        let record = sample_record("layer-1");
        create_metadata_record(Some(&pool), &record).await.unwrap();

        let found = get_metadata_record_by_layer_id(Some(&pool), "layer-1")
            .await
            .unwrap()
            .expect("record must be found");

        assert_eq!(found.id, record.id);
        assert_eq!(found.topic_category, Some("boundaries".to_string()));
        assert_eq!(found.keywords, vec!["catastro", "limites"]);
        assert_eq!(found.links.len(), 1);
        assert_eq!(found.links[0].protocol, "OGC:WMS");
        assert_eq!(found.reference_date, record.reference_date);
        assert_eq!(found.metadata_date, record.metadata_date);
    }

    #[tokio::test]
    async fn update_metadata_record_persists_changes_and_bumps_version() {
        let pool = in_memory_pool().await;
        let mut record = sample_record("layer-1");
        create_metadata_record(Some(&pool), &record).await.unwrap();

        record.topic_category = Some("elevation".to_string());
        record.links = vec![MetadataLink {
            id: "link-updated".to_string(),
            protocol: "OGC:WFS".to_string(),
            url: "https://example.com/wfs".to_string(),
            label: None,
        }];
        update_metadata_record(Some(&pool), &record).await.unwrap();

        let found = get_metadata_record_by_layer_id(Some(&pool), "layer-1")
            .await
            .unwrap()
            .expect("record must still exist");
        assert_eq!(found.topic_category, Some("elevation".to_string()));
        assert_eq!(found.links.len(), 1);
        assert_eq!(found.links[0].protocol, "OGC:WFS");
        assert_eq!(get_config_version(&pool).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn delete_metadata_record_removes_row_and_bumps_version() {
        let pool = in_memory_pool().await;
        create_metadata_record(Some(&pool), &sample_record("layer-1"))
            .await
            .unwrap();

        delete_metadata_record(Some(&pool), "layer-1").await.unwrap();

        let found = get_metadata_record_by_layer_id(Some(&pool), "layer-1")
            .await
            .unwrap();
        assert!(found.is_none());
        assert_eq!(get_config_version(&pool).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn create_metadata_record_rejects_second_record_for_same_layer() {
        let pool = in_memory_pool().await;
        create_metadata_record(Some(&pool), &sample_record("layer-1"))
            .await
            .unwrap();

        let mut duplicate = sample_record("layer-1");
        duplicate.id = "rec-other".to_string();

        let err = create_metadata_record(Some(&pool), &duplicate)
            .await
            .expect_err("a second record for the same layer_id must be rejected");

        assert!(
            matches!(err, AppError::Conflict(_)),
            "expected AppError::Conflict, got {err:?}"
        );
        // Rejected create must not bump the version a second time.
        assert_eq!(get_config_version(&pool).await.unwrap(), 1);
    }
}
