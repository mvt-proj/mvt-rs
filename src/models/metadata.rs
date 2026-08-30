use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// ISO 19115 metadata record for one published layer. Title/abstract/bbox and
/// own tile/TileJSON links are NOT stored here — they are derived at read
/// time from the live `Layer` (see `services::metadata::rules`, Phase 2) to
/// avoid drift between the catalog and its metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataRecord {
    pub id: String,
    /// 1:1 with `Layer.id`; enforced UNIQUE at the storage layer.
    pub layer_id: String,
    /// The metadata record's own identifier (`MD_Metadata.fileIdentifier`),
    /// stable across edits — distinct from `id` (mvt-rs's row key).
    pub file_identifier: String,
    pub language: String,
    pub character_set: Option<String>,
    /// `MD_TopicCategoryCode`.
    pub topic_category: Option<String>,
    pub keywords: Vec<String>,
    pub maintenance_frequency: Option<String>,
    pub restrictions: Option<String>,
    pub lineage: Option<String>,
    pub scale: Option<String>,
    pub spatial_resolution: Option<String>,
    /// `MD_ProgressCode`.
    pub status: Option<String>,
    pub edition: Option<String>,
    /// `MD_DataIdentification.purpose`.
    pub purpose: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub creation_date: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub publication_date: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub revision_date: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub temporal_extent_start: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub temporal_extent_end: Option<OffsetDateTime>,
    pub credits: Option<String>,
    pub supplemental_information: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub metadata_date: OffsetDateTime,
    /// Catalog visibility state (`"draft"` / `"published"`), NOT to be
    /// confused with `status` (`MD_ProgressCode`, the ISO resource-progress
    /// value) above. Gates whether the record appears in the public OGC
    /// API - Records `items`/`item` endpoints — see
    /// `services::metadata::codelists::WORKFLOW_STATUS_CODES` for the closed
    /// vocabulary and `api::metadata::is_publicly_visible` for the gate.
    pub workflow_status: String,
    /// Admin-entered external OGC service links, distinct from the
    /// auto-derived own tile/TileJSON links.
    #[serde(default)]
    pub links: Vec<MetadataLink>,
    /// Responsible-party contacts (design decision #2: `metadata_contacts`
    /// is a structural clone of `metadata_links` — no SQL FK, delete-then-
    /// reinsert on update, Rust-side cascade).
    #[serde(default)]
    pub contacts: Vec<MetadataContact>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataLink {
    pub id: String,
    /// e.g. `OGC:WMS`, `OGC:WFS`, `OGC:WCS`, `WWW:LINK-1.0-http--link`.
    pub protocol: String,
    pub url: String,
    pub label: Option<String>,
}

/// A responsible-party contact for a metadata record (`CI_ResponsibleParty`).
/// `role` MUST be one of `services::metadata::codelists::ROLE_CODES`,
/// validated at the application layer (design decision #4) — not enforced
/// here or by a SQL CHECK constraint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataContact {
    pub id: String,
    pub individual_name: Option<String>,
    pub organisation_name: Option<String>,
    pub position_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub role: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn sample_record() -> MetadataRecord {
        MetadataRecord {
            id: "rec-1".to_string(),
            layer_id: "layer-1".to_string(),
            file_identifier: "file-1".to_string(),
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
                id: "link-1".to_string(),
                protocol: "OGC:WMS".to_string(),
                url: "https://example.com/wms".to_string(),
                label: Some("WMS service".to_string()),
            }],
            contacts: vec![MetadataContact {
                id: "contact-1".to_string(),
                individual_name: Some("Ana Perez".to_string()),
                organisation_name: Some("IGN".to_string()),
                position_name: Some("GIS Analyst".to_string()),
                email: Some("ana@example.com".to_string()),
                phone: None,
                role: "pointOfContact".to_string(),
            }],
        }
    }

    #[test]
    fn serializes_dates_as_rfc3339() {
        let record = sample_record();
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["metadata_date"], "2026-08-27T12:00:00Z");
        assert_eq!(json["creation_date"], "2026-01-10T00:00:00Z");
        assert_eq!(json["publication_date"], "2026-01-15T00:00:00Z");
        assert_eq!(json["revision_date"], "2026-02-01T00:00:00Z");
        assert_eq!(json["temporal_extent_start"], "2020-01-01T00:00:00Z");
        assert_eq!(json["temporal_extent_end"], "2026-01-01T00:00:00Z");
    }

    #[test]
    fn round_trips_through_json() {
        let record = sample_record();
        let json = serde_json::to_string(&record).unwrap();
        let parsed: MetadataRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.metadata_date, record.metadata_date);
        assert_eq!(parsed.creation_date, record.creation_date);
        assert_eq!(parsed.publication_date, record.publication_date);
        assert_eq!(parsed.revision_date, record.revision_date);
        assert_eq!(parsed.temporal_extent_start, record.temporal_extent_start);
        assert_eq!(parsed.temporal_extent_end, record.temporal_extent_end);
        assert_eq!(parsed.purpose, record.purpose);
        assert_eq!(parsed.credits, record.credits);
        assert_eq!(parsed.supplemental_information, record.supplemental_information);
        assert_eq!(parsed.keywords, record.keywords);
        assert_eq!(parsed.links, record.links);
        assert_eq!(parsed.contacts, record.contacts);
    }

    #[test]
    fn optional_date_fields_omitted_round_trip_to_none() {
        let mut record = sample_record();
        record.creation_date = None;
        record.publication_date = None;
        record.revision_date = None;
        record.temporal_extent_start = None;
        record.temporal_extent_end = None;
        let json = serde_json::to_string(&record).unwrap();
        let parsed: MetadataRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.creation_date, None);
        assert_eq!(parsed.publication_date, None);
        assert_eq!(parsed.revision_date, None);
        assert_eq!(parsed.temporal_extent_start, None);
        assert_eq!(parsed.temporal_extent_end, None);
    }

    #[test]
    fn workflow_status_round_trips_and_defaults_are_not_assumed_by_serde() {
        let mut record = sample_record();
        record.workflow_status = "draft".to_string();
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"workflow_status\":\"draft\""));
        let parsed: MetadataRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.workflow_status, "draft");
    }

    #[test]
    fn empty_contacts_round_trips_to_empty_vec() {
        let mut record = sample_record();
        record.contacts = Vec::new();
        let json = serde_json::to_string(&record).unwrap();
        let parsed: MetadataRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.contacts, Vec::<MetadataContact>::new());
    }
}
