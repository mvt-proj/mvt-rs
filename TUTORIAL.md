# MVT Server Tutorial

MVT Server is not just a vector tile server.

It is an open source platform designed to publish vector maps directly from PostGIS. Through a web administration interface you can publish layers, organize them into catalogs and categories, manage users and permissions, configure MapLibre styles, serve legends, sprites and glyphs, monitor the platform and expose production-ready vector tile services without relying on complex configuration files.

## Typical Workflow

```text
PostGIS
    │
    ▼
MVT Server Administration
    │
    ├── Publish layers
    ├── Organize categories
    ├── Configure permissions
    ├── Manage MapLibre styles
    ├── Serve legends
    ├── Manage cache
    │
    ▼
Vector Tile Services
    │
    ├── MapLibre
    ├── QGIS
    ├── OpenLayers
    └── Leaflet
```

## Table of Contents
1. [Requirements](#requirements)
2. [Installation](#installation)
3. [Configuration](#configuration)
4. [First Run & Login](#first-run--login)
5. [The Admin Panel](#the-admin-panel)
6. [Publishing Your First Layer](#publishing-your-first-layer)
7. [Consuming Tiles](#consuming-tiles)
   - [Tile Sources](#tile-sources)
   - [TileJSON (Service Discovery)](#tilejson-service-discovery)
   - [QGIS](#qgis)
   - [Web Clients](#web-clients)
8. [Styling](#styling)
   - [Serving Styles](#serving-styles)
   - [Sprites](#sprites)
   - [Glyphs](#glyphs)
   - [Legends](#legends)
9. [Advanced Filtering](#advanced-filtering)
10. [Caching](#caching)
11. [Production Deployment](#production-deployment)
12. [Monitoring and Metrics](#monitoring-and-metrics)
---

## Requirements

- An operating system supported by Rust: Linux, FreeBSD, macOS or Windows.
- Access to a PostgreSQL server with PostGIS 3.0.0 or higher, local or remote. The geographic layers you publish will be read from here.
- A free port for the server (default: `5887`).

## Installation

For now, the only option is to download the code and compile it manually; binaries for different operating systems will be provided in the future. To compile the server, make sure [Rust is installed](https://www.rust-lang.org/tools/install) on your system.

```sh
# Clone the repository
git clone https://github.com/mvt-proj/mvt-rs.git
cd mvt-rs

# Compile for production
cargo build --release
```

The binary is generated at `target/release/mvt-server`. You can move it anywhere you like — just make sure it can find its configuration file (next section).

> **Prefer containers?** The repository ships a complete Docker setup (MVT Server + PostGIS + Redis) in [`docker-example/`](docker-example/DOCKER_README.md).

## Configuration

MVT Server reads its settings from a single `config.yaml` file. A fully commented reference is available at [`config.example.yaml`](config.example.yaml); copy it to `config/config.yaml` and adjust the values.

A minimal working configuration looks like this:

```yaml
server:
  host: "0.0.0.0"
  port: 5887

# At least one entry named "default" is required.
postgres_databases:
  pool_min: 2
  pool_max: 5
  default: "postgres://user:password@host:5432/database"
  # foo: "postgres://user:password@host:5432/database_foo"

database:
  sqlite_path: "mvtrs.db"
  # redis_url: "redis://localhost:6379"   # omit to use the disk cache

# Both secrets must be at least 32 characters long.
security:
  jwt_secret: "change-me-to-a-random-secret-at-least-32-chars-long"
  session_secret: "change-me-to-another-random-secret-at-least-32-chars"

paths:
  config: "config"
  cache: "cache"
  assets: "map_assets"
```

Some notes:

- `postgres_databases` can hold several named connections; each layer chooses which one it reads from. The `default` entry is mandatory.
- `database.sqlite_path` is the internal SQLite file where MVT Server stores its own configuration (users, groups, catalog, styles). The path is relative to `paths.config` and the file is created automatically on first run.
- `database.redis_url` switches the tile cache from disk to Redis — see [Caching](#caching).
- Every setting can also be provided as an environment variable with the `MVT_` prefix and `__` as sub-key separator, e.g. `MVT_SERVER__PORT=5887`.
- When running behind a proxy or load balancer, set `server.public_url` so absolute URLs (e.g. in TileJSON responses) use your public domain.

### Loading priority

The server looks for its configuration file in this order (highest to lowest):

1. Command line argument: `--config /path/to/config.yaml`
2. Default path: `config/config.yaml` (relative to the working directory)

Individual values are resolved as: CLI args > `MVT_*` environment variables > YAML file > defaults.

> **Upgrading from a version older than 0.18.0?** The `.env` file is no longer supported. Move its values into `config.yaml` using the structure above.

## First Run & Login

Start the server:

```sh
./target/release/mvt-server --config config/config.yaml
```

On the first run, MVT Server initializes everything it needs: it creates its internal SQLite database and an initial administrator account with the following credentials:

- Email: **admin@example.com**
- Password: **admin**

Open `http://localhost:5887` in your browser (or the corresponding domain if the server is hosted remotely) and log in.

<!-- screenshot: login screen -->

> **Important:** change the default password immediately after your first login. Leaving it as `admin` exposes your server and data to unauthorized access.

After logging in you land on the home page, from which the administration panel is reached:

<!-- screenshot: home / main panel after login -->

## The Admin Panel

The administration panel is where the whole platform is managed. It is organized in five main sections:

### Groups (User Roles)

Groups define roles with different levels of access. Create groups and assign them permissions to control who can perform administrative tasks, publish layers, or create styles. Layers can also be restricted so that only members of certain groups can consume them.

### Users

Create and manage user accounts, and assign each user to one or more groups. Only users belonging to the "admin" group can perform administrative tasks such as managing users, groups, categories, the catalog, and styles.

### Categories

Categories act as namespaces that organize layers and styles logically. They also form part of every tile URL (`category:layer_name`), and they are especially useful when working with a large number of layers.

### Catalog (Layer Publishing)

The central section of the panel: here you declare the geographic layers to publish as vector tiles — their data source, fields, zoom range, cache policy and access permissions. The next section walks through it.

<!-- screenshot: Catalog list with per-layer buttons (Map, cache, edit) -->

### Styles

Define and manage rendering styles following the [MapLibre Style Specification](https://maplibre.org/maplibre-style-spec/): colors, symbols, labels, color scales. Published styles can be consumed by clients such as QGIS and MapLibre — covered in [Styling](#styling).

## Publishing Your First Layer

1. Go to the **Catalog** menu
2. Click **Add Layer**
3. Fill out the form

<!-- screenshot: Add Layer form with schema, table and fields expanded -->

The **Name** field must contain a single word, preferably lowercase. **Alias** accepts a more descriptive label.

The form lists the schemas available in the PostgreSQL database. After selecting a schema, its tables (geographic layers) are displayed; once a table is selected, its fields are shown. It is recommended to publish only the fields you actually need.

It is also advisable to configure **ZMin** and **ZMax** properly to improve performance — setting ZMin = 0 for a small locality layer is unnecessary, for example. After adding the layer, you can use the map view to find appropriate zoom values.

Most of the remaining fields can keep their default values.

When setting up the cache, consider how frequently the layer's data changes:

- **Cache** is expressed in seconds; each layer manages its own expiration independently.
- For layers that change infrequently, set **Cache = 0**: cached tiles never expire.
- A layer's cache can be cleared at any time with the corresponding button — more on this in [Caching](#caching).

### Testing the Layer

Use the **Map** button to check that the parameters entered in the form are correct and the layer is being served.

<!-- screenshot: Map view of a published layer -->

## Consuming Tiles

Your layer is published — now let's consume it from clients. MVT Server exposes *vector tiles* through three types of *sources*, plus a TileJSON document per layer so clients can configure themselves automatically.

### Tile Sources

1. Single-layer source
2. Multi-layer source
3. Category-based source

#### 1. Retrieving Tiles from a Single Layer

To get *vector tiles* from a single layer, use the following route:

**Source:**
```
http://127.0.0.1:5887/services/tiles/category:layer_name/{z}/{x}/{y}.pbf
```

---

#### 2. Retrieving Tiles from Multiple Layers

To combine multiple layers into a single *tile*, use this route:

**Source:**
```
http://127.0.0.1:5887/services/tiles/multi/category_1:layer_name_1,category_2:layer_name_2/{z}/{x}/{y}.pbf
```

🔹 *This endpoint returns a composite tile containing both `"layer_name_1"` and `"layer_name_2"` layers.*

**Notes:**
- Multiple layers can be specified using commas (`,`).
- Useful for displaying combined data in the client.

---

#### 3. Retrieving Tiles by Category

To fetch all layers that belong to a specific category, use the following route:

**Source:**
```
http://127.0.0.1:5887/services/tiles/category/category_1/{z}/{x}/{y}.pbf
```

🔹 *This endpoint returns a tile containing all layers in the `"category_1"` category.*

---

#### Summary

| Source Type | Base Route | Example |
|------------|-----------|---------|
| **Single layer** | `/services/tiles/{layer}/{z}/{x}/{y}.pbf` | `/services/tiles/rivers/12/2345/3210.pbf` |
| **Multiple layers** | `/services/tiles/multi/{layers}/{z}/{x}/{y}.pbf` | `/services/tiles/multi/rivers,roads/12/2345/3210.pbf` |
| **By category** | `/services/tiles/category/{category}/{z}/{x}/{y}.pbf` | `/services/tiles/category/hydrography/12/2345/3210.pbf` |

Notes:

- Each layer within a composite tile follows its own rules regarding visibility, publishing and caching.
- Composition is performed at the server level (leveraging the built-in cache) rather than in the database.

### TileJSON (Service Discovery)

Every published layer also exposes a [TileJSON 3.0.0](https://github.com/mapbox/tilejson-spec/tree/master/3.0.0) document, so clients (MapLibre, QGIS, OpenLayers) can discover the tile URL, zoom range, bounds, and field schema without manual configuration.

**Index of available layers:**
```
http://127.0.0.1:5887/services/tilejson
```
Returns a JSON array with `id`, `name`, `description`, and `tilejson_url` for every published layer visible to the requesting user.

**Per-layer document:**
```
http://127.0.0.1:5887/services/tilejson/category:layer_name.json
```
Returns the TileJSON document for that layer:

```json
{
  "tilejson": "3.0.0",
  "tiles": ["http://127.0.0.1:5887/services/tiles/category:layer_name/{z}/{x}/{y}.pbf"],
  "vector_layers": [
    {
      "id": "layer_name",
      "minzoom": 0,
      "maxzoom": 22,
      "fields": { "id": "int4", "name": "Column comment or type name" }
    }
  ],
  "name": "Layer alias",
  "scheme": "xyz",
  "minzoom": 0,
  "maxzoom": 22,
  "bounds": [-63.08, -31.44, -63.01, -31.39],
  "center": [-63.05, -31.42, 11.0]
}
```

**Notes:**
- `name` comes from the layer's alias (falling back to its name); `description` from the layer's description.
- Each entry in `fields` is described by its PostgreSQL column comment when one is set (`COMMENT ON COLUMN ...`), otherwise by its type name.
- Access control mirrors the tile endpoint: only published layers are served, and group-restricted layers require authentication (404 / 403 otherwise).
- Behind a proxy or load balancer, set `server.public_url` (see [Configuration](#configuration)) so the URLs in the document use your public domain.

---

### QGIS

1. In the Browser panel, right-click **Vector Tiles** and choose **New Generic Connection**
2. Give the connection a name
3. **URL**: paste the tile URL of the published layer, e.g. `http://127.0.0.1:5887/services/tiles/category:layer_name/{z}/{x}/{y}.pbf`
4. Set **Min. Zoom Level** and **Max. Zoom Level** to match the layer
5. **Style URL** can be left empty for now — styles are covered in [Styling](#styling)

> **Note:** QGIS's built-in generic connection only accepts the XYZ tile
> template (`.../{z}/{x}/{y}.pbf`), not a TileJSON URL. The layer's TileJSON
> document (`http://.../services/tilejson/category:layer_name.json`) is still
> useful here: it gives you the exact tile URL to paste, plus the Min/Max Zoom
> values for the connection dialog and the layer's field schema. Plugins such
> as the MapTiler plugin can consume TileJSON URLs directly.

<!-- screenshot: QGIS New Generic Connection dialog -->

<!-- screenshot: QGIS with the layer rendered -->

### Web Clients

This section provides examples of how to consume vector tiles from the **MVT Server** using different mapping libraries: **MapLibre GL JS**, **OpenLayers**, and **Leaflet**.

#### MapLibre GL JS
[View Example](examples/maplibre.html)

This example demonstrates how to integrate vector tiles into a **MapLibre GL JS** map. The best approach is to use **MapLibre styles**, which allow for better layer management and styling flexibility. The example loads three separate sources for polygons, lines, and points:
- **Polygons:** `public:polygons_example`
- **Lines:** `public:lines_example`
- **Points:** `public:points_example`

Alternatively, a single source can be used to load all three layers at once from:
```
http://127.0.0.1:5887/services/tiles/category/public/{z}/{x}/{y}.pbf
```

A source can also be defined from the layer's TileJSON document instead of writing the `tiles` array by hand — MapLibre picks up the tile URL, zoom range, and bounds automatically:
```js
map.addSource("polygons", {
  type: "vector",
  url: "http://127.0.0.1:5887/services/tilejson/public:polygons_example.json"
});
```

#### OpenLayers
[View Example](examples/openlayers.html)

This example illustrates how to render vector tiles using **OpenLayers**. It loads the same three sources for polygons, lines, and points while also supporting the combined source for improved efficiency.

#### Leaflet
[View Example](examples/leaflet.html)

This example showcases how to use **Leaflet** with vector tiles. Since Leaflet does not natively support vector tiles, it utilizes plugins to correctly render the data from the MVT Server.

Each example is configured to fetch tiles from:
```
http://127.0.0.1:5887/services/tiles/public:{layer}/{z}/{x}/{y}.pbf
```
where `{layer}` can be:
- `polygons_example`
- `lines_example`
- `points_example`

or use the combined source:
```
http://127.0.0.1:5887/services/tiles/category/public/{z}/{x}/{y}.pbf
```
for all three layers.

These examples provide a starting point for integrating vector tiles into your web mapping applications.

## Styling

So far the map shows raw geometry. This section covers everything related to how it looks: styles, the sprites and glyphs styles reference, and legends generated from them.

### Serving Styles

MVT Server serves styles that define how vector tiles are rendered. They can be consumed in two ways:

1. **In QGIS:** styles are applied at the layer level, specifying colors, labels, symbols and color scales.
2. **In MapLibre:** styles define a complete "project", including sources, layers, metadata, sprites, glyphs, zoom levels and map center. See the [MapLibre Style Specification](https://maplibre.org/maplibre-style-spec/).

Styles are created and published from the **Styles** section of the admin panel.

<!-- screenshot: Styles list or style editor -->

### Sprites

Sprites bundle the icons a style uses into a single image plus a JSON index. Your assets should be organized as follows under `paths.assets`:

#### Directory Structure

```
map_assets
├── glyphs
└── sprites
    ├── fa-brand
    │   ├── sprite.json
    │   └── sprite.png
    ├── fa-regular
    │   ├── sprite.json
    │   ├── sprite.png
    │   ├── sprite@2x.json
    │   └── sprite@2x.png
    ├── fa-solid
    │   ├── sprite.json
    │   └── sprite.png
    ├── maplibre
    │   ├── sprite.json
    │   ├── sprite.png
    │   ├── sprite@2x.json
    │   └── sprite@2x.png
    └── maptiler
        ├── sprite.json
        ├── sprite.png
        ├── sprite@2x.json
        └── sprite@2x.png
```

#### Serving Sprites

Sprites are served dynamically by MVT Server. Each sprite set is accessible via a URL like this:

`http://127.0.0.1:5887/services/map_assets/sprites/{sprite_name}/sprite`

For example, to use the maplibre sprite set:

`http://127.0.0.1:5887/services/map_assets/sprites/maplibre/sprite`

To configure this in your MapLibre style JSON:
```
{
  "version": 8,
  "sprite": "http://127.0.0.1:5887/services/map_assets/sprites/maplibre/sprite",
  "sources": { ... },
  "layers": [ ... ]
}
```

This tells MapLibre to fetch the sprite JSON and images from your MVT Server.

#### Creating Custom Sprites with Spreet

To create your own sprite sets, you can use [Spreet](https://github.com/flother/spreet), a simple tool for generating sprite sheets and metadata from individual images.

### Glyphs

This tutorial will guide you through the process of generating glyphs for the **MVT Server** using **fontnik**. Glyphs allow the map server to render text labels properly.

#### 1. Setting Up the Project

Create a new project directory and install `fontnik`:

```sh
$ mkdir glyphs-project
$ cd glyphs-project
$ npm install fontnik
# or using pnpm
$ pnpm install fontnik
```

#### 2. Downloading a Font

Download a font of your choice. In this example, we will use **EmblemaOne** from Google Fonts:

[Google Fonts - Emblema One](https://fonts.google.com/specimen/Emblema+One)

Extract the downloaded ZIP file and move `EmblemaOne-Regular.ttf` into the `glyphs-project` directory.

#### 3. Generating Glyphs

Create a directory to store the glyphs:

```sh
$ mkdir -p glyphs/EmblemaOne-Regular
```

Run the following commands to generate glyph files for different Unicode ranges:

```sh
$ node -e "require('fontnik').range({font: require('fs').readFileSync('EmblemaOne-Regular.ttf'), start: 0, end: 255}, (err, data) => require('fs').writeFileSync('glyphs/EmblemaOne-Regular/0-255.pbf', data))"

$ node -e "require('fontnik').range({font: require('fs').readFileSync('EmblemaOne-Regular.ttf'), start: 256, end: 511}, (err, data) => require('fs').writeFileSync('glyphs/EmblemaOne-Regular/256-511.pbf', data))"

$ node -e "require('fontnik').range({font: require('fs').readFileSync('EmblemaOne-Regular.ttf'), start: 512, end: 767}, (err, data) => require('fs').writeFileSync('glyphs/EmblemaOne-Regular/512-767.pbf', data))"
```

##### Resulting Directory Structure

After running these commands, your `glyphs` directory should have the following structure:

```
glyphs/
└── EmblemaOne-Regular/
    ├── 0-255.pbf
    ├── 256-511.pbf
    └── 512-767.pbf
```

#### 4. Deploying Glyphs to MVT Server

Move or copy the `EmblemaOne-Regular` directory into your **MVT Server's** glyphs directory:

```sh
$ mv glyphs/EmblemaOne-Regular /path/to/map_assets/glyphs/
```

MVT Server will now be able to serve the glyphs.

#### 5. Configuring MapLibre to Use the Glyphs

In your **MapLibre** style JSON, add the glyphs path in the root:

```json
{
  "glyphs": "http://127.0.0.1:5887/services/map_assets/glyphs/{fontstack}/{range}.pbf"
}
```

In the **layout** section, specify the font name where needed:

```json
"text-font": ["EmblemaOne-Regular"]
```

##### Important Note
The current version of the MVT Server supports only one font in the array. This is because the server ensures the font's existence beforehand through the administration panel.

The glyphs available on the server can be viewed from the Glyphs menu.

### Legends

MVT Server can serve legends generated from published styles, using the [maplibre-legend](https://github.com/mvt-proj/maplibre-legend) library, part of the MVT Server ecosystem. The legend service is particularly useful for integration with data visualization software.

You can request:

- Individual legends by passing the layer ID
- Combined legends
- Legends with or without titles
- Legends that include or exclude raster layers

<!-- screenshot: legends output, individual and combined -->

**More documentation: coming soon**

## Advanced Filtering

Beyond serving whole layers, MVT Server supports filtering directly from the source URL using query parameters. Filters are translated into SQL `WHERE` clauses dynamically, which makes it possible to display different subsets of data depending on the user query — without modifying the backend or exposing database logic.

---

### Filter Syntax

The filter format supports three logical modes and several SQL-like operators.

#### Operators

| Suffix        | SQL Equivalent |
|---------------|----------------|
| `__eq` (default) | `=`          |
| `__ne`         | `<>`           |
| `__gt`         | `>`            |
| `__gte`        | `>=`           |
| `__lt`         | `<`            |
| `__lte`        | `<=`           |
| `__like`       | `LIKE`         |
| `__ilike`       | `ILIKE`         |
| `__in`         | `IN` (comma-separated values) |

#### Logical Modes

| Prefix        | Logic |
|---------------|-------|
| *(none)*      | `AND` |
| `or__`        | `OR`  |
| `not__`       | `NOT` |

---

### Example URLs

```text
/services/tiles/public:states/{z}/{x}/{y}.pbf?or__name__in='FOO','BAR'&or__id__in=6,9,22,24
/services/tiles/public:vtr2024/{z}/{x}/{y}.pbf?or__vur_foo__gte=9000&or__vur_bar__gte=11160000
```

These generate WHERE clauses like:

```sql
WHERE (name = ANY(ARRAY['FOO','BAR']) OR id = ANY(ARRAY[6,9,22,24]))
```

and

```sql
WHERE (vur_foo >= $1 OR vur_bar >= $2)
```

---

### Admin-defined `filter` (static filter)

In the layer configuration panel, administrators can define a **fixed SQL filter** in the `filter` field. This filter is applied **before** any dynamic query parameters.

For example, if the admin defined:

```sql
status = 'public'
```

and the user sends:

```
?or__category__eq='roads'
```

the final SQL will be:

```sql
WHERE status = 'public' AND (category = $1)
```

---

### Query Parameter Freedom

In the current version, users are free to specify **any field** in the query string. There's no restriction on which columns can be queried. This makes the system very flexible, but it also means:

> **You should control data exposure at the layer level**, not via filters.

It might be desirable in future versions to restrict which fields are allowed in filters, but this is not currently planned or guaranteed.

---

### Summary

- Combine static (`filter`) and dynamic (query params) filters.
- Express logical conditions using the default AND, `or__`, and `not__`.
- Safely binds user input to prevent SQL injection (except `IN` currently uses inline literals).
- Compatible with QGIS, MapLibre, and web clients.

### Programmable filtering (plugins)

Beyond query parameters, MVT Server supports Lua plugins that can inspect each tile request (user, groups, zoom, query string) and inject additional SQL filters — useful for access control and row-level security. See [docs/plugins.md](docs/plugins.md).

## Caching

Generating a tile costs a database query, so MVT Server caches every tile it serves. Two backends are available:

- **Disk cache** (default): tiles are stored under the directory set in `paths.cache`. No extra services required — ideal for a single-instance setup.
- **Redis**: enabled by setting `database.redis_url` in `config.yaml`. Required when several instances run behind a load balancer, so all of them share the same cache and invalidations reach every node (see [Production Deployment](#production-deployment)).

```yaml
database:
  sqlite_path: "mvtrs.db"
  redis_url: "redis://localhost:6379"   # omit to use the disk cache
```

How long tiles live is decided per layer, with two fields of the layer form:

- **Cache** (in seconds): how long a tile is served from the cache before being regenerated. `0` means cached tiles never expire — recommended for layers that rarely change.
- **Delete cache on start**: clears the layer's cache every time the server starts.

Editing a layer automatically invalidates its cached tiles, and each layer's cache can also be cleared manually from the Catalog with its purge button.

## Production Deployment

For production use, run MVT Server behind a reverse proxy such as Nginx: it can terminate TLS and compress tiles before they leave your network.

### Nginx reverse proxy

Example configuration (`/etc/nginx/sites-available/application.conf`):

```nginx
server {
    listen 80;
    server_name yourdomain.com;

    # Enable gzip compression for vector tiles and API responses.
    # .pbf tiles compress 60-80% on average, significantly reducing bandwidth.
    gzip on;
    gzip_types application/x-protobuf application/octet-stream application/json;
    gzip_min_length 256;
    gzip_proxied any;
    gzip_vary on;

    location / {
        proxy_pass http://localhost:5887;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

Remember to set `server.public_url` in `config.yaml` so absolute URLs (e.g. in TileJSON responses) use your public domain.

### Scaling out

To distribute traffic across several MVT Server instances — load balancing, shared Redis cache, configuration synchronization between nodes — see [docs/clustering.md](docs/clustering.md). For a containerized setup with PostGIS and Redis included, see [docker-example/](docker-example/DOCKER_README.md).

## Monitoring and Metrics

MVT Server includes a built-in monitoring dashboard with real-time metrics visualization. The server exposes both a web dashboard and Prometheus-compatible metrics endpoint.

### Accessing the Dashboard

Navigate to `/admin/monitor/dashboard` to view real-time server metrics including:

- **CPU Usage**: Process CPU utilization percentage (supports FreeBSD jails via getrusage fallback)
- **Memory**: Resident memory usage in GB
- **RPS (Requests Per Second)**: Real-time request throughput
- **Latency**: Last request and average response times in milliseconds
- **Cache Performance**: Cache hits and misses per second

<!-- screenshot: monitoring dashboard -->

The dashboard updates every 5 seconds via Server-Sent Events (SSE) and displays historical data in interactive charts.

### Prometheus Metrics

All metrics are available in Prometheus format at `/api/monitor/metrics`:

```
mvt_server_process_cpu_percent
mvt_server_process_memory_bytes
mvt_server_requests_total
mvt_server_cache_hits_total
mvt_server_cache_misses_total
mvt_server_last_request_latency_seconds
mvt_server_avg_request_latency_seconds
```

These can be scraped by Prometheus or any compatible monitoring system for long-term storage and alerting.

**Note**: In restricted environments like FreeBSD jails, CPU metrics automatically fall back to `getrusage()` when `sysinfo` is unavailable.
