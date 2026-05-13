# MVT Server: A Simple Vector Tile Server

This is a simple and high-speed vector tile server developed in Rust, using the Salvo web framework. It provides an efficient way to serve vector geospatial data over the web.

<div align="center">
  <img src="https://github.com/user-attachments/assets/f7726fd2-bd84-463b-8389-44d6a43fcef5" width="40%" />
</div>

**MVT Server** will allow you to publish any table or view with a geometry field as vector tiles. It relies on the use of the PostGIS function `ST_AsMVT`.

**MVT Server** focuses exclusively on the generation and delivery of Vector Tiles from PostGIS to maximize performance, simplicity, and ease of use.

## Key Features

- Support for multiple PostgreSQL databases simultaneously.
- Layer server, maps server (through Maplibre Style) and legends server.
- On-the-fly vector tile generation using PostgreSQL/PostGIS.
- Web-based administration for managing users, groups, layers, styles, categories, and more.
- Integrated caching with support for disk or Redis storage.
- Granular cache control at the layer level.
- Multiple source variants: Single layer, Multi-layer by composition, and Multi-layer by category.
- Built-in glyph and sprite server for custom styles.
- Layer access control via Basic Authentication or JWT.
- Monitoring and Metrics with Prometheus support.
- Extensible via Lua plugins: add custom filter logic per layer or category without recompiling.

## Performance Tips

- **Gzip compression**: Enable gzip in your reverse proxy (nginx, caddy, etc.) for `application/x-protobuf` responses. Vector tiles compress 60–80% on average. See the [nginx configuration example in the Tutorial](TUTORIAL.md#server-with-nginx).
- **Caching**: Configure `max_cache_age` per layer. Static layers (boundaries, basemaps) benefit from long or infinite cache. Real-time layers should use a short TTL. Cache can be invalidated manually per layer from the admin panel.
- **Redis**: Use Redis as the cache backend when running multiple instances behind a load balancer.

## Getting Started

Check out the **[MVT Server Tutorial](TUTORIAL.md)** for a complete step-by-step guide on:
- Requirements & Installation
- Configuration (YAML, Env vars, Arguments)
- Managing Layers & Administration UI
- Consuming Tiles in QGIS, MapLibre, OpenLayers, and Leaflet
- Advanced usage (Styles, Legends, Sprites, Glyphs)

## Configuration

MVT Server uses a hierarchical configuration system. Please see the [Tutorial](TUTORIAL.md) for detailed instructions on the new `config.yaml` structure and how to configure the server.

## Installation (Quick Start)

```sh
# Clone the repository
git clone https://github.com/mvt-proj/mvt-rs.git
cd mvt-rs

# Compile for production
cargo build --release
```

## 🙌 Support This Project

If you find **MVT Server** useful, please consider supporting its development.
Your contribution helps improve the project and keep it actively maintained.

[![Donate](https://img.shields.io/badge/Donate-PayPal-blue.svg)](https://paypal.me/josejachuf)

Thank you for your support! 💖
