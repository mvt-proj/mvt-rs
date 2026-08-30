// SQLite CRUD for `metadata_records`/`metadata_links`/`metadata_contacts`
// (Phase 1 + Phase 1.5 work unit 1 of the ISO 19115 metadata integration).
// Consumed by `services/metadata/*` and `api/metadata.rs` (Phase 2/3, not yet
// wired into the module tree) plus `config::layers::delete_layer` (already
// wired, see `delete_metadata_for_layer`). Mirrors the `#[allow(dead_code)]`
// convention in `config/system_settings.rs` for ahead-of-time CRUD; remove
// once Phase 2/3 mount their callers.
#![allow(dead_code)]

use crate::config::system_settings::bump_config_version;
use crate::error::{AppError, AppResult};
use crate::get_cf_pool;
use crate::models::metadata::{MetadataContact, MetadataLink, MetadataRecord};
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

fn format_optional_rfc3339(value: Option<OffsetDateTime>) -> Result<Option<String>, sqlx::Error> {
    value.map(format_rfc3339).transpose()
}

fn parse_optional_rfc3339(value: Option<String>) -> Result<Option<OffsetDateTime>, sqlx::Error> {
    value.map(|d| parse_rfc3339(&d)).transpose()
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

/// Inserts `contacts` for `record_id`. Mirrors [`insert_links`] line-for-line
/// (design decision #2): same child-table shape, no SQL FK (see
/// `config/db.rs` — connects without `PRAGMA foreign_keys=ON`).
async fn insert_contacts(
    pool: &SqlitePool,
    record_id: &str,
    contacts: &[MetadataContact],
) -> Result<(), sqlx::Error> {
    for contact in contacts {
        sqlx::query(
            "INSERT INTO metadata_contacts (
                id, record_id, individual_name, organisation_name, position_name, email, phone, role
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&contact.id)
        .bind(record_id)
        .bind(&contact.individual_name)
        .bind(&contact.organisation_name)
        .bind(&contact.position_name)
        .bind(&contact.email)
        .bind(&contact.phone)
        .bind(&contact.role)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Fetches `record_id`'s contacts, ordered by `role` then `id` (design
/// decision #3): deterministic and semantically grouped, unlike `links`'
/// plain `ORDER BY id` — UUID `id` ordering alone would scramble
/// admin-entered order on every reload.
async fn fetch_contacts(pool: &SqlitePool, record_id: &str) -> Result<Vec<MetadataContact>, sqlx::Error> {
    let rows = sqlx::query("SELECT * FROM metadata_contacts WHERE record_id = ? ORDER BY role, id")
        .bind(record_id)
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| MetadataContact {
            id: row.get("id"),
            individual_name: row.get("individual_name"),
            organisation_name: row.get("organisation_name"),
            position_name: row.get("position_name"),
            email: row.get("email"),
            phone: row.get("phone"),
            role: row.get("role"),
        })
        .collect())
}

fn row_to_record(row: &sqlx::sqlite::SqliteRow) -> Result<MetadataRecord, sqlx::Error> {
    let keywords: String = row.get("keywords");
    let metadata_date: String = row.get("metadata_date");
    let creation_date: Option<String> = row.get("creation_date");
    let publication_date: Option<String> = row.get("publication_date");
    let revision_date: Option<String> = row.get("revision_date");
    let temporal_extent_start: Option<String> = row.get("temporal_extent_start");
    let temporal_extent_end: Option<String> = row.get("temporal_extent_end");

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
        maintenance_frequency: row.get("maintenance_frequency"),
        restrictions: row.get("restrictions"),
        lineage: row.get("lineage"),
        scale: row.get("scale"),
        spatial_resolution: row.get("spatial_resolution"),
        status: row.get("status"),
        edition: row.get("edition"),
        purpose: row.get("purpose"),
        creation_date: parse_optional_rfc3339(creation_date)?,
        publication_date: parse_optional_rfc3339(publication_date)?,
        revision_date: parse_optional_rfc3339(revision_date)?,
        temporal_extent_start: parse_optional_rfc3339(temporal_extent_start)?,
        temporal_extent_end: parse_optional_rfc3339(temporal_extent_end)?,
        credits: row.get("credits"),
        supplemental_information: row.get("supplemental_information"),
        metadata_date: parse_rfc3339(&metadata_date)?,
        workflow_status: row.get("workflow_status"),
        links: Vec::new(),
        contacts: Vec::new(),
    })
}

