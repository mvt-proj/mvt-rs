# MVT Server

## One Platform. Every Cartographic Resource.

*An Open Source Cartographic Publishing Platform*

<div align="center">
  <img src="https://github.com/user-attachments/assets/f7726fd2-bd84-463b-8389-44d6a43fcef5" width="40%" />
</div>

**MVT Server** turns PostGIS data into complete, production-ready cartographic services — not just vector tiles.

Layers, maps, styles, legends, glyphs and sprites are all published, organized and operated from one place: a single Rust binary with a modern web administration interface behind it. No hand-edited config files, no juggling separate tools for each resource.

---

## What Can MVT Server Publish?

| Resource | Description |
|---|---|
| 🗺 Vector Tile Layers | Publish PostGIS tables and views as MVT services |
| 🧭 TileJSON | TileJSON 3.0.0 document per layer plus a discovery index, for zero-config client setup |
| 🌍 Maps | Complete MapLibre maps composed from multiple layers |
| 🎨 Layer Styles | Reusable styles for individual layers |
| 📖 Legends | Dynamic legends |
| 🔤 Glyphs | Font hosting |
| 🎯 Sprites | Icon hosting |

Every resource above is managed through the same web interface — created, versioned and served without touching a config file.

---

## See It In Action

Two clips, one continuous workflow — from raw PostGIS table to a styled map in QGIS, without touching a config file.

### 1. Publish a layer and consume it as a vector tile service

Publish a PostGIS table as an MVT layer from the admin UI, then connect to it live from QGIS as a Vector Tiles source.



https://github.com/user-attachments/assets/cb4071d2-39fa-4ccb-99a2-d97bad72e599



### 2. Publish a style and apply it to that layer

Create a MapLibre style from the admin UI and attach it to the layer published in step 1 — same layer, now with cartography.



https://github.com/user-attachments/assets/220999be-fb03-4c8d-85c0-b520a4037eb7



---

## Deployment

MVT Server supports both **Standalone** and **Cluster** deployments.

### Standalone
- Single executable
- Local cache or shared Redis cache
- Simple installation

### Cluster
- Multiple instances
- Shared PostgreSQL/PostGIS
- Shared Redis cache
- Load balancer support

See [docs/clustering.md](docs/clustering.md).

---

## Project Philosophy

Publishing vector maps should be as easy as publishing a web application.

The open source geospatial ecosystem already provides outstanding tools for serving vector tiles, implementing OGC standards and exposing geospatial APIs. MVT Server focuses on a different challenge:

> **One platform to publish, manage and operate every cartographic resource — directly from PostGIS.**

That's the idea behind the slogan: it's not about adding one more tile server to the ecosystem, it's about giving every resource a map needs (layers, styles, legends, glyphs, sprites) a single home, with a web interface instead of hand-edited config files.

---

## The Open Source Geospatial Ecosystem

The geospatial ecosystem is rich with excellent open source software. Every project has its own philosophy and strengths, and many of them complement each other rather than compete.

MVT Server is designed to integrate naturally with technologies such as:

- PostGIS
- MapLibre
- QGIS
- OpenLayers
- Leaflet
- Redis
- Nginx
- Prometheus

Its goal is not to replace existing tools, but to simplify the publication and administration of vector map services.

---

## Platform Capabilities

Beyond *what* MVT Server publishes (see the table above), this is *how* it operates as a platform:

### Sources & Composition

- Multiple PostgreSQL databases.
- Single-layer, multi-layer and category-based sources.
- Layer composition.

### Administration

- Modern web administration interface.
- Layer catalog.
- Categories.
- User and group management.
- Authentication using JWT or Basic Auth.

### Infrastructure

- Redis or disk cache.
- Layer-level cache control.
- Monitoring dashboard.
- Prometheus metrics.
- Lua plugin system.
- Built with Rust for performance and reliability.

---

## Performance Tips

- Enable gzip for vector tiles.
- Configure cache per layer.
- Use Redis when running multiple instances.

---

## Getting Started

```sh
git clone https://github.com/mvt-proj/mvt-rs.git
cd mvt-rs
cargo build --release
./target/release/mvt-rs
```

Then open the web administration interface at `http://localhost:<port>` *(confirm/replace with the actual default port)* to connect your PostGIS database and publish your first layer.

See [TUTORIAL.md](TUTORIAL.md) for full installation, configuration, publishing layers, MapLibre styles, QGIS integration, monitoring and clustering instructions.

---

## License

MVT Server is licensed under the [BSD-3-Clause license](https://github.com/mvt-proj/mvt-rs#BSD-3-Clause-1-ov-file).

---

## Support

If MVT Server helps your organization, consider supporting the project.

❤️ Thank you for helping keep the project active.
