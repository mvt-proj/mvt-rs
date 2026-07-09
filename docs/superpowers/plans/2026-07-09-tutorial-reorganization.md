# TUTORIAL.md Reorganization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure and rewrite `TUTORIAL.md` into a linear user journey (install → configure → publish → consume → style → advanced topics), per the approved spec `docs/superpowers/specs/2026-07-09-tutorial-reorganization-design.md`.

**Architecture:** Task 1 performs pure block moves/deletions to establish the final section order without touching prose (safe against content loss). Tasks 2–7 then rewrite each section in place, top to bottom. Task 7 regenerates the ToC last and runs link/anchor verification.

**Tech Stack:** Markdown only. Python 3 one-off script for anchor/link verification.

## Global Constraints

- The tutorial stays **English-only**, in a **single file** `TUTORIAL.md`.
- Standard port everywhere is **5887** (code default; the old `5800` references are bugs). After Task 7, `grep -n 5800 TUTORIAL.md` must return nothing.
- **All** existing screenshot links (`user-attachments` URLs) are removed; each planned screenshot location gets an HTML comment placeholder of the exact form `<!-- screenshot: <description> -->`. After Task 7, `grep -c 'user-attachments' TUTORIAL.md` must return 0 and `grep -c 'screenshot:' TUTORIAL.md` must return 10.
- Facts must match the code: binary is `target/release/mvt-server`, config default path `config/config.yaml`, `database.sqlite_path` is joined to `paths.config` (so the value is `"mvtrs.db"`, NOT `"config/mvtrs.db"`), per-layer `Cache` field = `max_cache_age` in **seconds**, `0` = never expires, editing a layer clears its tile cache (`src/html/admin/catalog.rs:274`).
- Every task ends with a commit whose message ends with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- Do not modify any file other than `TUTORIAL.md` (plan checkboxes aside).

---

### Task 1: Restructure — reorder blocks, no prose changes

**Files:**
- Modify: `TUTORIAL.md`

**Interfaces:**
- Produces: the final section ORDER. Later tasks rewrite sections in place and rely on this order.

Operations (pure moves/deletes; keep prose byte-identical except where stated):

- [ ] **Step 1: Delete the load balancer subsection**

Delete everything from the heading `#### Setting Up a Load Balancer with Nginx` up to (not including) `## First Use & Authentication`. This content is superseded by `docs/clustering.md`.

- [ ] **Step 2: Move the Nginx block to the end**

Cut the block from `### Server with Nginx` up to the point where Step 1's deletion started (i.e. the intro line and the nginx `server { ... }` config). Re-insert it immediately **before** `## Monitoring and Metrics`, wrapped under a new section heading, exactly:

```markdown
## Production Deployment

### Server with Nginx
<the moved intro line + nginx code block, unchanged>
```

- [ ] **Step 3: Move Filtering out of Consuming Services**

Cut the block from `### Filtering` up to (not including) `### QGIS`. Re-insert it immediately **before** the new `## Production Deployment` heading, changing only the first heading from `### Filtering` to `## Advanced Filtering` (subheadings stay as they are for now).

- [ ] **Step 4: Group the appearance sections under Styling**

Currently the order is: `## Serving Styles`, `## Serving Legends`, `## Serving Glyphs and Sprites in MVT Server`. Rearrange to:

```markdown
## Styling

### Serving Styles
<existing "Serving Styles" body, without its old "### Introduction" heading>

### Sprites
<existing "Directory Structure" + "Serving Sprites" + "Creating Custom Sprites with Spreet" content>

### Glyphs
<existing "Serving Glyphs" / "Creating Glyphs for MVT Server" content>

### Legends
<existing "Serving Legends" body, without its old "### Introduction" heading>
```

Demote the moved sub-headings one level where needed so nesting stays consistent (e.g. `##### 1. Setting Up the Project` under `### Glyphs` becomes `#### 1. Setting Up the Project`; apply the same one-level demotion to everything that was under `### Serving Glyphs`). Drop the old intro paragraph of "Serving Glyphs and Sprites in MVT Server" (`In MVT Server, sprites and glyphs are essential…`) — Task 5 writes a new umbrella intro.

- [ ] **Step 5: Verify the resulting outline**

Run: `grep -n "^## " TUTORIAL.md`
Expected order:

```
## Typical Workflow
## Table of Contents
## Requirements
## Installation / Compilation
## Configuration
## First Use & Authentication
## Serving a data layer
## Consuming Services
## Styling
## Advanced Filtering
## Production Deployment
## Monitoring and Metrics
```

(`## Typical Workflow` is part of the intro and stays.)

- [ ] **Step 6: Commit**

```bash
git add TUTORIAL.md
git commit -m "docs(tutorial): reorder sections into user-journey structure (moves only)"
```

