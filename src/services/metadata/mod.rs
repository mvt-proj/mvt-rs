//! Pure ISO 19115 / OGC API - Records domain logic for published-layer
//! metadata (Phase 2 / Work Unit 2 of the ISO 19115 metadata integration).
//!
//! This module is pure logic: no new SQLx/network calls beyond what is
//! already exposed by `db::metadata::query_extent()` and `config::metadata`
//! (Phase 1). It is not yet wired into `api::metadata` / `html::admin::metadata`
//! (Phase 3/4).

pub mod codelists;
pub mod ogc;
pub mod rules;
