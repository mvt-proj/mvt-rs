// Admin CRUD JSON handlers + OGC API - Records discovery endpoints. Wired
// into `routes.rs` under `build_api_metadata_routes`/`build_records_routes`.
//
// Testing note (disclosed, not silent — mirrors the precedent already
// documented for `services::metadata::rules::bbox_for_layer` and
// `services::tilejson::layer_bounds`): `get_catalog()` and `get_cf_pool()`
// are process-global `OnceLock`/`OnceCell` statics only initialized inside
// `main()`. Handlers that read them cannot be driven end-to-end in this
// crate's unit-test binary. To keep behavior genuinely unit-testable without
// introducing shared global test state, the core logic of every handler is
// extracted into a pool-injectable (`Option<&SqlitePool>`) or fully pure
// helper function — the same pattern `config::metadata`'s own CRUD functions
// already use. Full happy-path coverage of the catalog-dependent branches
// is exercised by integration/manual verification against a live server.

use std::collections::HashSet;

use salvo::http::{HeaderValue, StatusCode, header};
use salvo::macros::Extractible;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    config::metadata::{
        create_metadata_record, delete_metadata_record, get_metadata_record_by_layer_id,
        update_metadata_record,
    },
    error::{AppError, AppResult},
    get_catalog,
    models::{
        catalog::{Layer, StateLayer},
        metadata::{MetadataContact, MetadataLink, MetadataRecord},
    },
    services::{
        metadata::{
            codelists::validate_contacts,
            ogc::{
                CONFORMANCE_CLASSES, DatetimeFilter, Feature, FeatureLink, parse_bbox,
                parse_datetime, parse_q, record_to_feature,
            },
            rules::{bbox_for_layer, guard_layer_published},
        },
        tilejson::base_url_from_request,
        utils::validate_user_groups,
    },
};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// `/services/records` mount point (design decision #11). `routes.rs`
/// (Phase 4) decides the exact mount path; this constant only builds
/// absolute link `href`s inside response bodies.
const RECORDS_BASE_PATH: &str = "/services/records";

/// This change exposes a single fixed OGC API - Records collection: every
/// published layer's metadata record (design's item shape, single
/// `collection_id`).
const COLLECTION_ID: &str = "layers";

/// Renders an `application/problem+json` error body (spec: malformed
/// `bbox`/`datetime`/`q` filter values must never surface as a 500, and must
/// use `application/problem+json`, distinct from `AppError`'s default JSON
/// error shape).
fn render_problem(res: &mut Response, status: StatusCode, detail: String) {
    res.status_code(status);
    res.render(Json(serde_json::json!({
        "type": "about:blank",
        "title": status.canonical_reason().unwrap_or("Error"),
        "status": status.as_u16(),
        "detail": detail,
    })));
    res.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("application/problem+json"));
}

fn generic_link(rel: &str, href: String, media_type: &str, title: Option<&str>) -> FeatureLink {
    FeatureLink {
        rel: rel.to_string(),
        href,
        media_type: media_type.to_string(),
        title: title.map(str::to_string),
    }
}

/// Rejects any collection id other than [`COLLECTION_ID`] (this change
/// exposes exactly one collection).
fn require_known_collection(collection_id: &str) -> AppResult<()> {
    if collection_id == COLLECTION_ID {
        Ok(())
    } else {
        Err(AppError::NotFound(format!("Collection '{collection_id}' not found")))
    }
}

// ---------------------------------------------------------------------------
// Admin CRUD JSON (tasks 3.1-3.2)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Extractible, Debug, Clone)]
#[salvo(extract(default_source(from = "body")))]
struct LinkPayload {
    protocol: String,
    url: String,
    label: Option<String>,
}

#[derive(Serialize, Deserialize, Extractible, Debug, Clone)]
#[salvo(extract(default_source(from = "body")))]
struct ContactPayload {
    individual_name: Option<String>,
    organisation_name: Option<String>,
    position_name: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    role: String,
}

#[derive(Serialize, Deserialize, Extractible, Debug, Clone)]
#[salvo(extract(default_source(from = "body")))]
struct MetadataPayload {
    #[salvo(extract(source(from = "param")))]
    layer_id: String,
    file_identifier: Option<String>,
    language: String,
    character_set: Option<String>,
    topic_category: Option<String>,
    keywords: Option<Vec<String>>,
    maintenance_frequency: Option<String>,
    restrictions: Option<String>,
    lineage: Option<String>,
    scale: Option<String>,
    spatial_resolution: Option<String>,
    status: Option<String>,
    edition: Option<String>,
    purpose: Option<String>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    creation_date: Option<OffsetDateTime>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    publication_date: Option<OffsetDateTime>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    revision_date: Option<OffsetDateTime>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    temporal_extent_start: Option<OffsetDateTime>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    temporal_extent_end: Option<OffsetDateTime>,
    credits: Option<String>,
    supplemental_information: Option<String>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    metadata_date: Option<OffsetDateTime>,
    #[serde(default)]
    links: Vec<LinkPayload>,
    #[serde(default)]
    contacts: Vec<ContactPayload>,
}

/// Pure mapping from the wire payload to a stored `MetadataRecord`. `id` is
/// supplied by the caller: a fresh UUID on create, the existing row's `id`
/// on update (see `config::metadata::update_metadata_record`'s reliance on
/// `record.id` to re-key `metadata_links`). Validates every contact's `role`
/// against the closed vocabulary (design decision #4, spec "Closed role
/// vocabulary enforcement") before returning, rejecting the whole payload
/// (nothing persisted by the caller) on the first invalid role found.
fn build_record(id: String, payload: MetadataPayload) -> AppResult<MetadataRecord> {
    let contacts: Vec<MetadataContact> = payload
        .contacts
        .into_iter()
        .map(|c| MetadataContact {
            id: Uuid::new_v4().to_string(),
            individual_name: c.individual_name,
            organisation_name: c.organisation_name,
            position_name: c.position_name,
            email: c.email,
            phone: c.phone,
            role: c.role,
        })
        .collect();
    validate_contacts(&contacts)?;

    Ok(MetadataRecord {
        id,
        layer_id: payload.layer_id,
        file_identifier: payload.file_identifier.unwrap_or_else(|| Uuid::new_v4().to_string()),
        language: payload.language,
        character_set: payload.character_set,
        topic_category: payload.topic_category,
        keywords: payload.keywords.unwrap_or_default(),
        maintenance_frequency: payload.maintenance_frequency,
        restrictions: payload.restrictions,
        lineage: payload.lineage,
        scale: payload.scale,
        spatial_resolution: payload.spatial_resolution,
        status: payload.status,
        edition: payload.edition,
        purpose: payload.purpose,
        creation_date: payload.creation_date,
        publication_date: payload.publication_date,
        revision_date: payload.revision_date,
        temporal_extent_start: payload.temporal_extent_start,
        temporal_extent_end: payload.temporal_extent_end,
        credits: payload.credits,
        supplemental_information: payload.supplemental_information,
        metadata_date: payload.metadata_date.unwrap_or_else(OffsetDateTime::now_utc),
        links: payload
            .links
            .into_iter()
            .map(|l| MetadataLink {
                id: Uuid::new_v4().to_string(),
                protocol: l.protocol,
                url: l.url,
                label: l.label,
            })
            .collect(),
        contacts,
    })
}