---

### Task 2: Rewrite front matter — Requirements, Installation, Configuration, First Run & Login

**Files:**
- Modify: `TUTORIAL.md`

**Interfaces:**
- Consumes: section order from Task 1.
- Produces: headings `## Requirements`, `## Installation`, `## Configuration`, `### Loading priority`, `## First Run & Login` — Task 7's ToC links to these exact headings.

- [ ] **Step 1: Replace the Requirements section**

Replace everything from `## Requirements` up to (not including) `## Installation / Compilation` with:

```markdown
## Requirements

- An operating system supported by Rust: Linux, FreeBSD, macOS or Windows.
- Access to a PostgreSQL server with PostGIS 3.0.0 or higher, local or remote. The geographic layers you publish will be read from here.
- A free port for the server (default: `5887`).
```

- [ ] **Step 2: Replace the Installation section**

Replace everything from `## Installation / Compilation` up to (not including) `## Configuration` with:

````markdown
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
````

- [ ] **Step 3: Replace the Configuration section**

Replace everything from `## Configuration` up to (not including) `## First Use & Authentication` with:

````markdown
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
2. Environment variable: `MVT_CONFIG_PATH=/path/to/config.yaml`
3. Default path: `config/config.yaml` (relative to the working directory)

Individual values are resolved as: CLI args > YAML file > `MVT_*` environment variables > defaults.

> **Upgrading from a version older than 0.18.0?** The `.env` file is no longer supported. Move its values into `config.yaml` using the structure above.
````

- [ ] **Step 4: Replace First Use & Authentication (up to the Admin Panel subsection)**

Replace everything from `## First Use & Authentication` up to (not including) `### MVT Server Administration Panel` with:

````markdown
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
````

- [ ] **Step 5: Verify**

Run: `grep -n "5800\|config/mvtrs.db" TUTORIAL.md | head`
Expected: no matches within the sections just rewritten (matches further down are fixed in later tasks).

- [ ] **Step 6: Commit**

```bash
git add TUTORIAL.md
git commit -m "docs(tutorial): rewrite requirements, installation, configuration and first-run sections"
```

---

### Task 3: Rewrite The Admin Panel + Publishing Your First Layer

**Files:**
- Modify: `TUTORIAL.md`

**Interfaces:**
- Produces: headings `## The Admin Panel`, `## Publishing Your First Layer`, `### Testing the Layer` (ToC targets), and screenshot placeholders 3–5.

- [ ] **Step 1: Replace the Admin Panel block**

Replace everything from `### MVT Server Administration Panel` up to (not including) `## Serving a data layer` with:

```markdown
## The Admin Panel

The administration panel is where the whole platform is managed. It is organized in five sections:

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
```

- [ ] **Step 2: Replace Serving a data layer**

Replace everything from `## Serving a data layer` up to (not including) `## Consuming Services` with:

```markdown
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
```

- [ ] **Step 3: Commit**

```bash
git add TUTORIAL.md
git commit -m "docs(tutorial): rewrite admin panel tour and first-layer walkthrough"
```

---

### Task 4: Rewrite Consuming Tiles

**Files:**
- Modify: `TUTORIAL.md`

**Interfaces:**
- Produces: headings `## Consuming Tiles`, `### Tile Sources`, `### TileJSON (Service Discovery)`, `### QGIS`, `### Web Clients` (ToC targets), screenshot placeholders 7–8.

- [ ] **Step 1: Rewrite the section intro and source headings**

Replace the block from `## Consuming Services` through the end of `#### 3. Retrieving Tiles by Category` (up to, not including, `#### Summary and Final Notes`) keeping the three source descriptions and example URLs **unchanged**, but with this new framing:

- `## Consuming Services` → `## Consuming Tiles`
- `### About the Sources` → `### Tile Sources`
- New intro under `## Consuming Tiles`:

```markdown
Your layer is published — now let's consume it from clients. MVT Server exposes *vector tiles* through three types of *sources*, plus a TileJSON document per layer so clients can configure themselves automatically.
```

- Keep `#### 1. Retrieving Tiles from a Single Layer`, `#### 2. Retrieving Tiles from Multiple Layers`, `#### 3. Retrieving Tiles by Category` with their URLs and notes verbatim.

- [ ] **Step 2: Tighten the summary block**

Replace from `#### Summary and Final Notes` up to (not including) `### TileJSON (Service Discovery)` with:

