// OGC API - Records protocol constants, GeoJSON mapping, and filter
// parsing (Phase 2, tasks 2.3-2.4, 2.9-2.12).
//
// The protocol identifier strings below are public GeoNetwork/CSW
// `protocol` codelist values (required for QGIS MetaSearch interop, per
// design decision #13) — not sourced from any proprietary implementation.
#![allow(dead_code)]

/// External web-map service link.
pub const PROTOCOL_WMS: &str = "OGC:WMS";
/// External web-feature service link.
pub const PROTOCOL_WFS: &str = "OGC:WFS";
/// External web-coverage service link.
pub const PROTOCOL_WCS: &str = "OGC:WCS";
/// Plain HTTP link (used for mvt-rs's own derived tile/TileJSON links).
pub const PROTOCOL_WWW_LINK: &str = "WWW:LINK-1.0-http--link";

/// All protocol identifiers this module recognizes, in declaration order.
pub const KNOWN_PROTOCOLS: [&str; 4] =
    [PROTOCOL_WMS, PROTOCOL_WFS, PROTOCOL_WCS, PROTOCOL_WWW_LINK];

/// Whether `protocol` is one of [`KNOWN_PROTOCOLS`].
pub fn is_known_protocol(protocol: &str) -> bool {
    KNOWN_PROTOCOLS.contains(&protocol)
}

use salvo::oapi::ToSchema;
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::error::{AppError, AppResult};
use crate::models::catalog::Layer;
use crate::models::metadata::{MetadataContact, MetadataLink, MetadataRecord};
use crate::services::metadata::rules::derive_autofill;

/// Maximum accepted length of the `q` free-text filter (defense in depth —
/// this becomes a bound `LIKE` parameter downstream, never interpolated).
const MAX_Q_LENGTH: usize = 500;

/// Parses the `q` query filter: trims whitespace, treats a blank string as
/// "no filter", and rejects oversized input without panicking.
pub fn parse_q(raw: &str) -> AppResult<Option<String>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > MAX_Q_LENGTH {
        return Err(AppError::InvalidInput(format!(
            "q exceeds the maximum length of {MAX_Q_LENGTH} characters"
        )));
    }
    Ok(Some(trimmed.to_string()))
}

/// Parses the `bbox` query filter (`xmin,ymin,xmax,ymax`), rejecting
/// malformed input without panicking.
pub fn parse_bbox(raw: &str) -> AppResult<[f64; 4]> {
    let parts: Vec<&str> = raw.split(',').map(str::trim).collect();
    if parts.len() != 4 {
        return Err(AppError::InvalidInput(format!(
            "bbox must have exactly 4 comma-separated numbers (xmin,ymin,xmax,ymax), got {}",
            parts.len()
        )));
    }

    let mut values = [0.0_f64; 4];
    for (i, part) in parts.iter().enumerate() {
        values[i] = part
            .parse::<f64>()
            .map_err(|_| AppError::InvalidInput(format!("bbox value '{part}' is not a number")))?;
    }

    let [xmin, ymin, xmax, ymax] = values;
    if xmin > xmax || ymin > ymax {
        return Err(AppError::InvalidInput(
            "bbox is invalid: xmin must be <= xmax and ymin must be <= ymax".to_string(),
        ));
    }

    Ok(values)
}

/// A parsed OGC API `datetime` query filter: either a single instant or an
/// interval (either side may be open, expressed as `..`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DatetimeFilter {
    Instant(OffsetDateTime),
    Interval(Option<OffsetDateTime>, Option<OffsetDateTime>),
}

fn parse_rfc3339_side(raw: &str) -> AppResult<OffsetDateTime> {
    OffsetDateTime::parse(raw, &Rfc3339)
        .map_err(|e| AppError::InvalidInput(format!("datetime value '{raw}' is not RFC3339: {e}")))
}

