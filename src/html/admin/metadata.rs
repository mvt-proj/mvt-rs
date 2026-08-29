// Admin HTML CRUD for layer metadata (Phase 4, tasks 4.1-4.2). Mirrors
// `html::admin::groups`' list/new/edit/create/update/delete shape (design
// decision #12), keyed by `layer_id` instead of a freestanding entity id
// (metadata is 1:1 with a published `Layer` — decision #1).
//
// Testing note (disclosed, continuing the exact precedent already recorded
// for `api::metadata` in Work Unit 3 and `services::metadata::rules::bbox_for_layer`
// in Work Unit 2): every handler here touches `get_catalog()` and/or
// `get_cf_pool()` — process-global `OnceLock`/`OnceCell` statics only
// initialized inside `main()` — so none of them can be driven end-to-end in
// this crate's unit-test binary. `require_user_metadata_admin` itself has no
// direct TestClient coverage for the same reason (see
// `auth::handlers::tests`, comment above `is_metadata_admin_true_for_admin_group`).
// The genuinely pure mapping/parsing logic (form -> `MetadataRecord`) is
// instead extracted into standalone functions and directly unit-tested
// below; the catalog/pool-touching wiring is exercised by Phase 5
// integration/manual verification once `routes.rs` (task 4.4) mounts these
// handlers behind a live server.
#![allow(dead_code)]

use askama::Template;
use salvo::macros::Extractible;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::{
    auth::User,
    config::metadata::{
        create_metadata_record, delete_metadata_record, get_metadata_record_by_layer_id,
        update_metadata_record,
    },
    error::{AppError, AppResult},
    get_catalog,
    html::utils::{BaseTemplateData, make_base},
    models::{
        catalog::{Layer, StateLayer},
        metadata::{MetadataLink, MetadataRecord},
    },
    services::{
        metadata::{
            codelists::{
                CodelistEntry, PROGRESS_CODES, TOPIC_CATEGORY_CODES, progress_code_translate_key,
                topic_category_translate_key,
            },
            ogc::KNOWN_PROTOCOLS,
            rules::{derive_autofill, guard_layer_published},
        },
        tilejson::base_url_from_request,
    },
};

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

struct MetadataRow {
    layer: Layer,
    has_record: bool,
}

#[derive(Template)]
#[template(path = "admin/metadata/list.html")]
struct ListMetadataTemplate<'a> {
    current_user: &'a User,
    base: BaseTemplateData,
}

/// Table fragment rendered by `table_metadata`, mirroring
/// `html::catalog::CatalogTableTemplate` (`translate` sourced directly from
/// the depot rather than the whole `BaseTemplateData`, since the fragment is
/// swapped in by HTMX after the shell already rendered).
#[derive(Template)]
#[template(path = "admin/metadata/table.html")]
struct MetadataTableTemplate {
    rows: Vec<MetadataRow>,
    translate: HashMap<String, String>,
}

#[derive(Template)]
#[template(path = "admin/metadata/form.html")]
struct MetadataFormTemplate {
    layer: Layer,
    record: MetadataRecord,
    is_new: bool,
    /// `EPSG:{srid}`, derived at read time from `layer.get_srid()` (design
    /// decision #3) — read-only display, never a submittable form field
    /// (Phase 1.13 amendment).
    projection: String,
    topic_categories: Vec<CodelistOption>,
    progress_codes: Vec<CodelistOption>,
    /// Owned `String`s (rather than `KNOWN_PROTOCOLS` directly) so the
    /// `link.protocol == *protocol` equality check in `form.html` compares
    /// `String == String`, avoiding an Askama-side `&str`/`&&str` deref
    /// mismatch.
    protocols: Vec<String>,
    base: BaseTemplateData,
}

fn known_protocols() -> Vec<String> {
    KNOWN_PROTOCOLS.iter().map(|p| p.to_string()).collect()
}