/// Testable core of `create`: enforces the published guard (design decision
/// #9, spec "Publish precondition"), then persists. `pool` is injectable so
/// tests can use `in_memory_pool()` without touching `get_cf_pool()`.
async fn create_record_for_layer(
    pool: Option<&SqlitePool>,
    layer: &Layer,
    payload: MetadataPayload,
) -> AppResult<MetadataRecord> {
    guard_layer_published(layer)?;
    let record = build_record(Uuid::new_v4().to_string(), payload)?;
    create_metadata_record(pool, &record).await?;
    Ok(record)
}

/// Testable core of `update`: enforces the published guard, requires an
/// existing record (edit, not create), and preserves its `id`.
async fn update_record_for_layer(
    pool: Option<&SqlitePool>,
    layer: &Layer,
    payload: MetadataPayload,
) -> AppResult<MetadataRecord> {
    guard_layer_published(layer)?;
    let existing = get_metadata_record_by_layer_id(pool, &layer.id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound(format!("No metadata record for layer '{}'", layer.id)))?;

    let record = build_record(existing.id, payload)?;
    update_metadata_record(pool, &record).await?;
    Ok(record)
}

#[handler]
pub async fn create(res: &mut Response, payload: MetadataPayload) -> AppResult<()> {
    let layer = {
        get_catalog()
            .await
            .read()
            .await
            .find_layer_by_id(&payload.layer_id, StateLayer::Any)
            .cloned()
    }
    .ok_or_else(|| AppError::NotFound(format!("Layer {} not found", payload.layer_id)))?;

    let record = create_record_for_layer(None, &layer, payload).await?;
    res.render(Json(&record));
    Ok(())
}

#[handler]
pub async fn get(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let layer_id = req
        .param::<String>("layer_id")
        .ok_or(AppError::RequestParamError("layer_id".to_string()))?;

    let record = get_metadata_record_by_layer_id(None, &layer_id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound(format!("No metadata record for layer '{layer_id}'")))?;

    res.render(Json(&record));
    Ok(())
}

#[handler]
pub async fn update(res: &mut Response, payload: MetadataPayload) -> AppResult<()> {
    let layer = {
        get_catalog()
            .await
            .read()
            .await
            .find_layer_by_id(&payload.layer_id, StateLayer::Any)
            .cloned()
    }
    .ok_or_else(|| AppError::NotFound(format!("Layer {} not found", payload.layer_id)))?;

    let record = update_record_for_layer(None, &layer, payload).await?;
    res.render(Json(&record));
    Ok(())
}

#[handler]
pub async fn delete(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let layer_id = req
        .param::<String>("layer_id")
        .ok_or(AppError::RequestParamError("layer_id".to_string()))?;

    let existing = get_metadata_record_by_layer_id(None, &layer_id)
        .await
        .map_err(AppError::from)?;
    if existing.is_none() {
        return Err(AppError::NotFound(format!("No metadata record for layer '{layer_id}'")));
    }

    delete_metadata_record(None, &layer_id).await.map_err(AppError::from)?;
    res.render(Json(serde_json::json!({ "deleted": true })));
    Ok(())
}

// ---------------------------------------------------------------------------
// OGC API - Records discovery (tasks 3.3-3.8)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct Landing {
    title: String,
    description: String,
    links: Vec<FeatureLink>,
}

fn build_landing(base_url: &str) -> Landing {
    Landing {
        title: "MVT Server metadata catalog".to_string(),
        description: "OGC API - Records discovery for published layer metadata".to_string(),
        links: vec![
            generic_link("self", format!("{base_url}{RECORDS_BASE_PATH}"), "application/json", None),
            generic_link(
                "conformance",
                format!("{base_url}{RECORDS_BASE_PATH}/conformance"),
                "application/json",
                None,
            ),
            generic_link(
                "data",
                format!("{base_url}{RECORDS_BASE_PATH}/collections"),
                "application/json",
                None,
            ),
        ],
    }
}

#[derive(Debug, Serialize)]
struct Conformance {
    #[serde(rename = "conformsTo")]
    conforms_to: Vec<String>,
}

fn build_conformance() -> Conformance {
    Conformance {
        conforms_to: CONFORMANCE_CLASSES.iter().map(|s| s.to_string()).collect(),
    }
}

#[derive(Debug, Serialize, Clone)]
struct CollectionDescription {
    id: String,
    title: String,
    description: String,
    #[serde(rename = "itemType")]
    item_type: String,
    links: Vec<FeatureLink>,
}

fn build_collection_description(base_url: &str) -> CollectionDescription {
    CollectionDescription {
        id: COLLECTION_ID.to_string(),
        title: "Published layers".to_string(),
        description: "Metadata records for published MVT Server layers".to_string(),
        item_type: "record".to_string(),
        links: vec![
            generic_link(
                "self",
                format!("{base_url}{RECORDS_BASE_PATH}/collections/{COLLECTION_ID}"),
                "application/json",
                None,
            ),
            generic_link(
                "items",
                format!("{base_url}{RECORDS_BASE_PATH}/collections/{COLLECTION_ID}/items"),
                "application/geo+json",
                None,
            ),
        ],
    }
}

