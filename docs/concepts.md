🌐 **English** | [Español](concepts.es.md)

# MVT Server Concepts

MVT Server organizes everything it publishes around five ideas: **Database → Layer →
Style → Map → Client**. This page gives a quick mental model before you dive into
[TUTORIAL.md](../TUTORIAL.md) for the step-by-step instructions.

## Database

A **Database** is a named PostgreSQL/PostGIS connection registered in
`postgres_databases` (`config.yaml` or `MVT_POSTGRES_DATABASES__*` env vars). Most
setups only need one, called `default`, but you can register several — useful when
your geographic data lives across more than one PostGIS instance. Every layer you
publish is tied to exactly one database.

## Layer

A **Layer** is a PostGIS table or view published as a vector tile service. Publishing
is explicit and fine-grained: you choose which fields travel in the tile and in what
order, an optional SQL filter, a zoom range, a per-layer cache policy, a record limit,
and which user groups can access it. Nothing is exposed until you publish it from the
admin UI. Layers are organized into categories, and several layers can be combined
into multi-layer or category-based tile requests.

## Style

A **Style** is a MapLibre style document — the cartography (colors, symbols, zoom
behavior) applied to one or more layers. Styles reference their data sources with a
`mvt://` placeholder instead of a real, environment-bound URL, so the same style
works unchanged across environments; MVT Server rewrites those placeholders to
absolute tile/sprite/glyph URLs only when the style is actually served.

## Map

A **Map** is a Style rendered as a complete, interactive MapLibre map — what you get
when you open `/mapview/{style_id}` in a browser. Because a style can list several
layers as sources, a single map can compose multiple layers into one cartographic
product, ready to share or embed as-is.

## Client

A **Client** is whatever consumes what MVT Server publishes: QGIS, a MapLibre GL JS
or OpenLayers web app, Leaflet, or any OGC-aware tool. Clients typically start from
TileJSON (for zero-config layer discovery) or a style URL, and from there fetch tiles,
sprites, glyphs and legends directly — no manual URL construction required.

---

Putting it together: a **database** holds the raw geometry, a **layer** publishes a
piece of it under your control, a **style** gives one or more layers a cartographic
treatment, a **map** is that style rendered as a finished product, and a **client**
is whoever loads it. See the workflow diagram at the top of
[TUTORIAL.md](../TUTORIAL.md) for the same flow end to end.
