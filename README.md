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

## Getting Started

Check out the **[MVT Server Tutorial](TUTORIAL.md)** for a complete step-by-step guide on:
- Requirements & Installation
- Configuration (Environment Variables & Arguments)
- Managing Layers & Administration UI
- Consuming Tiles in QGIS, MapLibre, OpenLayers, and Leaflet
- Advanced usage (Styles, Legends, Sprites, Glyphs)

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