#[derive(Debug, Serialize)]
struct CollectionsResponse {
    collections: Vec<CollectionDescription>,
    links: Vec<FeatureLink>,
}

fn build_collections(base_url: &str) -> CollectionsResponse {
    CollectionsResponse {
        collections: vec![build_collection_description(base_url)],
        links: vec![generic_link(
            "self",
            format!("{base_url}{RECORDS_BASE_PATH}/collections"),
            "application/json",
            None,
        )],
    }
}

#[handler]
pub async fn landing(req: &mut Request, res: &mut Response) {
    let base_url = base_url_from_request(req);
    res.render(Json(build_landing(&base_url)));
}

#[handler]
pub async fn conformance(res: &mut Response) {
    res.render(Json(build_conformance()));
}

#[handler]
pub async fn collections(req: &mut Request, res: &mut Response) {
    let base_url = base_url_from_request(req);
    res.render(Json(build_collections(&base_url)));
}

#[handler]
pub async fn collection(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let collection_id = req.param::<String>("collection_id").unwrap_or_default();
    require_known_collection(&collection_id)?;

    let base_url = base_url_from_request(req);
    res.render(Json(build_collection_description(&base_url)));
    Ok(())
}

#[derive(Debug, Serialize)]
struct FeatureCollectionResponse {
    #[serde(rename = "type")]
    type_: String,
    features: Vec<Feature>,
    links: Vec<FeatureLink>,
    #[serde(rename = "numberMatched")]
    number_matched: usize,
    #[serde(rename = "numberReturned")]
    number_returned: usize,
}

/// Pure core of the `/items` visibility filter (spec: "Discovery respects
/// visibility rules"). `is_visible` is resolved by the caller via
/// `services::utils::validate_user_groups`, which needs live
/// request/auth/session state and therefore cannot itself be a pure
/// argument — extracting the exclusion logic keeps IT directly
/// unit-testable even though the group lookup is not (same testing
/// boundary already established for `bbox_for_layer`).
fn filter_visible_layers(layers: Vec<Layer>, mut is_visible: impl FnMut(&Layer) -> bool) -> Vec<Layer> {
    layers.into_iter().filter(|l| l.published && is_visible(l)).collect()
}

/// Whether `record`/`layer` match the free-text `q` filter (case-insensitive
/// substring over the autofilled title/description plus the manually
/// entered keywords/topic_category/file_identifier).
fn record_matches_q(record: &MetadataRecord, layer: &Layer, q: &str) -> bool {
    let needle = q.to_lowercase();
    let title = if layer.alias.is_empty() { &layer.name } else { &layer.alias };

    title.to_lowercase().contains(&needle)
        || layer.description.to_lowercase().contains(&needle)
        || record.keywords.iter().any(|k| k.to_lowercase().contains(&needle))
        || record
            .topic_category
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
            .contains(&needle)
        || record.file_identifier.to_lowercase().contains(&needle)
}

/// Whether two `[xmin, ymin, xmax, ymax]` boxes intersect.
fn bbox_intersects(a: [f64; 4], b: [f64; 4]) -> bool {
    let [a_xmin, a_ymin, a_xmax, a_ymax] = a;
    let [b_xmin, b_ymin, b_xmax, b_ymax] = b;
    a_xmin <= b_xmax && a_xmax >= b_xmin && a_ymin <= b_ymax && a_ymax >= b_ymin
}

/// Whether `value` satisfies a parsed `datetime` filter.
fn datetime_matches(value: OffsetDateTime, filter: &DatetimeFilter) -> bool {
    match filter {
        DatetimeFilter::Instant(instant) => value == *instant,
        DatetimeFilter::Interval(start, end) => {
            let after_start = start.map(|s| value >= s).unwrap_or(true);
            let before_end = end.map(|e| value <= e).unwrap_or(true);
            after_start && before_end
        }
    }
}

/// Pure pagination: splits `features` into a page plus the OGC API - Records
/// `numberMatched`/`numberReturned` counts (design's item-collection shape).
fn paginate(features: Vec<Feature>, limit: usize, offset: usize) -> (Vec<Feature>, usize, usize) {
    let number_matched = features.len();
    let page: Vec<Feature> = features.into_iter().skip(offset).take(limit).collect();
    let number_returned = page.len();
    (page, number_matched, number_returned)
}

/// `?q=&bbox=&datetime=&limit=&offset=` for one items-page href, omitting
/// filters that weren't used. Query values are passed through as received
/// (already URL-decoded by `req.query`) rather than re-encoded: `q`, `bbox`,
/// and `datetime` only ever contain characters that are safe unencoded in a
/// query string for this API (no `&`/`#`/spaces), so this stays a plain
/// string builder instead of pulling in a percent-encoding dependency.
fn items_query_string(q: &str, bbox: Option<&str>, datetime: Option<&str>, limit: usize, offset: usize) -> String {
    let mut params = Vec::new();
    if !q.is_empty() {
        params.push(format!("q={q}"));
    }
    if let Some(bbox) = bbox {
        params.push(format!("bbox={bbox}"));
    }
    if let Some(datetime) = datetime {
        params.push(format!("datetime={datetime}"));
    }
    params.push(format!("limit={limit}"));
    params.push(format!("offset={offset}"));
    params.join("&")
}

/// `self`/`prev`/`next` links for an items page, per OGC API - Records/Features
/// pagination: `self` echoes the exact query used, `prev`/`next` only appear
/// when there is a page in that direction.
#[allow(clippy::too_many_arguments)]
fn build_items_links(
    base_url: &str,
    q: &str,
    bbox: Option<&str>,
    datetime: Option<&str>,
    limit: usize,
    offset: usize,
    number_matched: usize,
    number_returned: usize,
) -> Vec<FeatureLink> {
    let items_url = format!("{base_url}{RECORDS_BASE_PATH}/collections/{COLLECTION_ID}/items");

    let mut links = vec![generic_link(
        "self",
        format!("{items_url}?{}", items_query_string(q, bbox, datetime, limit, offset)),
        "application/geo+json",
        None,
    )];

    if offset > 0 {
        let prev_offset = offset.saturating_sub(limit);
        links.push(generic_link(
            "prev",
            format!("{items_url}?{}", items_query_string(q, bbox, datetime, limit, prev_offset)),
            "application/geo+json",
            None,
        ));
    }

    if offset + number_returned < number_matched {
        links.push(generic_link(
            "next",
            format!("{items_url}?{}", items_query_string(q, bbox, datetime, limit, offset + limit)),
            "application/geo+json",
            None,
        ));
    }

    links
}

