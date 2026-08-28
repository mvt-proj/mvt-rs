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
    pub data_creator_contact: Option<String>,
    pub metadata_contact: Option<String>,
    pub maintenance_frequency: Option<String>,
    pub restrictions: Option<String>,
    pub lineage: Option<String>,
    pub scale: Option<String>,
    pub spatial_resolution: Option<String>,
    /// `MD_ProgressCode`.
    pub status: Option<String>,
    pub edition: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub reference_date: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub metadata_date: OffsetDateTime,
    /// Admin-entered external OGC service links, distinct from the
    /// auto-derived own tile/TileJSON links.
    #[serde(default)]
    pub links: Vec<MetadataLink>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataLink {
    pub id: String,
    /// e.g. `OGC:WMS`, `OGC:WFS`, `OGC:WCS`, `WWW:LINK-1.0-http--link`.
    pub protocol: String,
    pub url: String,
    pub label: Option<String>,
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
                id: "link-1".to_string(),
                protocol: "OGC:WMS".to_string(),
                url: "https://example.com/wms".to_string(),
                label: Some("WMS service".to_string()),
            }],
        }
    }

    #[test]
    fn serializes_dates_as_rfc3339() {
        let record = sample_record();
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["metadata_date"], "2026-08-27T12:00:00Z");
        assert_eq!(json["reference_date"], "2026-01-15T00:00:00Z");
    }

    #[test]
    fn round_trips_through_json() {
        let record = sample_record();
        let json = serde_json::to_string(&record).unwrap();
        let parsed: MetadataRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.metadata_date, record.metadata_date);
        assert_eq!(parsed.reference_date, record.reference_date);
        assert_eq!(parsed.keywords, record.keywords);
        assert_eq!(parsed.links, record.links);
    }

    #[test]
    fn reference_date_omitted_round_trips_to_none() {
        let mut record = sample_record();
        record.reference_date = None;
        let json = serde_json::to_string(&record).unwrap();
        let parsed: MetadataRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.reference_date, None);
    }
}