/// Parses the `datetime` query filter: a single RFC3339 instant, or an
/// `start/end` interval where either side may be `..` (open). Rejects
/// malformed input without panicking.
pub fn parse_datetime(raw: &str) -> AppResult<DatetimeFilter> {
    if let Some((start, end)) = raw.split_once('/') {
        if end.contains('/') {
            return Err(AppError::InvalidInput(format!(
                "datetime interval '{raw}' has more than one '/' separator"
            )));
        }

        let start = if start == ".." {
            None
        } else {
            Some(parse_rfc3339_side(start)?)
        };
        let end = if end == ".." { None } else { Some(parse_rfc3339_side(end)?) };

        if start.is_none() && end.is_none() {
            return Err(AppError::InvalidInput(
                "datetime interval cannot have both sides open".to_string(),
            ));
        }

        return Ok(DatetimeFilter::Interval(start, end));
    }

    Ok(DatetimeFilter::Instant(parse_rfc3339_side(raw)?))
}

/// OGC API - Records (Part 1: Core) conformance class URIs. Public spec
/// identifiers, retyped verbatim — not sourced from any proprietary
/// implementation.
///
/// Deliberately NOT declaring `.../conf/oas30` here even though
/// `/services/records/openapi` (`routes::build_records_routes`) now exists:
/// that conformance class asserts the served document is OpenAPI **3.0**,
/// but the pinned `salvo-oapi` 0.96.0 only emits 3.1.0 (see
/// `salvo_oapi::openapi::OpenApiVersion`, which has no 3.0 variant) —
/// claiming `oas30` while serving 3.1 would fail conformance harder than
/// the missing-API-definition warning it was meant to fix. Add it back once
/// either the document can be generated as true OAS 3.0, or the OGC
/// checker's `oas30` test is confirmed to accept 3.1.
pub const CONFORMANCE_CLASSES: [&str; 2] = [
    "http://www.opengis.net/spec/ogcapi-records-1/1.0/conf/core",
    "http://www.opengis.net/spec/ogcapi-records-1/1.0/conf/json",
];

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq)]
pub struct FeatureLink {
    pub rel: String,
    pub href: String,
    #[serde(rename = "type")]
    pub media_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq)]
#[serde(tag = "type")]
pub enum Geometry {
    Polygon { coordinates: Vec<Vec<[f64; 2]>> },
}

/// A single email/phone value on an OGC API - Records contact object
/// (design decision #5: `{value}` wrapper, not a bare string, per the
/// OGC API - Records contact schema).
#[derive(Debug, Clone, Serialize, ToSchema, PartialEq)]
pub struct ContactValue {
    pub value: String,
}

/// A responsible-party contact mapped to the OGC API - Records contact
/// object shape (design decision #5). Storage stays flat
/// (`models::metadata::MetadataContact`) — this shape exists only at the
/// discovery boundary. Blank optional fields are skipped (`null`-free
/// output), not serialized as `null`.
#[derive(Debug, Clone, Serialize, ToSchema, PartialEq)]
pub struct FeatureContact {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    pub roles: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub emails: Vec<ContactValue>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub phones: Vec<ContactValue>,
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq)]
pub struct FeatureProperties {
    pub title: String,
    pub description: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    pub keywords: Vec<String>,
    pub language: String,
    #[serde(rename = "externalIds")]
    pub external_ids: Vec<String>,
    /// `EPSG:{srid}`, derived at read time from `layer.get_srid()` — never a
    /// stored/admin-entered value (design decision #3, Phase 1.13 amendment).
    pub projection: String,
    /// Responsible-party contacts (spec "Contacts included in discovery
    /// output", Work Unit 2). Always present, empty array when the record
    /// has no contacts.
    pub contacts: Vec<FeatureContact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(rename = "publicationDate", skip_serializing_if = "Option::is_none", with = "time::serde::rfc3339::option")]
    pub publication_date: Option<OffsetDateTime>,
    #[serde(rename = "temporalExtentStart", skip_serializing_if = "Option::is_none", with = "time::serde::rfc3339::option")]
    pub temporal_extent_start: Option<OffsetDateTime>,
    #[serde(rename = "temporalExtentEnd", skip_serializing_if = "Option::is_none", with = "time::serde::rfc3339::option")]
    pub temporal_extent_end: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credits: Option<String>,
    #[serde(rename = "supplementalInformation", skip_serializing_if = "Option::is_none")]
    pub supplemental_information: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Feature {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(rename = "conformsTo")]
    pub conforms_to: Vec<String>,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<Geometry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox: Option<[f64; 4]>,
    pub properties: FeatureProperties,
    pub links: Vec<FeatureLink>,
}