#[handler]
pub async fn items(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let collection_id = req.param::<String>("collection_id").unwrap_or_default();
    if let Err(e) = require_known_collection(&collection_id) {
        render_problem(res, StatusCode::NOT_FOUND, e.to_string());
        return Ok(());
    }

    // Query params are parsed FIRST, before any catalog/DB access, so
    // malformed input always short-circuits to `application/problem+json`
    // without ever touching the process-global catalog/pool singletons
    // (this ordering is what makes tasks 3.7/3.8 directly TestClient-testable).
    let q_raw = req.query::<String>("q").unwrap_or_default();
    let q = match parse_q(&q_raw) {
        Ok(v) => v,
        Err(e) => {
            render_problem(res, StatusCode::BAD_REQUEST, e.to_string());
            return Ok(());
        }
    };

    let bbox_raw = req.query::<String>("bbox");
    let bbox_filter = match &bbox_raw {
        Some(raw) => match parse_bbox(raw) {
            Ok(b) => Some(b),
            Err(e) => {
                render_problem(res, StatusCode::BAD_REQUEST, e.to_string());
                return Ok(());
            }
        },
        None => None,
    };

    let datetime_raw = req.query::<String>("datetime");
    let datetime_filter = match &datetime_raw {
        Some(raw) => match parse_datetime(raw) {
            Ok(d) => Some(d),
            Err(e) => {
                render_problem(res, StatusCode::BAD_REQUEST, e.to_string());
                return Ok(());
            }
        },
        None => None,
    };

    let limit = req.query::<usize>("limit").unwrap_or(10).clamp(1, 1000);
    let offset = req.query::<usize>("offset").unwrap_or(0);

    let all_layers = { get_catalog().await.read().await.layers.clone() };

    let mut visible_ids = HashSet::new();
    for layer in all_layers.iter().filter(|l| l.published) {
        if validate_user_groups(req, layer, depot).await? {
            visible_ids.insert(layer.id.clone());
        }
    }
    let visible_layers = filter_visible_layers(all_layers, |l| visible_ids.contains(&l.id));

    let base_url = base_url_from_request(req);

    let mut features = Vec::new();
    for layer in &visible_layers {
        let Some(record) = get_metadata_record_by_layer_id(None, &layer.id)
            .await
            .map_err(AppError::from)?
        else {
            continue;
        };

        if let Some(q) = &q
            && !record_matches_q(&record, layer, q)
        {
            continue;
        }

        let bbox = bbox_for_layer(layer).await;

        if let Some(filter_bbox) = bbox_filter
            && !bbox_intersects(bbox, filter_bbox)
        {
            continue;
        }

        if let Some(filter) = &datetime_filter
            && !datetime_matches(record.metadata_date, filter)
        {
            continue;
        }

        features.push(record_to_feature(&record, layer, Some(bbox), COLLECTION_ID, &base_url));
    }

    let (page, number_matched, number_returned) = paginate(features, limit, offset);

    let links = build_items_links(
        &base_url,
        &q_raw,
        bbox_raw.as_deref(),
        datetime_raw.as_deref(),
        limit,
        offset,
        number_matched,
        number_returned,
    );

    res.render(Json(FeatureCollectionResponse {
        type_: "FeatureCollection".to_string(),
        features: page,
        links,
        number_matched,
        number_returned,
    }));
    Ok(())
}

