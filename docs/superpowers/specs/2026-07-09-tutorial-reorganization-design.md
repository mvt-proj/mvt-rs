# TUTORIAL.md Reorganization — Design

**Date:** 2026-07-09
**Status:** Approved approach, pending spec review

## Problem

TUTORIAL.md grew organically: each new feature was appended without an overall
narrative. Symptoms:

- Nginx reverse proxy and load balancer configuration sit in the middle of
  basic setup (between "Configuration" and "First Use"), interrupting the flow.
- The Table of Contents does not match the actual sections ("Running the
  Application", "Environment Variables", "Command Arguments" are listed but do
  not exist as sections).
- The load balancer section duplicates content now covered in depth by
  `docs/clustering.md`.
- Repeated text (e.g. "First Use" states the default credentials twice).
- Screenshots are outdated.
- No incremental complexity: advanced topics are mixed with beginner steps.

## Goal

A single `TUTORIAL.md` (kept in English) that reads as a linear journey: a new
user starts at installation and configuration, and more complex topics are
introduced progressively as the reading advances. Reorder **and** rewrite:
fix transitions, remove duplication, and fill evident gaps using the code,
`config.example.yaml`, README, and `docs/` as sources.

## Non-goals / future work

- Splitting into multiple documents — deferred; current size (~800 lines) does
  not warrant it.
- A separate quickstart file — explicitly noted as a possible future addition,
  not part of this change.
- Rewriting `docs/clustering.md`, `docs/plugins.md`, or the README.
- Translating the tutorial (stays English-only).

## New structure ("user journey")

1. **Introduction** — what MVT Server is + the existing workflow diagram
   (current intro is good; keep with light editing).
2. **Requirements** — as today.
3. **Installation** — compile from source; brief mention of `docker-example/`
   with a link to its `DOCKER_README.md`.
4. **Configuration** — minimal working `config.yaml`, loading priority
   (CLI arg > env var > default path). The "Migrating to 0.18.0" content
   shrinks to a short note (or appendix link) rather than framing the whole
   section as a migration.
5. **First Run & Login** — default admin credentials (stated once), change
   the password, access URL.
6. **The Admin Panel** — tour of Groups, Users, Categories, Catalog, Styles.
   Remove filler prose (e.g. "MVT Server likely supports…" — it *does*
   support the MapLibre Style Spec; assert it).
7. **Publishing Your First Layer** — Catalog form walkthrough, Name/Alias,
   schema→table→fields selection, ZMin/ZMax advice, per-layer cache setting,
   testing with the Map button.
8. **Consuming Tiles** — the three source types (single, multi, category)
   with the summary table; TileJSON discovery; QGIS connection; web clients
   (MapLibre, OpenLayers, Leaflet) linking to `examples/`.
9. **Styling** — grouped because they all concern map appearance:
   serving styles (QGIS vs MapLibre usage), sprites (structure, Spreet),
   glyphs (fontnik walkthrough), legends.
10. **Advanced Filtering** — dynamic query-param filters (operators, logical
    modes, examples), admin-defined static `filter`, the data-exposure
    caveat; link to `docs/plugins.md` for programmable filtering.
11. **Caching** — new section filling a gap: disk vs Redis backends, how to
    configure each, when to prefer Redis (multi-instance), relation to the
    per-layer cache duration and the purge buttons. Content sourced from
    `config.example.yaml`, `src/cache/`, and README.
12. **Production Deployment** — nginx reverse proxy with gzip (moved here
    from the top); a short pointer to `docs/clustering.md` replaces the
    duplicated load-balancer section.
13. **Monitoring & Metrics** — dashboard + Prometheus, as today with light
    editing.

The Table of Contents is regenerated to match the final sections exactly.

## Content-level changes

- **Delete:** the "Setting Up a Load Balancer with Nginx" subsection
  (superseded by `docs/clustering.md`; replaced by a link).
- **Move:** nginx reverse proxy → §12; the current standalone "Filtering"
  block stays but becomes §10 after the basic consumption flow.
- **Rewrite:** First Use (dedupe credentials), Admin Panel descriptions
  (tighten, remove hedging), Consuming Services intro, transitions between
  all sections so each one links narratively to the previous.
- **Add:** Caching section (§11); docker-example mention (§3); cross-links
  to `docs/clustering.md` and `docs/plugins.md`.
- All facts added or asserted during rewrite must be verified against the
  code/config before being written (e.g. default port, cache config keys,
  endpoint paths).

## Screenshots

All screenshots will be recaptured from scratch — no existing image is
reused. Suggested list and placement:

| # | Screenshot | Section |
|---|-----------|---------|
| 1 | Login screen | §5 First Run & Login |
| 2 | Home / main panel after login | §5–6 |
| 3 | Catalog list with per-layer buttons (Map, cache, edit) | §6–7 |
| 4 | "Add Layer" form with schema/table/fields expanded | §7 |
| 5 | Map view of a published layer | §7 |
| 6 | Styles list or style editor | §9 |
| 7 | QGIS generic connection dialog | §8 |
| 8 | QGIS with the layer rendered | §8 |
| 9 | Monitoring dashboard | §13 |
| 10 | Legends output (individual and combined) | §9 |

All old screenshot links are removed during the rewrite. Each placement gets
a `<!-- screenshot: <description> -->` placeholder so the text can land
first; the user swaps in the new images as they are captured.

## Error handling / risks

- Risk: rewriting introduces factual errors. Mitigation: verify every
  endpoint, config key, and default against the source before asserting it.
- Risk: broken intra-document anchors after renaming sections. Mitigation:
  regenerate the ToC last and check every anchor resolves.

## Testing / verification

- Markdown renders correctly (headings hierarchy, tables, code fences).
- All ToC anchors resolve to real headings.
- All relative links (`examples/*.html`, `docs/*.md`, `docker-example/`)
  point to existing files.
- Endpoints and config snippets match the current code
  (`src/routes.rs`, `config.example.yaml`).
