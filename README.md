# MVT Server: An Open Source Vector Map Publishing Platform for PostGIS

<div align="center">
  <img src="https://github.com/user-attachments/assets/f7726fd2-bd84-463b-8389-44d6a43fcef5" width="40%" />
</div>

**MVT Server** is an open source platform for publishing vector maps directly from PostGIS.

It combines high-performance vector tile generation with a modern web administration interface for managing layers, styles, categories, authentication, caching, monitoring and map assets.

Rather than focusing only on serving vector tiles, MVT Server provides the tools required to publish, organize and operate production-ready vector map services.

---

## Project Philosophy

Publishing vector maps should be as easy as publishing a web application.

The open source geospatial ecosystem already provides outstanding tools for serving vector tiles, implementing OGC standards and exposing geospatial APIs.

MVT Server focuses on a different challenge:

> **Providing a complete platform to publish, manage and operate vector map services directly from PostGIS.**

Instead of manually editing configuration files, administrators can manage the complete publication workflow through a web interface.

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

## Key Features

### Publishing

- Publish PostGIS tables and views as vector tiles.
- Multiple PostgreSQL databases.
- Single-layer, multi-layer and category-based sources.
- Layer composition.
- MapLibre Style hosting.
- Legend server.
- Sprite and glyph hosting.

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

See the **TUTORIAL.md** for installation, configuration, publishing layers, MapLibre styles, QGIS integration, monitoring and clustering.

```sh
git clone https://github.com/mvt-proj/mvt-rs.git
cd mvt-rs
cargo build --release
```

---

## Support

If MVT Server helps your organization, consider supporting the project.

❤️ Thank you for helping keep the project active.