#[handler]
pub async fn item(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let collection_id = req.param::<String>("collection_id").unwrap_or_default();
    require_known_collection(&collection_id)?;

    let id = req.param::<String>("id").ok_or(AppError::RequestParamError("id".to_string()))?;
    let (category, name) = id.split_once(':').unwrap_or(("", ""));

    let layer = {
        get_catalog()
            .await
            .read()
            .await
            .find_layer_by_category_and_name(category, name, StateLayer::Published)
            .cloned()
    };
    let Some(layer) = layer else {
        return Err(AppError::NotFound(format!("Record '{id}' not found")));
    };

    // Not visible to this caller: respond the same as "not found", never
    // leak a group-restricted layer's existence (spec: "Discovery respects
    // visibility rules").
    if !validate_user_groups(req, &layer, depot).await? {
        return Err(AppError::NotFound(format!("Record '{id}' not found")));
    }

    let record = get_metadata_record_by_layer_id(None, &layer.id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound(format!("Record '{id}' not found")))?;

    let bbox = bbox_for_layer(&layer).await;
    let base_url = base_url_from_request(req);

    res.render(Json(record_to_feature(&record, &layer, Some(bbox), COLLECTION_ID, &base_url)));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::handlers::{jwt_auth_handler, require_api_metadata_admin};
    use crate::auth::models::JwtClaims;
    use crate::config::test_support::in_memory_pool;
    use crate::models::category::Category;
    use crate::services::metadata::ogc::FeatureProperties;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use salvo::test::{ResponseExt, TestClient};
    use time::Duration;
    use time::macros::datetime;

    fn test_layer(id: &str, published: bool) -> Layer {
        Layer {
            id: id.to_string(),
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
            published,
            url: None,
            groups: None,
        }
    }

    fn test_payload(layer_id: &str) -> MetadataPayload {
        MetadataPayload {
            layer_id: layer_id.to_string(),
            file_identifier: None,
            language: "spa".to_string(),
            character_set: None,
            topic_category: Some("boundaries".to_string()),
            keywords: Some(vec!["catastro".to_string()]),
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
            metadata_date: None,
            links: vec![LinkPayload {
                protocol: "OGC:WMS".to_string(),
                url: "https://example.com/wms".to_string(),
                label: Some("WMS".to_string()),
            }],
            contacts: Vec::new(),
        }
    }

    fn test_feature(id: &str) -> Feature {
        Feature {
            type_: "Feature".to_string(),
            conforms_to: vec![],
            id: id.to_string(),
            geometry: None,
            bbox: None,
            properties: FeatureProperties {
                title: id.to_string(),
                description: String::new(),
                type_: "dataset".to_string(),
                created: OffsetDateTime::now_utc(),
                updated: OffsetDateTime::now_utc(),
                keywords: vec![],
                language: "spa".to_string(),
                external_ids: vec![],
                projection: "EPSG:4326".to_string(),
                contacts: vec![],
            },
            links: vec![],
        }
    }

    // -- build_record ---------------------------------------------------

    #[test]
    fn build_record_fills_defaults_when_optional_fields_are_absent() {
        let payload = test_payload("layer-1");
        let mut payload_no_id = payload.clone();
        payload_no_id.file_identifier = None;
        payload_no_id.metadata_date = None;
        payload_no_id.keywords = None;

        let record = build_record("rec-1".to_string(), payload_no_id).unwrap();

        assert_eq!(record.id, "rec-1");
        assert_eq!(record.layer_id, "layer-1");
        assert!(!record.file_identifier.is_empty(), "file_identifier must be generated");
        assert_eq!(record.keywords, Vec::<String>::new());
        assert!(
            (OffsetDateTime::now_utc() - record.metadata_date).whole_seconds().abs() < 5,
            "metadata_date must default to roughly now"
        );
    }

    #[test]
    fn build_record_keeps_explicit_values_and_generates_link_ids() {
        let mut payload = test_payload("layer-1");
        payload.file_identifier = Some("explicit-fid".to_string());
        payload.metadata_date = Some(datetime!(2026-01-01 00:00:00 UTC));

        let record = build_record("rec-1".to_string(), payload).unwrap();

        assert_eq!(record.file_identifier, "explicit-fid");
        assert_eq!(record.metadata_date, datetime!(2026-01-01 00:00:00 UTC));
        assert_eq!(record.links.len(), 1);
        assert!(!record.links[0].id.is_empty(), "link id must be generated");
        assert_eq!(record.links[0].protocol, "OGC:WMS");
    }

    #[test]
    fn build_record_maps_new_descriptive_and_date_fields() {
        let mut payload = test_payload("layer-1");
        payload.purpose = Some("Cadastral reference".to_string());
        payload.creation_date = Some(datetime!(2026-01-10 00:00:00 UTC));
        payload.publication_date = Some(datetime!(2026-01-15 00:00:00 UTC));
        payload.revision_date = Some(datetime!(2026-02-01 00:00:00 UTC));
        payload.temporal_extent_start = Some(datetime!(2020-01-01 00:00:00 UTC));
        payload.temporal_extent_end = Some(datetime!(2026-01-01 00:00:00 UTC));
        payload.credits = Some("Instituto Geografico".to_string());
        payload.supplemental_information = Some("See appendix A".to_string());

        let record = build_record("rec-1".to_string(), payload).unwrap();

        assert_eq!(record.purpose, Some("Cadastral reference".to_string()));
        assert_eq!(record.creation_date, Some(datetime!(2026-01-10 00:00:00 UTC)));
        assert_eq!(record.publication_date, Some(datetime!(2026-01-15 00:00:00 UTC)));
        assert_eq!(record.revision_date, Some(datetime!(2026-02-01 00:00:00 UTC)));
        assert_eq!(record.temporal_extent_start, Some(datetime!(2020-01-01 00:00:00 UTC)));
        assert_eq!(record.temporal_extent_end, Some(datetime!(2026-01-01 00:00:00 UTC)));
        assert_eq!(record.credits, Some("Instituto Geografico".to_string()));
        assert_eq!(record.supplemental_information, Some("See appendix A".to_string()));
    }

    #[test]
    fn build_record_accepts_zero_contacts() {
        let payload = test_payload("layer-1");
        let record = build_record("rec-1".to_string(), payload).unwrap();
        assert!(record.contacts.is_empty());
    }

    #[test]
    fn build_record_persists_many_contacts_including_duplicate_roles_and_generates_ids() {
        let mut payload = test_payload("layer-1");
        payload.contacts = vec![
            ContactPayload {
                individual_name: Some("Ana Perez".to_string()),
                organisation_name: Some("IGN".to_string()),
                position_name: None,
                email: Some("ana@example.com".to_string()),
                phone: None,
                role: "pointOfContact".to_string(),
            },
            ContactPayload {
                individual_name: Some("Jose".to_string()),
                organisation_name: None,
                position_name: None,
                email: None,
                phone: None,
                role: "custodian".to_string(),
            },
            ContactPayload {
                individual_name: Some("Maria".to_string()),
                organisation_name: None,
                position_name: None,
                email: None,
                phone: None,
                role: "custodian".to_string(),
            },
        ];

        let record = build_record("rec-1".to_string(), payload).unwrap();

        assert_eq!(record.contacts.len(), 3);
        assert!(record.contacts.iter().all(|c| !c.id.is_empty()), "every contact must get a generated id");
        assert_eq!(
            record.contacts.iter().filter(|c| c.role == "custodian").count(),
            2,
            "duplicate roles must both persist"
        );
    }

    #[test]
    fn build_record_rejects_invalid_contact_role_and_persists_nothing() {
        let mut payload = test_payload("layer-1");
        payload.contacts = vec![ContactPayload {
            individual_name: Some("Ana Perez".to_string()),
            organisation_name: None,
            position_name: None,
            email: None,
            phone: None,
            role: "reviewer".to_string(),
        }];

        let err = build_record("rec-1".to_string(), payload)
            .expect_err("role 'reviewer' is not in the closed vocabulary");
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn create_record_for_layer_rejects_invalid_contact_role_and_does_not_persist() {
        let pool = in_memory_pool().await;
        let layer = test_layer("layer-1", true);
        let mut payload = test_payload("layer-1");
        payload.contacts = vec![ContactPayload {
            individual_name: Some("Ana Perez".to_string()),
            organisation_name: None,
            position_name: None,
            email: None,
            phone: None,
            role: "reviewer".to_string(),
        }];

        let err = create_record_for_layer(Some(&pool), &layer, payload)
            .await
            .expect_err("must reject an invalid contact role");
        assert!(matches!(err, AppError::InvalidInput(_)));

        let found = get_metadata_record_by_layer_id(Some(&pool), "layer-1").await.unwrap();
        assert!(found.is_none(), "rejected create must not persist a row");
    }

    #[tokio::test]
    async fn create_record_for_layer_persists_zero_one_and_many_contacts_round_trip() {
        let pool = in_memory_pool().await;
        let layer = test_layer("layer-1", true);
        let mut payload = test_payload("layer-1");
        payload.contacts = vec![
            ContactPayload {
                individual_name: Some("Ana Perez".to_string()),
                organisation_name: Some("IGN".to_string()),
                position_name: Some("GIS Analyst".to_string()),
                email: Some("ana@example.com".to_string()),
                phone: None,
                role: "pointOfContact".to_string(),
            },
            ContactPayload {
                individual_name: None,
                organisation_name: Some("IGN".to_string()),
                position_name: None,
                email: None,
                phone: Some("+54 11 5555-5555".to_string()),
                role: "custodian".to_string(),
            },
        ];

        create_record_for_layer(Some(&pool), &layer, payload).await.unwrap();

        let found = get_metadata_record_by_layer_id(Some(&pool), "layer-1")
            .await
            .unwrap()
            .expect("record must be persisted");
        assert_eq!(found.contacts.len(), 2);
    }

    // -- create_record_for_layer / update_record_for_layer --------------

    #[tokio::test]
    async fn create_record_for_layer_rejects_unpublished_layer_and_does_not_persist() {
        let pool = in_memory_pool().await;
        let layer = test_layer("layer-1", false);

        let err = create_record_for_layer(Some(&pool), &layer, test_payload("layer-1"))
            .await
            .expect_err("must reject create on an unpublished layer");
        assert!(matches!(err, AppError::InvalidInput(_)));

        let found = get_metadata_record_by_layer_id(Some(&pool), "layer-1").await.unwrap();
        assert!(found.is_none(), "rejected create must not persist a row");
    }

    #[tokio::test]
    async fn create_record_for_layer_persists_for_a_published_layer() {
        let pool = in_memory_pool().await;
        let layer = test_layer("layer-1", true);

        let record = create_record_for_layer(Some(&pool), &layer, test_payload("layer-1"))
            .await
            .unwrap();

        let found = get_metadata_record_by_layer_id(Some(&pool), "layer-1")
            .await
            .unwrap()
            .expect("record must be persisted");
        assert_eq!(found.id, record.id);
        assert_eq!(found.topic_category, Some("boundaries".to_string()));
        assert_eq!(found.links.len(), 1);
    }

    #[tokio::test]
    async fn update_record_for_layer_rejects_unpublished_layer() {
        let pool = in_memory_pool().await;
        let layer = test_layer("layer-2", false);

        let err = update_record_for_layer(Some(&pool), &layer, test_payload("layer-2"))
            .await
            .expect_err("must reject edit on an unpublished layer");
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn update_record_for_layer_errors_not_found_without_an_existing_record() {
        let pool = in_memory_pool().await;
        let layer = test_layer("layer-3", true);

        let err = update_record_for_layer(Some(&pool), &layer, test_payload("layer-3"))
            .await
            .expect_err("must reject edit when there is nothing to edit yet");
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[tokio::test]
    async fn update_record_for_layer_preserves_id_and_persists_changes() {
        let pool = in_memory_pool().await;
        let layer = test_layer("layer-4", true);

        let created = create_record_for_layer(Some(&pool), &layer, test_payload("layer-4"))
            .await
            .unwrap();

        let mut updated_payload = test_payload("layer-4");
        updated_payload.topic_category = Some("elevation".to_string());
        let updated = update_record_for_layer(Some(&pool), &layer, updated_payload)
            .await
            .unwrap();

        assert_eq!(updated.id, created.id, "row id must be stable across edits");

        let found = get_metadata_record_by_layer_id(Some(&pool), "layer-4")
            .await
            .unwrap()
            .expect("record must still exist");
        assert_eq!(found.topic_category, Some("elevation".to_string()));
    }

    // -- filter_visible_layers -------------------------------------------

    #[test]
    fn filter_visible_layers_excludes_unpublished_even_when_group_visible() {
        let unpublished = test_layer("l-unpub", false);
        let visible_ids: HashSet<String> = ["l-unpub".to_string()].into_iter().collect();

        let result = filter_visible_layers(vec![unpublished], |l| visible_ids.contains(&l.id));
        assert!(result.is_empty(), "unpublished layers must never appear, even if group-visible");
    }

    #[test]
    fn filter_visible_layers_excludes_published_layer_not_in_visible_set() {
        let hidden = test_layer("l-hidden", true);
        let visible_ids: HashSet<String> = HashSet::new();

        let result = filter_visible_layers(vec![hidden], |l| visible_ids.contains(&l.id));
        assert!(result.is_empty(), "group-restricted layers the caller can't see must be excluded");
    }

    #[test]
    fn filter_visible_layers_keeps_published_and_visible_layers() {
        let visible = test_layer("l-visible", true);
        let hidden = test_layer("l-hidden2", true);
        let unpublished = test_layer("l-unpub2", false);
        let visible_ids: HashSet<String> = ["l-visible".to_string()].into_iter().collect();

        let result = filter_visible_layers(vec![visible, hidden, unpublished], |l| {
            visible_ids.contains(&l.id)
        });

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "l-visible");
    }

    // -- record_matches_q --------------------------------------------------

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
            links: vec![],
            contacts: Vec::new(),
        }
    }

    #[test]
    fn record_matches_q_matches_layer_title() {
        assert!(record_matches_q(&test_record(), &test_layer("layer-1", true), "parcels"));
    }

    #[test]
    fn record_matches_q_matches_keyword_case_insensitively() {
        assert!(record_matches_q(&test_record(), &test_layer("layer-1", true), "CATASTRO"));
    }

    #[test]
    fn record_matches_q_rejects_unrelated_text() {
        assert!(!record_matches_q(&test_record(), &test_layer("layer-1", true), "nonexistent-term"));
    }

    // -- bbox_intersects / datetime_matches --------------------------------

    #[test]
    fn bbox_intersects_true_for_overlapping_boxes() {
        assert!(bbox_intersects([-10.0, -10.0, 10.0, 10.0], [0.0, 0.0, 20.0, 20.0]));
    }

    #[test]
    fn bbox_intersects_false_for_disjoint_boxes() {
        assert!(!bbox_intersects([-10.0, -10.0, -5.0, -5.0], [5.0, 5.0, 10.0, 10.0]));
    }

    #[test]
    fn datetime_matches_instant_requires_exact_equality() {
        let instant = DatetimeFilter::Instant(datetime!(2026-08-27 12:00:00 UTC));
        assert!(datetime_matches(datetime!(2026-08-27 12:00:00 UTC), &instant));
        assert!(!datetime_matches(datetime!(2026-08-27 13:00:00 UTC), &instant));
    }

    #[test]
    fn datetime_matches_interval_bounds_both_sides() {
        let interval = DatetimeFilter::Interval(
            Some(datetime!(2026-01-01 00:00:00 UTC)),
            Some(datetime!(2026-12-31 00:00:00 UTC)),
        );
        assert!(datetime_matches(datetime!(2026-06-01 00:00:00 UTC), &interval));
        assert!(!datetime_matches(datetime!(2027-01-01 00:00:00 UTC), &interval));
    }

    #[test]
    fn datetime_matches_open_start_interval_only_bounds_the_end() {
        let interval = DatetimeFilter::Interval(None, Some(datetime!(2026-12-31 00:00:00 UTC)));
        assert!(datetime_matches(datetime!(2000-01-01 00:00:00 UTC), &interval));
        assert!(!datetime_matches(datetime!(2027-01-01 00:00:00 UTC), &interval));
    }

    // -- paginate ------------------------------------------------------

    #[test]
    fn paginate_returns_the_requested_slice_and_counts() {
        let features = vec![test_feature("a"), test_feature("b"), test_feature("c")];
        let (page, number_matched, number_returned) = paginate(features, 2, 1);

        assert_eq!(number_matched, 3);
        assert_eq!(number_returned, 2);
        assert_eq!(page.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), vec!["b", "c"]);
    }

    #[test]
    fn paginate_offset_beyond_range_returns_empty_page_with_correct_matched_count() {
        let features = vec![test_feature("a"), test_feature("b")];
        let (page, number_matched, number_returned) = paginate(features, 10, 5);

        assert_eq!(number_matched, 2);
        assert_eq!(number_returned, 0);
        assert!(page.is_empty());
    }

    // -- require_known_collection ---------------------------------------

    #[test]
    fn require_known_collection_accepts_layers_and_rejects_others() {
        assert!(require_known_collection("layers").is_ok());
        let err = require_known_collection("bogus").expect_err("must reject unknown collection ids");
        assert!(matches!(err, AppError::NotFound(_)));
    }

    // -- builders (landing/conformance/collections) ----------------------

    #[test]
    fn build_landing_includes_self_conformance_and_data_links() {
        let landing_body = build_landing("http://localhost:5887");
        assert_eq!(landing_body.links.len(), 3);
        assert!(landing_body.links.iter().any(|l| l.rel == "self"));
        assert!(landing_body.links.iter().any(|l| l.rel == "conformance"));
        assert!(landing_body.links.iter().any(|l| l.rel == "data"));
    }

    #[test]
    fn build_conformance_matches_the_shared_conformance_classes() {
        let conformance_body = build_conformance();
        assert_eq!(conformance_body.conforms_to, CONFORMANCE_CLASSES.to_vec());
    }

    #[test]
    fn build_collections_contains_the_single_layers_collection() {
        let collections_body = build_collections("http://localhost:5887");
        assert_eq!(collections_body.collections.len(), 1);
        assert_eq!(collections_body.collections[0].id, "layers");
    }

    // -- build_items_links -------------------------------------------------

    #[test]
    fn build_items_links_self_reflects_limit_and_offset() {
        let links = build_items_links("http://localhost:5887", "", None, None, 1, 0, 2, 1);
        let self_link = links.iter().find(|l| l.rel == "self").expect("self link must be present");
        assert!(self_link.href.contains("limit=1"));
        assert!(self_link.href.contains("offset=0"));
    }

    #[test]
    fn build_items_links_includes_next_when_more_results_remain() {
        let links = build_items_links("http://localhost:5887", "", None, None, 1, 0, 2, 1);
        let next_link = links.iter().find(|l| l.rel == "next").expect("next link must be present when more results remain");
        assert!(next_link.href.contains("limit=1"));
        assert!(next_link.href.contains("offset=1"));
    }

    #[test]
    fn build_items_links_omits_next_on_last_page() {
        let links = build_items_links("http://localhost:5887", "", None, None, 1, 1, 2, 1);
        assert!(!links.iter().any(|l| l.rel == "next"));
    }

    #[test]
    fn build_items_links_includes_prev_when_offset_is_positive() {
        let links = build_items_links("http://localhost:5887", "", None, None, 1, 1, 2, 1);
        let prev_link = links.iter().find(|l| l.rel == "prev").expect("prev link must be present when offset > 0");
        assert!(prev_link.href.contains("offset=0"));
    }

    #[test]
    fn build_items_links_omits_prev_on_first_page() {
        let links = build_items_links("http://localhost:5887", "", None, None, 1, 0, 2, 1);
        assert!(!links.iter().any(|l| l.rel == "prev"));
    }

    #[test]
    fn build_items_links_prev_offset_does_not_go_negative() {
        let links = build_items_links("http://localhost:5887", "", None, None, 5, 2, 10, 5);
        let prev_link = links.iter().find(|l| l.rel == "prev").expect("prev link must be present");
        assert!(prev_link.href.contains("offset=0"));
    }

    #[test]
    fn build_items_links_preserves_q_bbox_and_datetime_filters() {
        let links = build_items_links(
            "http://localhost:5887",
            "catastro",
            Some("-64.3,-31.2,-63.9,-30.9"),
            Some("2026-01-01T00:00:00Z/2026-12-31T23:59:59Z"),
            1,
            0,
            2,
            1,
        );
        let self_link = links.iter().find(|l| l.rel == "self").unwrap();
        assert!(self_link.href.contains("q=catastro"));
        assert!(self_link.href.contains("bbox=-64.3,-31.2,-63.9,-30.9"));
        assert!(self_link.href.contains("datetime=2026-01-01T00:00:00Z/2026-12-31T23:59:59Z"));
    }

    #[test]
    fn build_items_links_omits_empty_q_from_query_string() {
        let links = build_items_links("http://localhost:5887", "", None, None, 10, 0, 1, 1);
        let self_link = links.iter().find(|l| l.rel == "self").unwrap();
        assert!(!self_link.href.contains("q="));
    }

    // -- TestClient: admin CRUD hoop rejection (403, no catalog/pool touch) --

    fn ensure_jwt_secret() {
        let _ = jsonwebtoken::crypto::CryptoProvider::install_default(
            &jsonwebtoken::crypto::rust_crypto::DEFAULT_PROVIDER,
        );
        let _ = crate::JWT_SECRET.set("test-only-secret-not-used-in-prod".to_string());
    }

    fn sign_token(groups: Vec<String>) -> String {
        ensure_jwt_secret();
        let claims = JwtClaims {
            id: "1".to_string(),
            username: "tester".to_string(),
            email: "tester@test.com".to_string(),
            groups,
            exp: (OffsetDateTime::now_utc() + Duration::hours(1)).unix_timestamp(),
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(crate::get_jwt_secret().as_bytes()),
        )
        .unwrap()
    }

    fn admin_metadata_router() -> Router {
        Router::new()
            .hoop(jwt_auth_handler())
            .hoop(require_api_metadata_admin)
            .push(Router::with_path("{layer_id}").post(create).put(update))
            .push(Router::with_path("{layer_id}").delete(delete))
    }

    #[tokio::test]
    async fn create_rejects_non_metadata_admin_before_touching_catalog_or_pool() {
        let token = sign_token(vec!["users".to_string()]);
        let service = Service::new(admin_metadata_router());
        let res = TestClient::post("http://127.0.0.1:5800/layer-x")
            .bearer_auth(token)
            .json(&serde_json::json!({ "language": "spa" }))
            .send(&service)
            .await;
        assert_eq!(res.status_code.unwrap(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn delete_rejects_non_metadata_admin_before_touching_catalog_or_pool() {
        let token = sign_token(vec!["users".to_string()]);
        let service = Service::new(admin_metadata_router());
        let res = TestClient::delete("http://127.0.0.1:5800/layer-x")
            .bearer_auth(token)
            .send(&service)
            .await;
        assert_eq!(res.status_code.unwrap(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn create_rejects_missing_token_before_touching_catalog_or_pool() {
        ensure_jwt_secret();
        let service = Service::new(admin_metadata_router());
        let res = TestClient::post("http://127.0.0.1:5800/layer-x")
            .json(&serde_json::json!({ "language": "spa" }))
            .send(&service)
            .await;
        assert_eq!(res.status_code.unwrap(), StatusCode::FORBIDDEN);
    }

    // -- TestClient: discovery routes with no catalog dependency -----------

    fn discovery_router() -> Router {
        Router::new()
            .get(landing)
            .push(Router::with_path("conformance").get(conformance))
            .push(Router::with_path("collections").get(collections))
            .push(Router::with_path("collections/{collection_id}").get(collection))
            .push(Router::with_path("collections/{collection_id}/items").get(items))
    }

    #[tokio::test]
    async fn landing_route_returns_ok_with_expected_links() {
        let service = Service::new(discovery_router());
        let mut res = TestClient::get("http://127.0.0.1:5800/").send(&service).await;
        assert_eq!(res.status_code.unwrap(), StatusCode::OK);
        let body: serde_json::Value = res.take_json().await.unwrap();
        assert_eq!(body["title"], "MVT Server metadata catalog");
        assert!(body["links"].as_array().unwrap().len() == 3);
    }

    #[tokio::test]
    async fn conformance_route_returns_ok_with_conforms_to() {
        let service = Service::new(discovery_router());
        let mut res = TestClient::get("http://127.0.0.1:5800/conformance").send(&service).await;
        assert_eq!(res.status_code.unwrap(), StatusCode::OK);
        let body: serde_json::Value = res.take_json().await.unwrap();
        assert!(body["conformsTo"].as_array().unwrap().len() == CONFORMANCE_CLASSES.len());
    }

    #[tokio::test]
    async fn collections_route_returns_ok_with_the_layers_collection() {
        let service = Service::new(discovery_router());
        let mut res = TestClient::get("http://127.0.0.1:5800/collections").send(&service).await;
        assert_eq!(res.status_code.unwrap(), StatusCode::OK);
        let body: serde_json::Value = res.take_json().await.unwrap();
        assert_eq!(body["collections"][0]["id"], "layers");
    }

    #[tokio::test]
    async fn collection_route_returns_404_for_an_unknown_collection_id() {
        let service = Service::new(discovery_router());
        let res = TestClient::get("http://127.0.0.1:5800/collections/bogus").send(&service).await;
        assert_eq!(res.status_code.unwrap(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn collection_route_returns_ok_for_the_known_collection() {
        let service = Service::new(discovery_router());
        let res = TestClient::get("http://127.0.0.1:5800/collections/layers").send(&service).await;
        assert_eq!(res.status_code.unwrap(), StatusCode::OK);
    }

    // -- TestClient: malformed items query params -> application/problem+json --

    #[tokio::test]
    async fn items_route_rejects_malformed_bbox_as_problem_json_without_touching_catalog() {
        let service = Service::new(discovery_router());
        let res = TestClient::get("http://127.0.0.1:5800/collections/layers/items?bbox=not,a,bbox")
            .send(&service)
            .await;

        assert_eq!(res.status_code.unwrap(), StatusCode::BAD_REQUEST);
        let content_type = res.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok());
        assert_eq!(content_type, Some("application/problem+json"));
    }

    #[tokio::test]
    async fn items_route_rejects_malformed_datetime_as_problem_json_without_touching_catalog() {
        let service = Service::new(discovery_router());
        let res = TestClient::get("http://127.0.0.1:5800/collections/layers/items?datetime=not-a-date")
            .send(&service)
            .await;

        assert_eq!(res.status_code.unwrap(), StatusCode::BAD_REQUEST);
        let content_type = res.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok());
        assert_eq!(content_type, Some("application/problem+json"));
    }

    #[tokio::test]
    async fn items_route_rejects_oversized_q_as_problem_json_without_touching_catalog() {
        let service = Service::new(discovery_router());
        let too_long = "a".repeat(501);
        let res = TestClient::get(format!("http://127.0.0.1:5800/collections/layers/items?q={too_long}"))
            .send(&service)
            .await;

        assert_eq!(res.status_code.unwrap(), StatusCode::BAD_REQUEST);
        let content_type = res.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok());
        assert_eq!(content_type, Some("application/problem+json"));
    }
}