/// A codelist entry ready to render: the machine `code` plus its already
/// Fluent-translated display `label` (Phase 2.5). Precomputed here — not in
/// the template — because Askama's string-concat helper (`~`) only
/// implements `Display`, not the `Borrow<str>` needed to index a
/// `HashMap<String, String>` directly (confirmed by a failed compile
/// attempt), and the project's established pattern for anything Askama
/// struggles with is to precompute owned values in the handler (Key
/// Learning #11).
struct CodelistOption {
    code: &'static str,
    label: String,
}

/// Builds the render-ready `(code, label)` list for a codelist table by
/// resolving each entry's Fluent translate key against `translate`. Falls
/// back to the raw `code` if a key is somehow missing from the loaded
/// bundle (should never happen once every locale carries all 26 keys — see
/// the completeness test in `services::metadata::codelists` — but this
/// keeps rendering non-panicking even if that invariant is ever violated).
fn codelist_options(
    table: &'static [CodelistEntry],
    translate_key: fn(&str) -> Option<String>,
    translate: &HashMap<String, String>,
) -> Vec<CodelistOption> {
    table
        .iter()
        .map(|entry| {
            let label = translate_key(entry.code)
                .and_then(|key| translate.get(&key).cloned())
                .unwrap_or_else(|| entry.code.to_string());
            CodelistOption { code: entry.code, label }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Form payload + pure mapping helpers (RED/GREEN unit-tested below)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Extractible, Debug)]
#[salvo(extract(default_source(from = "body")))]
struct MetadataForm {
    layer_id: String,
    language: String,
    character_set: Option<String>,
    topic_category: Option<String>,
    /// Comma-separated free text; see [`parse_keywords`].
    keywords: Option<String>,
    maintenance_frequency: Option<String>,
    restrictions: Option<String>,
    lineage: Option<String>,
    scale: Option<String>,
    spatial_resolution: Option<String>,
    status: Option<String>,
    edition: Option<String>,
    // NOTE: the 8 new descriptive fields (purpose, typed dates, credits,
    // supplemental_information) and contact rows are added to the form in
    // Work Unit 3 (Phase 3, tasks 3.3-3.6); this Work Unit 1 change only
    // keeps `MetadataForm`/`build_record` compiling against the updated
    // `MetadataRecord` shape.
    #[serde(default)]
    link_protocol: Vec<String>,
    #[serde(default)]
    link_url: Vec<String>,
    #[serde(default)]
    link_label: Vec<String>,
}

/// A blank, autofilled-nothing record for the "new" form (no stored row yet
/// for `layer_id`). `id`/`file_identifier` are placeholders — never
/// persisted as-is; `create_metadata` generates real ones.
fn blank_record(layer_id: &str) -> MetadataRecord {
    MetadataRecord {
        id: String::new(),
        layer_id: layer_id.to_string(),
        file_identifier: String::new(),
        language: "spa".to_string(),
        character_set: None,
        topic_category: None,
        keywords: Vec::new(),
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
        metadata_date: OffsetDateTime::now_utc(),
        links: Vec::new(),
        contacts: Vec::new(),
    }
}

/// Trims `raw`; an empty result maps to `None` (HTML text inputs submit `""`
/// rather than omitting the field when left blank).
fn non_empty(raw: Option<String>) -> Option<String> {
    raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Splits a comma-separated keyword list, trimming and dropping empties.
fn parse_keywords(raw: Option<&str>) -> Vec<String> {
    raw.map(|s| {
        s.split(',')
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

/// Parses an optional `YYYY-MM-DD` date into midnight UTC RFC3339. Empty or
/// absent input is `Ok(None)`; malformed input is a typed `InvalidInput`
/// rejection, never a panic.
fn parse_optional_date(raw: Option<&str>) -> AppResult<Option<OffsetDateTime>> {
    match raw.map(str::trim) {
        None => Ok(None),
        Some("") => Ok(None),
        Some(date) => {
            let rfc3339 = format!("{date}T00:00:00Z");
            OffsetDateTime::parse(&rfc3339, &Rfc3339)
                .map(Some)
                .map_err(|e| AppError::InvalidInput(format!("invalid date '{date}': {e}")))
        }
    }
}

/// Zips the three parallel link-row vectors submitted by the repeatable
/// external-links sub-form (design decision #6) into `MetadataLink`s. Rows
/// with an empty `url` are dropped (an admin can leave trailing blank rows).
fn build_links(protocols: Vec<String>, urls: Vec<String>, labels: Vec<String>) -> Vec<MetadataLink> {
    protocols
        .into_iter()
        .zip(urls)
        .enumerate()
        .filter(|(_, (_, url))| !url.trim().is_empty())
        .map(|(i, (protocol, url))| MetadataLink {
            id: Uuid::new_v4().to_string(),
            protocol,
            url,
            label: labels.get(i).cloned().filter(|l| !l.trim().is_empty()),
        })
        .collect()
}

/// Pure mapping from the submitted form to a `MetadataRecord`. `id` and
/// `file_identifier` are supplied by the caller: fresh UUIDs on create, the
/// existing row's values on update (`file_identifier` is stable across
/// edits per `models::metadata::MetadataRecord`'s own doc comment).
/// `metadata_date` is always "now" — it tracks last edit, not user input.
fn build_record(id: String, file_identifier: String, form: MetadataForm) -> AppResult<MetadataRecord> {
    Ok(MetadataRecord {
        id,
        layer_id: form.layer_id,
        file_identifier,
        language: form.language,
        character_set: non_empty(form.character_set),
        topic_category: non_empty(form.topic_category),
        keywords: parse_keywords(form.keywords.as_deref()),
        maintenance_frequency: non_empty(form.maintenance_frequency),
        restrictions: non_empty(form.restrictions),
        lineage: non_empty(form.lineage),
        scale: non_empty(form.scale),
        spatial_resolution: non_empty(form.spatial_resolution),
        status: non_empty(form.status),
        edition: non_empty(form.edition),
        // Work Unit 3 wires these from the form; Work Unit 1 defaults them
        // so `MetadataRecord` compiles with its new fields.
        purpose: None,
        creation_date: None,
        publication_date: None,
        revision_date: None,
        temporal_extent_start: None,
        temporal_extent_end: None,
        credits: None,
        supplemental_information: None,
        metadata_date: OffsetDateTime::now_utc(),
        links: build_links(form.link_protocol, form.link_url, form.link_label),
        contacts: Vec::new(),
    })
}

/// Whether `layer` matches the admin list's free-text `filter` (Phase 4.7),
/// case-insensitive substring match on alias/category-name/layer-name — same
/// three fields and semantics as `html::catalog::render_catalog_table`'s
/// inline filter.
fn layer_matches_filter(layer: &Layer, filter: &str) -> bool {
    let needle = filter.to_lowercase();
    layer.alias.to_lowercase().contains(&needle)
        || layer.category.name.to_lowercase().contains(&needle)
        || layer.name.to_lowercase().contains(&needle)
}

/// Formats `date` as `YYYY-MM-DD` for the form's `<input type="date">`
/// `value` attribute; `None` becomes an empty string (Askama cannot format
/// `OffsetDateTime` inline).
fn format_date_input(date: Option<OffsetDateTime>) -> String {
    date.map(|d| format!("{:04}-{:02}-{:02}", d.year(), u8::from(d.month()), d.day()))
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn load_layer(layer_id: &str) -> AppResult<Layer> {
    get_catalog()
        .await
        .read()
        .await
        .find_layer_by_id(layer_id, StateLayer::Any)
        .cloned()
        .ok_or_else(|| AppError::NotFound(format!("Layer '{layer_id}' not found")))
}

/// Thin shell handler (Phase 4.7): renders instantly with no DB/catalog
/// access. The row table is loaded separately by `table_metadata` via HTMX,
/// mirroring `html::admin::catalog::catalog_page` /
/// `html::catalog::table_catalog_admin`.
#[handler]
pub async fn list_metadata(res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, user) = make_base(depot).await;
    let Some(current_user) = user else {
        res.render(Redirect::other("/login"));
        res.status_code(StatusCode::FOUND);
        return Ok(());
    };

    let template = ListMetadataTemplate { current_user: &current_user, base };
    res.render(Text::Html(template.render()?));
    Ok(())
}

/// Table fragment handler (Phase 4.7), called by HTMX on load and on every
/// filter keystroke — mirrors `html::catalog::render_catalog_table`'s
/// filter/query shape.
#[handler]
pub async fn table_metadata(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let filter = req.query::<String>("filter");

    let published_layers: Vec<Layer> = {
        let catalog = get_catalog().await.read().await;
        catalog.layers.iter().filter(|l| l.published).cloned().collect()
    };

    let mut rows = Vec::with_capacity(published_layers.len());
    for layer in published_layers {
        if let Some(filter) = &filter
            && !layer_matches_filter(&layer, filter)
        {
            continue;
        }

        let has_record = get_metadata_record_by_layer_id(None, &layer.id)
            .await
            .map_err(AppError::from)?
            .is_some();
        rows.push(MetadataRow { layer, has_record });
    }

    let translate = depot
        .get::<HashMap<String, String>>("translate")
        .cloned()
        .unwrap_or_default();

    let template = MetadataTableTemplate { rows, translate };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn new_metadata_page(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;
    let layer_id = req
        .param::<String>("layer_id")
        .ok_or(AppError::RequestParamError("layer_id".to_string()))?;
    let layer = load_layer(&layer_id).await?;
    guard_layer_published(&layer)?;

    if get_metadata_record_by_layer_id(None, &layer_id)
        .await
        .map_err(AppError::from)?
        .is_some()
    {
        res.render(Redirect::other(format!("/admin/metadata/edit/{layer_id}")));
        res.status_code(StatusCode::FOUND);
        return Ok(());
    }

    let projection = derive_autofill(&layer, &base_url_from_request(req)).projection;
    let topic_categories = codelist_options(TOPIC_CATEGORY_CODES, topic_category_translate_key, &base.translate);
    let progress_codes = codelist_options(PROGRESS_CODES, progress_code_translate_key, &base.translate);
    let template = MetadataFormTemplate {
        record: blank_record(&layer_id),
        layer,
        is_new: true,
        projection,
        topic_categories,
        progress_codes,
        protocols: known_protocols(),
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn edit_metadata_page(req: &mut Request, res: &mut Response, depot: &mut Depot) -> AppResult<()> {
    let (base, _) = make_base(depot).await;
    let layer_id = req
        .param::<String>("layer_id")
        .ok_or(AppError::RequestParamError("layer_id".to_string()))?;
    let layer = load_layer(&layer_id).await?;

    let Some(record) = get_metadata_record_by_layer_id(None, &layer_id)
        .await
        .map_err(AppError::from)?
    else {
        res.render(Redirect::other(format!("/admin/metadata/new/{layer_id}")));
        res.status_code(StatusCode::FOUND);
        return Ok(());
    };

    let projection = derive_autofill(&layer, &base_url_from_request(req)).projection;
    let topic_categories = codelist_options(TOPIC_CATEGORY_CODES, topic_category_translate_key, &base.translate);
    let progress_codes = codelist_options(PROGRESS_CODES, progress_code_translate_key, &base.translate);
    let template = MetadataFormTemplate {
        record,
        layer,
        is_new: false,
        projection,
        topic_categories,
        progress_codes,
        protocols: known_protocols(),
        base,
    };
    res.render(Text::Html(template.render()?));
    Ok(())
}

#[handler]
pub async fn create_metadata(res: &mut Response, form: MetadataForm) -> AppResult<()> {
    let layer = load_layer(&form.layer_id).await?;
    guard_layer_published(&layer)?;

    if get_metadata_record_by_layer_id(None, &layer.id)
        .await
        .map_err(AppError::from)?
        .is_some()
    {
        res.status_code(StatusCode::CONFLICT);
        return Err(AppError::Conflict(format!(
            "layer '{}' already has a metadata record",
            layer.id
        )));
    }

    let record = build_record(Uuid::new_v4().to_string(), Uuid::new_v4().to_string(), form)?;
    create_metadata_record(None, &record).await?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/metadata"));
    Ok(())
}

#[handler]
pub async fn update_metadata(res: &mut Response, form: MetadataForm) -> AppResult<()> {
    let layer = load_layer(&form.layer_id).await?;
    guard_layer_published(&layer)?;

    let existing = get_metadata_record_by_layer_id(None, &layer.id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound(format!("No metadata record for layer '{}'", layer.id)))?;

    let record = build_record(existing.id, existing.file_identifier, form)?;
    update_metadata_record(None, &record).await?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/metadata"));
    Ok(())
}

#[handler]
pub async fn delete_metadata(req: &mut Request, res: &mut Response) -> AppResult<()> {
    let layer_id = req
        .param::<String>("layer_id")
        .ok_or(AppError::RequestParamError("layer_id".to_string()))?;

    if get_metadata_record_by_layer_id(None, &layer_id)
        .await
        .map_err(AppError::from)?
        .is_none()
    {
        res.status_code(StatusCode::NOT_FOUND);
        return Err(AppError::NotFound(format!("No metadata record for layer '{layer_id}'")));
    }

    delete_metadata_record(None, &layer_id).await.map_err(AppError::from)?;

    res.headers_mut()
        .insert("content-type", "text/html".parse()?);
    res.render(Redirect::other("/admin/metadata"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::I18n;
    use crate::models::category::Category;

    fn form(layer_id: &str) -> MetadataForm {
        MetadataForm {
            layer_id: layer_id.to_string(),
            language: "spa".to_string(),
            character_set: Some("utf8".to_string()),
            topic_category: Some("boundaries".to_string()),
            keywords: Some(" catastro , limites ,,".to_string()),
            maintenance_frequency: Some("annually".to_string()),
            restrictions: None,
            lineage: None,
            scale: Some("1:5000".to_string()),
            spatial_resolution: None,
            status: Some("onGoing".to_string()),
            edition: None,
            link_protocol: vec!["OGC:WMS".to_string(), "OGC:WFS".to_string()],
            link_url: vec!["https://example.com/wms".to_string(), String::new()],
            link_label: vec!["WMS service".to_string()],
        }
    }

    #[test]
    fn parse_keywords_trims_and_drops_empty_entries() {
        assert_eq!(
            parse_keywords(Some(" catastro , limites ,,")),
            vec!["catastro".to_string(), "limites".to_string()]
        );
    }

    #[test]
    fn parse_keywords_none_input_yields_empty_vec() {
        assert_eq!(parse_keywords(None), Vec::<String>::new());
    }

    #[test]
    fn non_empty_trims_and_converts_blank_to_none() {
        assert_eq!(non_empty(Some("  hello  ".to_string())), Some("hello".to_string()));
        assert_eq!(non_empty(Some("   ".to_string())), None);
        assert_eq!(non_empty(None), None);
    }

    #[test]
    fn parse_optional_date_accepts_a_plain_date() {
        let parsed = parse_optional_date(Some("2026-01-15")).unwrap();
        assert_eq!(parsed, OffsetDateTime::parse("2026-01-15T00:00:00Z", &Rfc3339).ok());
    }

    #[test]
    fn parse_optional_date_empty_or_absent_is_none() {
        assert_eq!(parse_optional_date(Some("")).unwrap(), None);
        assert_eq!(parse_optional_date(Some("   ")).unwrap(), None);
        assert_eq!(parse_optional_date(None).unwrap(), None);
    }

    #[test]
    fn parse_optional_date_rejects_malformed_input() {
        let err = parse_optional_date(Some("not-a-date")).expect_err("must reject malformed date");
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn build_links_zips_rows_and_drops_blank_urls() {
        let links = build_links(
            vec!["OGC:WMS".to_string(), "OGC:WFS".to_string()],
            vec!["https://example.com/wms".to_string(), String::new()],
            vec!["WMS service".to_string()],
        );
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].protocol, "OGC:WMS");
        assert_eq!(links[0].url, "https://example.com/wms");
        assert_eq!(links[0].label, Some("WMS service".to_string()));
    }

    #[test]
    fn build_links_missing_label_row_is_none() {
        let links = build_links(
            vec!["OGC:WFS".to_string()],
            vec!["https://example.com/wfs".to_string()],
            vec![],
        );
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].label, None);
    }

    #[test]
    fn build_record_maps_form_and_preserves_supplied_id_and_file_identifier() {
        let record = build_record("rec-1".to_string(), "file-1".to_string(), form("layer-1")).unwrap();
        assert_eq!(record.id, "rec-1");
        assert_eq!(record.file_identifier, "file-1");
        assert_eq!(record.layer_id, "layer-1");
        assert_eq!(record.character_set, Some("utf8".to_string()));
        assert_eq!(record.keywords, vec!["catastro".to_string(), "limites".to_string()]);
        assert_eq!(record.links.len(), 1);
        assert_eq!(record.links[0].protocol, "OGC:WMS");
        assert_eq!(record.purpose, None);
        assert_eq!(record.creation_date, None);
        assert!(record.contacts.is_empty());
    }

    #[test]
    fn format_date_input_formats_as_iso_date() {
        let date = OffsetDateTime::parse("2026-01-05T00:00:00Z", &Rfc3339).unwrap();
        assert_eq!(format_date_input(Some(date)), "2026-01-05");
    }

    #[test]
    fn format_date_input_none_is_empty_string() {
        assert_eq!(format_date_input(None), "");
    }

    #[test]
    fn codelist_options_resolves_translated_label_when_key_present() {
        let mut translate = HashMap::new();
        translate.insert("topic-category-boundaries".to_string(), "Boundaries".to_string());
        let options = codelist_options(TOPIC_CATEGORY_CODES, topic_category_translate_key, &translate);
        let boundaries = options.iter().find(|o| o.code == "boundaries").unwrap();
        assert_eq!(boundaries.label, "Boundaries");
    }

    #[test]
    fn codelist_options_falls_back_to_code_when_key_missing() {
        let translate = HashMap::new();
        let options = codelist_options(PROGRESS_CODES, progress_code_translate_key, &translate);
        let on_going = options.iter().find(|o| o.code == "onGoing").unwrap();
        assert_eq!(on_going.label, "onGoing");
    }

    // -- layer_matches_filter (Phase 4.7) -----------------------------------

    #[test]
    fn layer_matches_filter_matches_alias_case_insensitively() {
        assert!(layer_matches_filter(&test_layer("layer-1"), "PARCELS"));
    }

    #[test]
    fn layer_matches_filter_matches_category_name() {
        assert!(layer_matches_filter(&test_layer("layer-1"), "public"));
    }

    #[test]
    fn layer_matches_filter_matches_layer_name() {
        assert!(layer_matches_filter(&test_layer("layer-1"), "parcels"));
    }

    #[test]
    fn layer_matches_filter_rejects_unrelated_text() {
        assert!(!layer_matches_filter(&test_layer("layer-1"), "nonexistent-term"));
    }

    #[test]
    fn blank_record_defaults_are_empty() {
        let record = blank_record("layer-1");
        assert_eq!(record.layer_id, "layer-1");
        assert_eq!(record.language, "spa");
        assert!(record.keywords.is_empty());
        assert!(record.links.is_empty());
        assert_eq!(record.topic_category, None);
    }

    fn test_layer(id: &str) -> Layer {
        Layer {
            id: id.to_string(),
            category: Category { id: "cat-1".to_string(), name: "public".to_string(), description: String::new() },
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

    fn test_user() -> User {
        User {
            id: "user-1".to_string(),
            username: "admin".to_string(),
            email: "admin@example.com".to_string(),
            first_name: None,
            last_name: None,
            password: String::new(),
            groups: vec![],
        }
    }

    fn full_table_translate() -> HashMap<String, String> {
        [("category", "Category"), ("layer-name", "Layer"), ("alias", "Alias"), ("edit", "Edit"), ("delete", "Delete")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// Phase 4.7: the shell template must render instantly with the
    /// copy-URL banner and the filter input, and must NOT render the row
    /// table itself anymore (moved to `table.html`).
    #[test]
    fn list_metadata_shell_renders_banner_and_filter_without_table() {
        // Uses the real `I18n` loader (same as `render_form_html` below)
        // rather than a hand-picked translate map, since the shell extends
        // `admin/layout_admin.html`, which itself indexes several
        // `translate[...]` keys not otherwise relevant to this test.
        let i18n = I18n::new();
        let translate = i18n.get_all_translations("es-AR");
        let base = BaseTemplateData { is_auth: true, is_admin: true, translate, version: "0.0.0-test" };
        let user = test_user();
        let template = ListMetadataTemplate { current_user: &user, base };
        let html = template.render().expect("list.html shell must render without panicking or erroring");

        assert!(html.contains("id=\"url-records\""), "copy-URL banner must survive the shell split");
        assert!(html.contains("id=\"filter\""), "shell must include the filter input");
        assert!(html.contains("hx-get=\"/admin/metadata/table\""), "shell must HTMX-load the table fragment");
        assert!(!html.contains("No hay capas publicadas"), "row table content must have moved to table.html");
    }

    /// Phase 4.7: the table fragment renders the empty state when there are
    /// no matching rows.
    #[test]
    fn table_metadata_fragment_renders_empty_state_for_no_rows() {
        let template = MetadataTableTemplate { rows: vec![], translate: full_table_translate() };
        let html = template.render().expect("table.html fragment must render without panicking or erroring");
        assert!(html.contains("No hay capas publicadas"));
    }

    /// Phase 4.7: the table fragment renders a row with an "edit" action
    /// when a record exists, and reflects the layer's category/name/alias.
    #[test]
    fn table_metadata_fragment_renders_row_with_record() {
        let rows = vec![MetadataRow { layer: test_layer("layer-1"), has_record: true }];
        let template = MetadataTableTemplate { rows, translate: full_table_translate() };
        let html = template.render().expect("table.html fragment must render without panicking or erroring");
        assert!(html.contains("Parcels"));
        assert!(html.contains("/admin/metadata/edit/layer-1"));
    }

    /// Renders the real `admin/metadata/form.html` template with the exact
    /// production `I18n` loader for `lang` (same `locales/*.ftl` embed
    /// `I18n::new()` uses at runtime), so this test catches the same
    /// `HashMap::index` panic a live server would hit if Phase 2.5.3 ever
    /// left a locale gap (Key Learning #10).
    fn render_form_html(lang: &str) -> String {
        let i18n = I18n::new();
        let translate = i18n.get_all_translations(lang);
        let topic_categories = codelist_options(TOPIC_CATEGORY_CODES, topic_category_translate_key, &translate);
        let progress_codes = codelist_options(PROGRESS_CODES, progress_code_translate_key, &translate);
        let base = BaseTemplateData { is_auth: true, is_admin: true, translate, version: "0.0.0-test" };
        let template = MetadataFormTemplate {
            layer: test_layer("layer-1"),
            record: blank_record("layer-1"),
            is_new: true,
            projection: "EPSG:4326".to_string(),
            topic_categories,
            progress_codes,
            protocols: known_protocols(),
            base,
        };
        template.render().expect("form.html must render without panicking or erroring")
    }

    /// Spec/Phase 2.5.7: the topic_category and status selects must show
    /// translated (non-blank) labels for `es-AR`, the fallback/default
    /// locale — not blank text and not a render-time panic.
    #[test]
    fn form_renders_translated_codelist_labels_for_es_ar() {
        let html = render_form_html("es-AR");
        assert!(html.contains("Límites"), "topic-category-boundaries must render translated in es-AR");
        assert!(html.contains("En curso"), "progress-code-onGoing must render translated in es-AR");
    }

    /// Same as above for `en-US`.
    #[test]
    fn form_renders_translated_codelist_labels_for_en_us() {
        let html = render_form_html("en-US");
        assert!(html.contains("Boundaries"), "topic-category-boundaries must render translated in en-US");
        assert!(html.contains("On Going"), "progress-code-onGoing must render translated in en-US");
    }
}