/// A closed 5-point polygon ring tracing `bbox` (`[xmin, ymin, xmax, ymax]`).
fn bbox_to_polygon(bbox: [f64; 4]) -> Geometry {
    let [xmin, ymin, xmax, ymax] = bbox;
    Geometry::Polygon {
        coordinates: vec![vec![
            [xmin, ymin],
            [xmax, ymin],
            [xmax, ymax],
            [xmin, ymax],
            [xmin, ymin],
        ]],
    }
}

/// `rel`/media-type for a `MetadataLink`, based on its protocol (own
/// `WWW:LINK-1.0-http--link` derived links vs. admin-entered `OGC:*`
/// service links).
fn link_rel_and_media_type(link: &MetadataLink) -> (&'static str, &'static str) {
    match link.protocol.as_str() {
        PROTOCOL_WWW_LINK => {
            if link.url.ends_with(".pbf") {
                ("alternate", "application/vnd.mapbox-vector-tile")
            } else if link.url.ends_with(".json") {
                ("alternate", "application/json")
            } else {
                ("alternate", "text/html")
            }
        }
        _ => ("service", "application/xml"),
    }
}

fn metadata_link_to_feature_link(link: &MetadataLink) -> FeatureLink {
    let (rel, media_type) = link_rel_and_media_type(link);
    FeatureLink {
        rel: rel.to_string(),
        href: link.url.clone(),
        media_type: media_type.to_string(),
        title: link.label.clone(),
    }
}

/// `properties.created` fallback (spec "Temporal properties reflect typed
/// dates"): `creation_date` when present, else `metadata_date`.
fn created_at(record: &MetadataRecord) -> OffsetDateTime {
    record.creation_date.unwrap_or(record.metadata_date)
}

/// `properties.updated` fallback (spec "Temporal properties reflect typed
/// dates"): `revision_date` when present, else `metadata_date`.
fn updated_at(record: &MetadataRecord) -> OffsetDateTime {
    record.revision_date.unwrap_or(record.metadata_date)
}

/// Maps one stored `MetadataContact` to the OGC API - Records contact
/// object shape (design decision #5): a single `role` becomes a one-element
/// `roles` array, and `email`/`phone` become zero-or-one-element `{value}`
/// arrays so blank contact fields are omitted rather than serialized null.
fn metadata_contact_to_feature_contact(contact: &MetadataContact) -> FeatureContact {
    FeatureContact {
        name: contact.individual_name.clone(),
        organization: contact.organisation_name.clone(),
        position: contact.position_name.clone(),
        roles: vec![contact.role.clone()],
        emails: contact
            .email
            .clone()
            .map(|value| vec![ContactValue { value }])
            .unwrap_or_default(),
        phones: contact
            .phone
            .clone()
            .map(|value| vec![ContactValue { value }])
            .unwrap_or_default(),
    }
}