/// Insert a new metadata record and its links/contacts. Rejects a second
/// record for the same `layer_id` (storage-level UNIQUE constraint, mapped to
/// a typed `AppError::Conflict`).
pub async fn create_metadata_record(
    pool: Option<&SqlitePool>,
    record: &MetadataRecord,
) -> AppResult<()> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let keywords = record.keywords.join(",");
    let metadata_date = format_rfc3339(record.metadata_date)?;
    let creation_date = format_optional_rfc3339(record.creation_date)?;
    let publication_date = format_optional_rfc3339(record.publication_date)?;
    let revision_date = format_optional_rfc3339(record.revision_date)?;
    let temporal_extent_start = format_optional_rfc3339(record.temporal_extent_start)?;
    let temporal_extent_end = format_optional_rfc3339(record.temporal_extent_end)?;

    let result = sqlx::query(
        "INSERT INTO metadata_records (
            id, layer_id, file_identifier, language, character_set, topic_category, keywords,
            maintenance_frequency, restrictions, lineage, scale, spatial_resolution, status, edition,
            purpose, creation_date, publication_date, revision_date, temporal_extent_start,
            temporal_extent_end, credits, supplemental_information, metadata_date, workflow_status
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&record.id)
    .bind(&record.layer_id)
    .bind(&record.file_identifier)
    .bind(&record.language)
    .bind(&record.character_set)
    .bind(&record.topic_category)
    .bind(keywords)
    .bind(&record.maintenance_frequency)
    .bind(&record.restrictions)
    .bind(&record.lineage)
    .bind(&record.scale)
    .bind(&record.spatial_resolution)
    .bind(&record.status)
    .bind(&record.edition)
    .bind(&record.purpose)
    .bind(&creation_date)
    .bind(&publication_date)
    .bind(&revision_date)
    .bind(&temporal_extent_start)
    .bind(&temporal_extent_end)
    .bind(&record.credits)
    .bind(&record.supplemental_information)
    .bind(&metadata_date)
    .bind(&record.workflow_status)
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
    insert_contacts(pool, &record.id, &record.contacts).await?;
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
    record.contacts = fetch_contacts(pool, &record.id).await?;

    Ok(Some(record))
}

pub async fn update_metadata_record(
    pool: Option<&SqlitePool>,
    record: &MetadataRecord,
) -> AppResult<()> {
    let pool = pool.unwrap_or_else(|| get_cf_pool());

    let keywords = record.keywords.join(",");
    let metadata_date = format_rfc3339(record.metadata_date)?;
    let creation_date = format_optional_rfc3339(record.creation_date)?;
    let publication_date = format_optional_rfc3339(record.publication_date)?;
    let revision_date = format_optional_rfc3339(record.revision_date)?;
    let temporal_extent_start = format_optional_rfc3339(record.temporal_extent_start)?;
    let temporal_extent_end = format_optional_rfc3339(record.temporal_extent_end)?;

    sqlx::query(
        "UPDATE metadata_records SET
            file_identifier = ?, language = ?, character_set = ?, topic_category = ?, keywords = ?,
            maintenance_frequency = ?, restrictions = ?, lineage = ?, scale = ?, spatial_resolution = ?,
            status = ?, edition = ?, purpose = ?, creation_date = ?, publication_date = ?,
            revision_date = ?, temporal_extent_start = ?, temporal_extent_end = ?, credits = ?,
            supplemental_information = ?, metadata_date = ?, workflow_status = ? WHERE layer_id = ?",
    )
    .bind(&record.file_identifier)
    .bind(&record.language)
    .bind(&record.character_set)
    .bind(&record.topic_category)
    .bind(keywords)
    .bind(&record.maintenance_frequency)
    .bind(&record.restrictions)
    .bind(&record.lineage)
    .bind(&record.scale)
    .bind(&record.spatial_resolution)
    .bind(&record.status)
    .bind(&record.edition)
    .bind(&record.purpose)
    .bind(&creation_date)
    .bind(&publication_date)
    .bind(&revision_date)
    .bind(&temporal_extent_start)
    .bind(&temporal_extent_end)
    .bind(&record.credits)
    .bind(&record.supplemental_information)
    .bind(&metadata_date)
    .bind(&record.workflow_status)
    .bind(&record.layer_id)
    .execute(pool)
    .await?;

    sqlx::query("DELETE FROM metadata_links WHERE record_id = ?")
        .bind(&record.id)
        .execute(pool)
        .await?;
    insert_links(pool, &record.id, &record.links).await?;

    sqlx::query("DELETE FROM metadata_contacts WHERE record_id = ?")
        .bind(&record.id)
        .execute(pool)
        .await?;
    insert_contacts(pool, &record.id, &record.contacts).await?;

    bump_config_version(pool).await?;

    Ok(())
}

