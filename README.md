# MVT-RS Simple Vector Tile Server

This is a simple and high-speed vector tile server developed in Rust, utilizing the Salvo web framework. It provides an efficient way to serve vector geospatial data over the web.

Requires a PostgreSQL server with PostGIS version 3.0.0 or higher, either local or remote. It relies on the use of the PostGIS function ST_AsMVT. More information can be found at https://postgis.net/docs/en/ST_AsMVT.html.

**mvt-rs** will allow you to publish any table or view with a geometry field as vector tiles through the definition of a configuration file.


## Table of Layer Configuration Fields

Each layer intended for publication is defined as a JSON file with the fields as shown below. These files should be located in a directory named **layers** at the root of your project.

| Field                     | Meaning                                | Required | Default  | Example                |
|---------------------------|----------------------------------------|----------|----------|------------------------|
| `geometry`                | Geometry of layer ['points', 'lines', 'polygons'] | true   |          | `polygons`              |
| `name`                    | Layer name used in the URL              | true   |          | `streets`              |
| `alias`                   | Layer alias                             | true   |          | `Street Layer`         |
| `schema`                  | PostgreSQL database schema name         | false  | `public` | `my_schema`            |
| `table`                   | Table or view to serve as VT            | true   |          | `my_table`             |
| `fields`                  | List of fields to include in vector tiles | true |          | `[id, field1, field2]` |
| `filter`                  | Data filter                             | false  |          | `field1 > 100`         |
| `srid`                    | Spatial Reference Identifier             | false  | `4326`   | `3857`                 |
| `geom`                    | Geometry column in the table            | false  | `geom`   | `geom_column`          |
| `buffer`                  | Is the buffer size in tile coordinate space for geometry clippig | false | `256` | `512`                  |
| `extent`                  | Is the tile extent size in tile coordinate space                  | false | `4096` | `8192`                 |
| `zmin`                    | Minimum zoom level                       | false  | `0`      | `4`                    |
| `zmax`                    | Maximum zoom level                       | false  | `22`     | `12`                   |
| `zmax_do_not_simplify`    | Maximum zoom level for no simplification | false | `16`     | `10`                   |
| `buffer_do_not_simplify`  | Vector tile buffer size for no simplification | false | `256` | `128`                  |
| `extent_do_not_simplify`  | Vector tile extent size for no simplification | false | `4096` | `2048`                 |
| `clip_geom`               | Is a boolean to control if geometries are clipped or encoded as-is | false | `true` | `false`                |
| `delete_cache_on_start`   | Delete cache on server start            | false  | `false` | `true`                 |
| `max_cache_age`           | Maximum cache age in seconds. 0 means infinite time | false | `0` | `3600`                 |




You can customize the layer configuration by modifying values in a specific configuration file.

Example:

```json
{
  "geometry": "polygons",
  "name": "departamentos",
  "alias": "Departamentos",
  "schema": "public",
  "table": "departamentos",
  "fields": ["id", "codigo", "nombre"],
  "srid": 4326,
  "zmin": 6,
  "zmax": 14,
  "geom": "geom",
  "buffer": 256,
  "extent": 4096,
  "clip_geom": false,
  "delete_cache_on_start": true,
  "max_cache_age": 0
}
```

In this case, the URL that the service publishes will be:

http://127.0.0.1:5887/tiles/departamentos/{z}/{x}/{y}.pbf


Can you see all the published layers at:

http://127.0.0.1:5887/


## Environment Variables (.env)

The server uses environment variables for its configuration. Make sure to create a `.env` file at the root of your project with the following variables:

- `DATABASE_URL`: The connection URL for the PostgreSQL database.
  Example: `postgres://user:pass@host/db`

- `POOLSIZEMIN`: Minimum size of the connection pool. Example: `3`

- `POOLSIZEMAX`: Maximum size of the connection pool. Example: `5`

- `IPHOST`: The IP address where the server will listen. Example: `0.0.0.0`

- `PORT`: The port on which the server will run. Example: `5880`

- `DELETECACHE`:  Specifies whether to delete the cache (1 for true, 0 for false). Indicate whether you should attempt to clear the cache globally when starting the service. Also, take into consideration the delete_cache_start attribute that will be evaluated in the end.

Ensure that the `.env` file is kept secure and not shared in public repositories.


## About the Cache

There are two ways to perform caching:

    - In-memory using MokaStore through the framework itself with a duration of 30 seconds.
    - On disk following the layer's configuration. The disk cache is asynchronous, using `tokio::fs`

Regarding caching and filter application, it will only be saved when the filter is provided in the layer's configuration and will not be applied when it comes from a request to the server.

## Running

To run the server, ensure you have Rust installed on your system.

https://www.rust-lang.org/tools/install


Then, you can compile and run the project as follows:

```bash
# Clone the repository
git clone https://github.com/mvt-proj/mvt-rs.git
# Navigate to the project directory
cd mvt-rs

# Create the .env file with the required environment variables

# Compile and run the server
cargo run

# Compile for production
cargo build --release