/// Maps a `MetadataRecord` + its `Layer` to an OGC API - Records GeoJSON
/// `Feature` (design's item shape). `bbox` is resolved separately (see
/// `rules::bbox_for_layer`, I/O) so this function stays pure; `base_url`
/// is the already-resolved absolute base
/// (`services::tilejson::resolve_base_url`).
pub fn record_to_feature(
    record: &MetadataRecord,
    layer: &Layer,
    bbox: Option<[f64; 4]>,
    collection_id: &str,
    base_url: &str,
) -> Feature {
    let autofill = derive_autofill(layer, base_url);
    let item_id = format!("{}:{}", layer.category.name, layer.name);

    let self_link = FeatureLink {
        rel: "self".to_string(),
        href: format!("{base_url}/services/records/collections/{collection_id}/items/{item_id}"),
        media_type: "application/geo+json".to_string(),
        title: None,
    };
    let collection_link = FeatureLink {
        rel: "collection".to_string(),
        href: format!("{base_url}/services/records/collections/{collection_id}"),
        media_type: "application/json".to_string(),
        title: None,
    };

    let mut links = vec![self_link, collection_link];
    links.extend(autofill.own_links.iter().map(metadata_link_to_feature_link));
    links.extend(record.links.iter().map(metadata_link_to_feature_link));

    Feature {
        type_: "Feature".to_string(),
        conforms_to: CONFORMANCE_CLASSES.iter().map(|s| s.to_string()).collect(),
        id: item_id,
        geometry: bbox.map(bbox_to_polygon),
        bbox,
        properties: FeatureProperties {
            title: autofill.title,
            description: autofill.abstract_text,
            type_: "dataset".to_string(),
            created: created_at(record),
            updated: updated_at(record),
            keywords: record.keywords.clone(),
            language: record.language.clone(),
            external_ids: vec![record.file_identifier.clone()],
            projection: autofill.projection,
            contacts: record.contacts.iter().map(metadata_contact_to_feature_contact).collect(),
            purpose: record.purpose.clone(),
            publication_date: record.publication_date,
            temporal_extent_start: record.temporal_extent_start,
            temporal_extent_end: record.temporal_extent_end,
            credits: record.credits.clone(),
            supplemental_information: record.supplemental_information.clone(),
        },
        links,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use crate::models::catalog::Layer;
    use crate::models::category::Category;
    use crate::models::metadata::{MetadataContact, MetadataLink, MetadataRecord};
    use time::macros::datetime;

    fn test_layer() -> Layer {
        Layer {
            id: "layer-1".to_string(),
            category: Category {
                id: "cat-1".to_string(),
                name: "public".to_string(),
                description: String::new(),
            },
            geometry: "polygons".to_string(),
            name: "parcels".to_string(),
            alias: "Parcels".to_string(),
            description: "Cadastral parcels".to_string(),
            database_id: "default".to_string(),
            schema: "public".to_string(),
            table_name: "parcels".to_string(),
            fields: vec!["gid".to_string()],
            filter: None,
            srid: None,
            geom: None,
            label_layer: false,
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
            groups: None,
        }
    }

    fn test_record() -> MetadataRecord {
        MetadataRecord {
            id: "rec-1".to_string(),
            layer_id: "layer-1".to_string(),
            file_identifier: "file-1".to_string(),
            language: "spa".to_string(),
            character_set: None,
            topic_category: Some("boundaries".to_string()),
            keywords: vec!["catastro".to_string()],
            maintenance_frequency: None,
            restrictions: None,
            lineage: None,
            scale: None,
            spatial_resolution: None,
            status: None,
            edition: None,
            purpose: None,
            creation_date: None,
            publication_date: None,
            revision_date: None,
            temporal_extent_start: None,
            temporal_extent_end: None,
            credits: None,
            supplemental_information: None,
            metadata_date: datetime!(2026-08-27 12:00:00 UTC),
            links: vec![MetadataLink {
                id: "link-1".to_string(),
                protocol: PROTOCOL_WMS.to_string(),
                url: "https://example.com/wms".to_string(),
                label: Some("WMS service".to_string()),
            }],
            contacts: Vec::new(),
        }
    }

    #[test]
    fn record_to_feature_maps_basic_properties_from_record_and_autofill() {
        let feature = record_to_feature(
            &test_record(),
            &test_layer(),
            None,
            "layers",
            "http://localhost:5887",
        );

        assert_eq!(feature.id, "public:parcels");
        assert_eq!(feature.properties.title, "Parcels");
        assert_eq!(feature.properties.description, "Cadastral parcels");
        assert_eq!(feature.properties.type_, "dataset");
        assert_eq!(feature.properties.keywords, vec!["catastro".to_string()]);
        assert_eq!(feature.properties.language, "spa");
        assert_eq!(feature.properties.external_ids, vec!["file-1".to_string()]);
    }

    #[test]
    fn record_to_feature_derives_projection_from_layer_srid() {
        let mut layer = test_layer();
        layer.srid = Some(3857);
        let feature = record_to_feature(&test_record(), &layer, None, "layers", "http://localhost:5887");
        assert_eq!(feature.properties.projection, "EPSG:3857");
    }

    #[test]
    fn record_to_feature_projection_defaults_to_epsg_4326_when_srid_is_unset() {
        let feature =
            record_to_feature(&test_record(), &test_layer(), None, "layers", "http://localhost:5887");
        assert_eq!(feature.properties.projection, "EPSG:4326");
    }

    #[test]
    fn record_to_feature_omits_geometry_and_bbox_when_bbox_is_none() {
        let feature = record_to_feature(
            &test_record(),
            &test_layer(),
            None,
            "layers",
            "http://localhost:5887",
        );

        assert!(feature.geometry.is_none());
        assert!(feature.bbox.is_none());

        let json = serde_json::to_value(&feature).unwrap();
        assert!(json.get("geometry").is_none(), "None geometry must be omitted, not null");
        assert!(json.get("bbox").is_none(), "None bbox must be omitted, not null");
    }

    #[test]
    fn record_to_feature_builds_closed_bbox_polygon_when_bbox_is_some() {
        let feature = record_to_feature(
            &test_record(),
            &test_layer(),
            Some([-60.0, -40.0, -50.0, -30.0]),
            "layers",
            "http://localhost:5887",
        );

        assert_eq!(feature.bbox, Some([-60.0, -40.0, -50.0, -30.0]));
        match feature.geometry {
            Some(Geometry::Polygon { coordinates }) => {
                let ring = &coordinates[0];
                assert_eq!(ring.len(), 5, "a closed ring has 5 points (first == last)");
                assert_eq!(ring[0], ring[4], "ring must be closed");
                assert_eq!(ring[0], [-60.0, -40.0]);
                assert_eq!(ring[2], [-50.0, -30.0]);
            }
            None => panic!("geometry must be Some when bbox is Some"),
        }
    }

    #[test]
    fn record_to_feature_created_and_updated_fall_back_to_metadata_date_when_neither_set() {
        let feature = record_to_feature(
            &test_record(),
            &test_layer(),
            None,
            "layers",
            "http://localhost:5887",
        );
        assert_eq!(feature.properties.created, datetime!(2026-08-27 12:00:00 UTC));
        assert_eq!(feature.properties.updated, datetime!(2026-08-27 12:00:00 UTC));
    }

    #[test]
    fn record_to_feature_created_uses_creation_date_when_set() {
        let mut record = test_record();
        record.creation_date = Some(datetime!(2026-01-10 00:00:00 UTC));
        let feature =
            record_to_feature(&record, &test_layer(), None, "layers", "http://localhost:5887");
        assert_eq!(feature.properties.created, datetime!(2026-01-10 00:00:00 UTC));
        // updated still falls back — revision_date was not set on this record
        assert_eq!(feature.properties.updated, datetime!(2026-08-27 12:00:00 UTC));
    }

    #[test]
    fn record_to_feature_updated_uses_revision_date_when_set() {
        let mut record = test_record();
        record.revision_date = Some(datetime!(2026-02-01 00:00:00 UTC));
        let feature =
            record_to_feature(&record, &test_layer(), None, "layers", "http://localhost:5887");
        assert_eq!(feature.properties.updated, datetime!(2026-02-01 00:00:00 UTC));
        // created still falls back — creation_date was not set on this record
        assert_eq!(feature.properties.created, datetime!(2026-08-27 12:00:00 UTC));
    }

    #[test]
    fn record_to_feature_created_and_updated_prefer_typed_dates_over_metadata_date() {
        let mut record = test_record();
        record.creation_date = Some(datetime!(2026-01-10 00:00:00 UTC));
        record.revision_date = Some(datetime!(2026-02-01 00:00:00 UTC));
        let feature =
            record_to_feature(&record, &test_layer(), None, "layers", "http://localhost:5887");
        assert_eq!(feature.properties.created, datetime!(2026-01-10 00:00:00 UTC));
        assert_eq!(feature.properties.updated, datetime!(2026-02-01 00:00:00 UTC));
    }

    #[test]
    fn record_to_feature_empty_contacts_yields_empty_array() {
        let feature = record_to_feature(
            &test_record(),
            &test_layer(),
            None,
            "layers",
            "http://localhost:5887",
        );
        assert_eq!(feature.properties.contacts, Vec::<FeatureContact>::new());
    }

    // -- descriptive/temporal properties (purpose, credits, etc.) ----------

    #[test]
    fn record_to_feature_includes_descriptive_properties_when_set() {
        let mut record = test_record();
        record.purpose = Some("Zoning reference".to_string());
        record.credits = Some("Municipality of Example".to_string());
        record.supplemental_information = Some("Updated annually.".to_string());
        record.publication_date = Some(datetime!(2026-03-01 00:00:00 UTC));
        record.temporal_extent_start = Some(datetime!(2020-01-01 00:00:00 UTC));
        record.temporal_extent_end = Some(datetime!(2026-01-01 00:00:00 UTC));

        let feature =
            record_to_feature(&record, &test_layer(), None, "layers", "http://localhost:5887");
        let json = serde_json::to_value(&feature.properties).unwrap();

        assert_eq!(json["purpose"], "Zoning reference");
        assert_eq!(json["credits"], "Municipality of Example");
        assert_eq!(json["supplementalInformation"], "Updated annually.");
        assert_eq!(json["publicationDate"], "2026-03-01T00:00:00Z");
        assert_eq!(json["temporalExtentStart"], "2020-01-01T00:00:00Z");
        assert_eq!(json["temporalExtentEnd"], "2026-01-01T00:00:00Z");
    }

    #[test]
    fn record_to_feature_omits_unset_descriptive_properties() {
        let feature = record_to_feature(
            &test_record(),
            &test_layer(),
            None,
            "layers",
            "http://localhost:5887",
        );
        let json = serde_json::to_value(&feature.properties).unwrap();

        for key in [
            "purpose",
            "credits",
            "supplementalInformation",
            "publicationDate",
            "temporalExtentStart",
            "temporalExtentEnd",
        ] {
            assert!(json.get(key).is_none(), "expected {key} to be omitted when unset");
        }
    }

    #[test]
    fn record_to_feature_maps_contacts_to_ogc_shape_with_nulls_skipped() {
        let mut record = test_record();
        record.contacts = vec![
            MetadataContact {
                id: "contact-1".to_string(),
                individual_name: Some("Ana Perez".to_string()),
                organisation_name: Some("IGN".to_string()),
                position_name: Some("GIS Analyst".to_string()),
                email: Some("ana@example.com".to_string()),
                phone: None,
                role: "pointOfContact".to_string(),
            },
            MetadataContact {
                id: "contact-2".to_string(),
                individual_name: None,
                organisation_name: Some("IGN".to_string()),
                position_name: None,
                email: None,
                phone: Some("+54 11 5555-5555".to_string()),
                role: "custodian".to_string(),
            },
        ];

        let feature =
            record_to_feature(&record, &test_layer(), None, "layers", "http://localhost:5887");

        assert_eq!(feature.properties.contacts.len(), 2);

        let first = &feature.properties.contacts[0];
        assert_eq!(first.name, Some("Ana Perez".to_string()));
        assert_eq!(first.organization, Some("IGN".to_string()));
        assert_eq!(first.position, Some("GIS Analyst".to_string()));
        assert_eq!(first.roles, vec!["pointOfContact".to_string()]);
        assert_eq!(first.emails, vec![ContactValue { value: "ana@example.com".to_string() }]);
        assert_eq!(first.phones, Vec::<ContactValue>::new());

        let second = &feature.properties.contacts[1];
        assert_eq!(second.name, None);
        assert_eq!(second.organization, Some("IGN".to_string()));
        assert_eq!(second.position, None);
        assert_eq!(second.roles, vec!["custodian".to_string()]);
        assert_eq!(second.emails, Vec::<ContactValue>::new());
        assert_eq!(second.phones, vec![ContactValue { value: "+54 11 5555-5555".to_string() }]);

        // nulls skipped in JSON, not serialized as `null`
        let json = serde_json::to_value(&feature).unwrap();
        let second_json = &json["properties"]["contacts"][1];
        assert!(second_json.get("name").is_none(), "None name must be omitted, not null");
        assert!(second_json.get("position").is_none(), "None position must be omitted, not null");
        assert!(second_json.get("emails").is_none(), "empty emails must be omitted, not null");
    }

    #[test]
    fn record_to_feature_combines_self_collection_own_and_admin_links() {
        let feature = record_to_feature(
            &test_record(),
            &test_layer(),
            None,
            "layers",
            "http://localhost:5887",
        );

        // self + collection + 2 derived own links (tiles + tilejson) + 1 admin OGC:WMS link
        assert_eq!(feature.links.len(), 5);

        let self_link = feature
            .links
            .iter()
            .find(|l| l.rel == "self")
            .expect("must include a self link");
        assert_eq!(
            self_link.href,
            "http://localhost:5887/services/records/collections/layers/items/public:parcels"
        );

        assert!(
            feature.links.iter().any(|l| l.href == "https://example.com/wms"),
            "must include the admin-entered OGC:WMS link"
        );
        assert!(
            feature
                .links
                .iter()
                .any(|l| l.href.contains("/services/tiles/public:parcels/")),
            "must include the derived own tile link"
        );
    }

    #[test]
    fn parse_q_trims_and_returns_none_for_blank_input() {
        assert_eq!(parse_q("  ").unwrap(), None);
        assert_eq!(parse_q("  catastro  ").unwrap(), Some("catastro".to_string()));
    }

    #[test]
    fn parse_q_rejects_input_over_the_max_length_without_panicking() {
        let too_long = "a".repeat(501);
        let err = parse_q(&too_long).expect_err("must reject an oversized q without panicking");
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn parse_bbox_accepts_four_comma_separated_numbers() {
        let bbox = parse_bbox("-60.0,-40.0,-50.0,-30.0").unwrap();
        assert_eq!(bbox, [-60.0, -40.0, -50.0, -30.0]);
    }

    #[test]
    fn parse_bbox_rejects_wrong_element_count_without_panicking() {
        let err = parse_bbox("-60.0,-40.0,-50.0").expect_err("must reject a 3-element bbox");
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn parse_bbox_rejects_non_numeric_values_without_panicking() {
        let err = parse_bbox("a,b,c,d").expect_err("must reject non-numeric bbox values");
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn parse_bbox_rejects_min_greater_than_max_without_panicking() {
        let err = parse_bbox("10,10,-10,-10").expect_err("must reject xmin > xmax / ymin > ymax");
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn parse_datetime_accepts_a_single_rfc3339_instant() {
        let parsed = parse_datetime("2026-08-27T12:00:00Z").unwrap();
        assert_eq!(parsed, DatetimeFilter::Instant(datetime!(2026-08-27 12:00:00 UTC)));
    }

    #[test]
    fn parse_datetime_accepts_a_closed_interval() {
        let parsed = parse_datetime("2026-01-01T00:00:00Z/2026-12-31T00:00:00Z").unwrap();
        assert_eq!(
            parsed,
            DatetimeFilter::Interval(
                Some(datetime!(2026-01-01 00:00:00 UTC)),
                Some(datetime!(2026-12-31 00:00:00 UTC)),
            )
        );
    }

    #[test]
    fn parse_datetime_accepts_an_open_start_interval() {
        let parsed = parse_datetime("../2026-12-31T00:00:00Z").unwrap();
        assert_eq!(
            parsed,
            DatetimeFilter::Interval(None, Some(datetime!(2026-12-31 00:00:00 UTC)))
        );
    }

    #[test]
    fn parse_datetime_rejects_malformed_input_without_panicking() {
        let err = parse_datetime("not-a-date").expect_err("must reject malformed datetime");
        assert!(matches!(err, AppError::InvalidInput(_)));

        let err = parse_datetime("2026-01-01T00:00:00Z/bad/extra")
            .expect_err("must reject a malformed interval without panicking");
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn protocol_constants_match_the_public_geonetwork_csw_identifiers() {
        assert_eq!(PROTOCOL_WMS, "OGC:WMS");
        assert_eq!(PROTOCOL_WFS, "OGC:WFS");
        assert_eq!(PROTOCOL_WCS, "OGC:WCS");
        assert_eq!(PROTOCOL_WWW_LINK, "WWW:LINK-1.0-http--link");
    }

    #[test]
    fn known_protocols_table_contains_all_four_constants() {
        assert_eq!(KNOWN_PROTOCOLS, [PROTOCOL_WMS, PROTOCOL_WFS, PROTOCOL_WCS, PROTOCOL_WWW_LINK]);
    }

    #[test]
    fn is_known_protocol_accepts_every_table_entry_and_rejects_unknown() {
        for protocol in KNOWN_PROTOCOLS {
            assert!(is_known_protocol(protocol));
        }
        assert!(!is_known_protocol("OGC:CSW"));
    }

    #[test]
    fn conformance_classes_omit_oas30_until_the_openapi_document_is_actually_3_0() {
        assert!(
            !CONFORMANCE_CLASSES.contains(&"http://www.opengis.net/spec/ogcapi-records-1/1.0/conf/oas30"),
            "oas30 asserts an OpenAPI 3.0 document; the served document is 3.1 (salvo-oapi 0.96.0 \
             has no 3.0 output mode) — declaring it now would fail conformance, not fix it"
        );
    }
}