```markdown
#### Summary

| Source Type | Base Route | Example |
|------------|-----------|---------|
| **Single layer** | `/services/tiles/{layer}/{z}/{x}/{y}.pbf` | `/services/tiles/rivers/12/2345/3210.pbf` |
| **Multiple layers** | `/services/tiles/multi/{layers}/{z}/{x}/{y}.pbf` | `/services/tiles/multi/rivers,roads/12/2345/3210.pbf` |
| **By category** | `/services/tiles/category/{category}/{z}/{x}/{y}.pbf` | `/services/tiles/category/hydrography/12/2345/3210.pbf` |

Notes:

- Each layer within a composite tile follows its own rules regarding visibility, publishing and caching.
- Composition is performed at the server level (leveraging the built-in cache) rather than in the database.
```

- [ ] **Step 3: Keep TileJSON verbatim**

The `### TileJSON (Service Discovery)` block stays unchanged (it is recent and accurate).

- [ ] **Step 4: Rewrite the QGIS steps**

Replace from `### QGIS` up to (not including) `### Web Clients` with (the blockquote note is the existing one, kept verbatim):

```markdown
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
```

- [ ] **Step 5: Keep Web Clients, verify example links**

The `### Web Clients` block (MapLibre GL JS, OpenLayers, Leaflet) stays as is — only remove the stray blank lines (3+ consecutive) if present.

Run: `ls examples/maplibre.html examples/openlayers.html examples/leaflet.html`
Expected: all three files listed.

- [ ] **Step 6: Commit**

```bash
git add TUTORIAL.md
git commit -m "docs(tutorial): rewrite tile consumption section (sources, QGIS, web clients)"
```

---

### Task 5: Rewrite Styling (styles, sprites, glyphs, legends)

**Files:**
- Modify: `TUTORIAL.md`

**Interfaces:**
- Consumes: `## Styling` umbrella created in Task 1 Step 4.
- Produces: headings `### Serving Styles`, `### Sprites`, `### Glyphs`, `### Legends` (ToC targets), screenshot placeholders 6 and 10.

- [ ] **Step 1: Write the umbrella intro + Serving Styles**

Replace from `## Styling` up to (not including) `### Sprites` with:

```markdown
## Styling

So far the map shows raw geometry. This section covers everything related to how it looks: styles, the sprites and glyphs styles reference, and legends generated from them.

### Serving Styles

MVT Server serves styles that define how vector tiles are rendered. They can be consumed in two ways:

1. **In QGIS:** styles are applied at the layer level, specifying colors, labels, symbols and color scales.
2. **In MapLibre:** styles define a complete "project", including sources, layers, metadata, sprites, glyphs, zoom levels and map center. See the [MapLibre Style Specification](https://maplibre.org/maplibre-style-spec/).

Styles are created and published from the **Styles** section of the admin panel.

<!-- screenshot: Styles list or style editor -->
```

- [ ] **Step 2: Sprites — keep content, add lead-in**

Under `### Sprites`, keep the existing directory structure, serving URLs and Spreet content unchanged, preceded by this lead-in paragraph:

```markdown
Sprites bundle the icons a style uses into a single image plus a JSON index. Your assets should be organized as follows under `paths.assets`:
```

- [ ] **Step 3: Glyphs — port fix**

Under `### Glyphs`, keep the fontnik walkthrough unchanged except:
- In the MapLibre glyphs config snippet, change `http://127.0.0.1:5800/services/glyphs/...` to `http://127.0.0.1:5887/services/glyphs/...`.

- [ ] **Step 4: Legends — rewrite**

Replace the `### Legends` block with:

```markdown
### Legends

MVT Server can serve legends generated from published styles, using the [maplibre-legend](https://github.com/mvt-proj/maplibre-legend) library, part of the MVT Server ecosystem. The legend service is particularly useful for integration with data visualization software.

You can request:

- Individual legends by passing the layer ID
- Combined legends
- Legends with or without titles
- Legends that include or exclude raster layers

<!-- screenshot: legends output, individual and combined -->

**More documentation: coming soon**
```

- [ ] **Step 5: Verify no old images remain in Styling**

Run: `sed -n '/^## Styling/,/^## Advanced Filtering/p' TUTORIAL.md | grep -c user-attachments`
Expected: `0`

- [ ] **Step 6: Commit**

```bash
git add TUTORIAL.md
git commit -m "docs(tutorial): unify styles, sprites, glyphs and legends under Styling"
```

---

### Task 6: Rewrite Advanced Filtering + add Caching

**Files:**
- Modify: `TUTORIAL.md`

**Interfaces:**
- Produces: headings `## Advanced Filtering`, `## Caching` (ToC targets).

- [ ] **Step 1: Adjust the Advanced Filtering intro and add the plugins pointer**

Under `## Advanced Filtering` (moved in Task 1), keep the operator/logical-mode tables, example URLs, admin-defined `filter`, "Query Parameter Freedom" and "Summary" blocks unchanged, with two edits:

Replace the two intro paragraphs (from "MVT Server supports advanced filtering…" through "…without modifying the backend or exposing database logic.") with:

```markdown
Beyond serving whole layers, MVT Server supports filtering directly from the source URL using query parameters. Filters are translated into SQL `WHERE` clauses dynamically, which makes it possible to display different subsets of data depending on the user query — without modifying the backend or exposing database logic.
```

After the `#### Summary` block (end of the filtering section), append:

```markdown
#### Programmable filtering (plugins)

Beyond query parameters, MVT Server supports Lua plugins that can inspect each tile request (user, groups, zoom, query string) and inject additional SQL filters — useful for access control and row-level security. See [docs/plugins.md](docs/plugins.md).
```

- [ ] **Step 2: Insert the Caching section**

Insert immediately before `## Production Deployment`:

````markdown
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
````

- [ ] **Step 3: Commit**

```bash
git add TUTORIAL.md
git commit -m "docs(tutorial): rewrite filtering intro, add plugins pointer and caching section"
```

---

### Task 7: Production Deployment, Monitoring, ToC and final verification

**Files:**
- Modify: `TUTORIAL.md`

**Interfaces:**
- Consumes: all final headings from Tasks 2–6.

- [ ] **Step 1: Rewrite Production Deployment**

Replace from `## Production Deployment` up to (not including) `## Monitoring and Metrics` with:

````markdown
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
````

- [ ] **Step 2: Monitoring — placeholder swap and light edits**

In `## Monitoring and Metrics`:
- Replace the `<img ...user-attachments...>` tag with `<!-- screenshot: monitoring dashboard -->`.
- Change "MVT-RS includes" to "MVT Server includes" (consistent naming).
- Everything else stays.

- [ ] **Step 3: Regenerate the Table of Contents**

Replace the current `## Table of Contents` block (list only, keep the heading and the trailing `---`) with:

```markdown
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
```

- [ ] **Step 4: Run the verification script**

Write to `/tmp/claude-1000/-home-jose-trabajos-mvt-proj-mvt-rs/70a91407-a8a0-4432-9b7b-ebe3204e2bf7/scratchpad/check_tutorial.py`:

```python
import re, sys, os

text = open("TUTORIAL.md").read()

def anchor(h):
    # GitHub slugger: strip punctuation, then EACH space becomes a hyphen
    # ("First Run & Login" -> "first-run--login")
    a = re.sub(r"[^\w\s-]", "", h.strip().lower())
    return re.sub(r"\s", "-", a)

counts = {}
valid = set()
for h in re.findall(r"^#{1,6}\s+(.*)$", text, re.M):
    a = anchor(h)
    n = counts.get(a, 0)
    valid.add(a if n == 0 else f"{a}-{n}")
    counts[a] = n + 1

errors = []
for m in re.finditer(r"\]\(#([^)]+)\)", text):
    if m.group(1) not in valid:
        errors.append(f"broken anchor: #{m.group(1)}")
for m in re.finditer(r"\]\((?!https?://|#|mailto:)([^)]+)\)", text):
    path = m.group(1).split("#")[0]
    if path and not os.path.exists(path):
        errors.append(f"missing file: {path}")
if "5800" in text:
    errors.append("stale port 5800 present")
if "user-attachments" in text:
    errors.append("old screenshot link present")
n_shots = len(re.findall(r"<!-- screenshot:", text))
if n_shots != 10:
    errors.append(f"expected 10 screenshot placeholders, found {n_shots}")

print("\n".join(errors) or "OK")
sys.exit(1 if errors else 0)
```

Run from the repo root: `python3 <scratchpad>/check_tutorial.py`
Expected output: `OK`

If it reports errors, fix them in `TUTORIAL.md` and re-run until `OK`.

- [ ] **Step 5: Full read-through**

Read the final `TUTORIAL.md` top to bottom and check: heading levels are consistent (no `####` directly under `##`), no duplicated paragraphs, no leftover `### Introduction` headings, every section flows into the next.

- [ ] **Step 6: Commit**

```bash
git add TUTORIAL.md
git commit -m "docs(tutorial): production deployment section, regenerated ToC and final link check"
```

---

## Screenshot placeholder inventory (must total 10)

| # | Placeholder text contains | Section |
|---|---------------------------|---------|
| 1 | login screen | First Run & Login |
| 2 | home / main panel after login | First Run & Login |
| 3 | Catalog list with per-layer buttons | The Admin Panel |
| 4 | Add Layer form | Publishing Your First Layer |
| 5 | Map view of a published layer | Testing the Layer |
| 6 | Styles list or style editor | Styling → Serving Styles |
| 7 | QGIS New Generic Connection dialog | Consuming Tiles → QGIS |
| 8 | QGIS with the layer rendered | Consuming Tiles → QGIS |
| 9 | monitoring dashboard | Monitoring and Metrics |
| 10 | legends output | Styling → Legends |
