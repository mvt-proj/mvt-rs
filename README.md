# MVT-RS Simple Vector Tile Server

This is a simple and high-speed vector tile server developed in Rust, using the Salvo web framework. It provides an efficient way to serve vector geospatial data over the web.

Requires a PostgreSQL server with PostGIS version 3.0.0 or higher, either local or remote. It relies on the use of the PostGIS function ST_AsMVT. More information can be found at https://postgis.net/docs/en/ST_AsMVT.html.

<div align="center">
  <img src="https://github.com/mvt-proj/mvt-rs/assets/5981345/b31e1d59-2253-4406-90c9-453750c1bff2" width="40%" />
</div>

**mvt-rs** will allow you to publish any table or view with a geometry field as vector tiles through the definition in the layers' configuration.


## Table of Layer Configuration Fields

Each published layer is defined as a JSON file with the fields as shown below.

| Field                     | Meaning                                | Required | Default  | Example                |
|---------------------------|----------------------------------------|----------|----------|------------------------|
| `geometry`                | Geometry of layer ['points', 'lines', 'polygons'] | true   |          | `polygons`              |
| `name`                    | Layer name used in the URL              | true   |          | `streets`              |
| `alias`                   | Layer alias                             | true   |          | `Street Layer`         |
| `schema`                  | PostgreSQL database schema name         | true   |          | `public`            |
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
| `published`           | A layer can be declared in the configuration, but it is not published as a service | true | true | false                 |



You can customize the layer configuration by modifying values in a specific JSON configuration file or through the admin using a web form.

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
  "max_cache_age": 0,
  "published": true
}
```

In this case, the service publishes the following URL:

http://127.0.0.1:5887/tiles/departamentos/{z}/{x}/{y}.pbf


You could see all the served layers at:

http://127.0.0.1:5887/catalog


By default, the file catalog.json is in the "config" directory located at the root of your project. However, you can also specify a different location for these configuration files as an argument when the server starts. Example:

`./mvt-rs --config /usr/local/etc/mvt-rs/config`



## Environment Variables (.env)

The server uses environment variables for its configuration. Make sure to create a `.env` file at the root of your project with the following variables:

- `DBCONN`: The connection URL for the PostgreSQL database.
  Example: `postgres://user:pass@host/db`

- `POOLSIZEMIN`: Minimum size of the connection pool. Example: `3`

- `POOLSIZEMAX`: Maximum size of the connection pool. Example: `5`

- `SQLMODE`: CTE: Common Table Expression or SQ: Subquery. Default: `CTE`

- `IPHOST`: The IP address where the server will listen. Example: `0.0.0.0`

- `PORT`: The port on which the server will run. Example: `5880`

- `REDISCONN`: The connection URL for the Redis. If a Redis connection is provided, it is assumed that Redis should be used as the primary cache, overriding disk cache usage. Example: `redis://127.0.0.1:6379`

- `SALTSTRING`: User passwords are stored encrypted using Argon2. This variable is used to enhance the security of the password hashing process.

- `JWTSECRET`: It is used to create and validate JWT tokens.




Remember the `.env` file has to kept secure and not shared in public repositories.

## About the Cache

There are two ways to perform caching:

    - In-memory using MokaStore through the framework itself with a duration of 30 seconds.
    - On disk or on Redis server following the layer's configuration. The disk cache is asynchronous, using `tokio::fs`

If a Redis connection is provided, either through the environment variable REDISCONN or the --redisconn argument, it will serve as the default cache. Otherwise, disk storage will be employed.

Regarding caching and filter application, it will only be saved when the filter is provided in the layer's configuration and will not be applied when it comes from a request to the server.

By default, the cache files are stored in the "cache" directory located at the root of your project. However, you can also specify a different location for the cache directory as an argument when starting the server. Example:

`./mvt-rs --cache /tmp/cache`




## To-Do

- Something very basic has been developed to manage users and layers using templates. Additionally, an API for the admin has been started, so that it can be developed as a standalone app.


## Running

To run the server, ensure you have Rust installed on your system.

https://www.rust-lang.org/tools/install


Then, you can compile and run the project as follows:

```sh
# Clone the repository
git clone https://github.com/mvt-proj/mvt-rs.git
# Navigate to the project directory
cd mvt-rs

# Create the .env file with the required environment variables

# Compile and run the server
cargo run

# Compile for production
cargo build --release
```


## Arguments

```
Usage: mvt-rs [OPTIONS]

Options:
  -c, --config <CONFIGDIR>     Directory where config files are placed [default: config]
  -b, --cache <CACHEDIR>       Directory where cache files are placed [default: cache]
  -d, --dbconn <DBCONN>        Database connection
  -s, --sqlmode <SQLMODE>      CTE: Common Table Expression or SQ: Subquery
  -r, --redisconn <REDISCONN>  Redis connection
  -j, --jwtsecret <JWTSECRET>  JWT secret key
  -h, --help                   Print help
```