/// Delete the metadata record (and its links/contacts) for `layer_id`, if
/// any, and bump the config version once for the whole operation.
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
    sqlx::query("DELETE FROM metadata_contacts WHERE record_id = ?")
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
    use crate::models::metadata::{MetadataContact, MetadataLink, MetadataRecord};
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
            maintenance_frequency: None,
            restrictions: None,
            lineage: None,
            scale: None,
            spatial_resolution: None,
            status: None,
            edition: None,
            purpose: Some("Cadastral reference".to_string()),
            creation_date: Some(datetime!(2026-01-10 00:00:00 UTC)),
            publication_date: Some(datetime!(2026-01-15 00:00:00 UTC)),
            revision_date: Some(datetime!(2026-02-01 00:00:00 UTC)),
            temporal_extent_start: Some(datetime!(2020-01-01 00:00:00 UTC)),
            temporal_extent_end: Some(datetime!(2026-01-01 00:00:00 UTC)),
            credits: Some("Instituto Geografico".to_string()),
            supplemental_information: Some("See appendix A".to_string()),
            metadata_date: datetime!(2026-08-27 12:00:00 UTC),
            workflow_status: "published".to_string(),
            links: vec![MetadataLink {
                id: format!("link-{layer_id}"),
                protocol: "OGC:WMS".to_string(),
                url: "https://example.com/wms".to_string(),
                label: Some("WMS service".to_string()),
            }],
            contacts: vec![
                MetadataContact {
                    id: format!("contact-a-{layer_id}"),
                    individual_name: Some("Ana Perez".to_string()),
                    organisation_name: Some("IGN".to_string()),
                    position_name: Some("GIS Analyst".to_string()),
                    email: Some("ana@example.com".to_string()),
                    phone: None,
                    role: "pointOfContact".to_string(),
                },
                MetadataContact {
                    id: format!("contact-b-{layer_id}"),
                    individual_name: None,
                    organisation_name: Some("IGN".to_string()),
                    position_name: None,
                    email: None,
                    phone: Some("+54 11 5555-5555".to_string()),
                    role: "custodian".to_string(),
                },
            ],
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
        assert_eq!(found.metadata_date, record.metadata_date);
    }

    /// Task 1.1: create -> read -> update -> read equality on every new
    /// field (`purpose`, the 5 typed dates, `credits`,
    /// `supplemental_information`) plus 0/1/many contacts, guarding against
    /// column-list drift across the 3 hand-written SQL sites.
    #[tokio::test]
    async fn round_trip_persists_all_new_fields_and_contacts() {
        let pool = in_memory_pool().await;
        let record = sample_record("layer-1");
        create_metadata_record(Some(&pool), &record).await.unwrap();

        let found = get_metadata_record_by_layer_id(Some(&pool), "layer-1")
            .await
            .unwrap()
            .expect("record must be found");

        assert_eq!(found.purpose, record.purpose);
        assert_eq!(found.creation_date, record.creation_date);
        assert_eq!(found.publication_date, record.publication_date);
        assert_eq!(found.revision_date, record.revision_date);
        assert_eq!(found.temporal_extent_start, record.temporal_extent_start);
        assert_eq!(found.temporal_extent_end, record.temporal_extent_end);
        assert_eq!(found.credits, record.credits);
        assert_eq!(found.supplemental_information, record.supplemental_information);
        // fetch_contacts orders by `role, id` (design decision #3), not
        // insertion order, so compare a role-sorted clone of what we wrote.
        let mut expected_contacts = record.contacts.clone();
        expected_contacts.sort_by(|a, b| (&a.role, &a.id).cmp(&(&b.role, &b.id)));
        assert_eq!(found.contacts, expected_contacts);

        // Update: clear the dates/strings, swap in a single new contact with
        // a duplicate-role sibling to prove multiple contacts sharing one
        // role persist and read back unchanged.
        let mut updated = found.clone();
        updated.purpose = None;
        updated.creation_date = None;
        updated.credits = Some("Updated credits".to_string());
        updated.contacts = vec![
            MetadataContact {
                id: "contact-x".to_string(),
                individual_name: Some("Jose".to_string()),
                organisation_name: None,
                position_name: None,
                email: None,
                phone: None,
                role: "custodian".to_string(),
            },
            MetadataContact {
                id: "contact-y".to_string(),
                individual_name: Some("Maria".to_string()),
                organisation_name: None,
                position_name: None,
                email: None,
                phone: None,
                role: "custodian".to_string(),
            },
        ];
        update_metadata_record(Some(&pool), &updated).await.unwrap();

        let found_after_update = get_metadata_record_by_layer_id(Some(&pool), "layer-1")
            .await
            .unwrap()
            .expect("record must still exist after update");

        assert_eq!(found_after_update.purpose, None);
        assert_eq!(found_after_update.creation_date, None);
        assert_eq!(found_after_update.credits, Some("Updated credits".to_string()));
        assert_eq!(found_after_update.contacts.len(), 2);
        assert!(found_after_update.contacts.iter().all(|c| c.role == "custodian"));
    }

    #[tokio::test]
    async fn round_trip_persists_workflow_status_through_create_and_update() {
        let pool = in_memory_pool().await;
        let mut record = sample_record("layer-1");
        record.workflow_status = "draft".to_string();
        create_metadata_record(Some(&pool), &record).await.unwrap();

        let found = get_metadata_record_by_layer_id(Some(&pool), "layer-1")
            .await
            .unwrap()
            .expect("record must be found");
        assert_eq!(found.workflow_status, "draft");

        let mut updated = found;
        updated.workflow_status = "published".to_string();
        update_metadata_record(Some(&pool), &updated).await.unwrap();

        let found_after_update = get_metadata_record_by_layer_id(Some(&pool), "layer-1")
            .await
            .unwrap()
            .expect("record must still exist");
        assert_eq!(found_after_update.workflow_status, "published");
    }

    #[tokio::test]
    async fn create_metadata_record_accepts_zero_contacts() {
        let pool = in_memory_pool().await;
        let mut record = sample_record("layer-1");
        record.contacts = Vec::new();
        create_metadata_record(Some(&pool), &record).await.unwrap();

        let found = get_metadata_record_by_layer_id(Some(&pool), "layer-1")
            .await
            .unwrap()
            .expect("record must be found");
        assert!(found.contacts.is_empty());
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
    async fn delete_metadata_record_removes_contacts_with_zero_orphans() {
        let pool = in_memory_pool().await;
        create_metadata_record(Some(&pool), &sample_record("layer-1"))
            .await
            .unwrap();

        delete_metadata_record(Some(&pool), "layer-1").await.unwrap();

        let orphan_contacts: i64 = sqlx::query("SELECT COUNT(*) AS c FROM metadata_contacts")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get("c");
        assert_eq!(orphan_contacts, 0);
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
