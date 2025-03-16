# mvt server: A Simple Vector Tile Server

This is a simple and high-speed vector tile server developed in Rust, using the Salvo web framework. It provides an efficient way to serve vector geospatial data over the web.

<div align="center">
  <img src="https://github.com/user-attachments/assets/a7b743de-5775-4c47-b5c5-adb1378d03ef" width="40%" />
</div>

**mvt server** will allow you to publish any table or view with a geometry field as vector tiles through the definition in the layers' configuration. It will also enable the publication of styles to be used in tools like QGIS. These styles define how layers are displayed, including visualization scales, colors, labels, and other visual elements. This functionality ensures seamless integration with GIS tools, guaranteeing that vector tiles are rendered consistently and as intended across different platforms.

Requires a PostgreSQL server with PostGIS version 3.0.0 or higher, either local or remote. It relies on the use of the PostGIS function ST_AsMVT. More information can be found at https://postgis.net/docs/en/ST_AsMVT.html.

## Key Features

- On-the-fly vector tile generation using PostgreSQL/PostGIS.
- Web-based administration with user, group, layer, style, and category management.
- Integrated caching with support for disk or Redis storage.
- Granular cache control at the layer level.
- Multiple source variants:
  - **Single layer**: one layer per source.
  - **Multi-layer by composition**: combining multiple layers into a single source.
  - **Multi-layer by category**: grouping layers by thematic categories.
- Built-in glyph and sprite server for custom styles.
- Layer access control via Basic Authentication.
- Alpha-stage API for querying and managing the layer catalog.


## Tutorial

[Show tutorial](TUTORIAL.md)

## Environment Variables (.env)

### The server uses environment variables for its configuration.
### Make sure to create a `.env` file at the root of your project with the following variables:


```sh
# Database connection URL (PostgreSQL)
DBCONN=postgres://user:pass@host/db

# Connection pool size
POOLSIZEMIN=3   # Minimum size of the connection pool
POOLSIZEMAX=5   # Maximum size of the connection pool

# Server settings
IPHOST=0.0.0.0  # The IP address where the server will listen
PORT=5880       # The port on which the server will run

# Redis connection (optional, overrides disk cache if provided)
REDISCONN=redis://127.0.0.1:6379

# Security settings
JWTSECRET=supersecretjwt # Used to create and validate JWT tokens
SESSIONSECRET=supersecretsession # Secret key for session management

# Directories
CONFIG=config  # Directory path for configuration files
CACHE=cache    # Directory path for cache storage
```

Remember the `.env` file has to kept secure and not shared in public repositories.

## About the Cache

There are two ways to perform caching:

    - In-memory using MokaStore through the framework itself with a duration of 30 seconds.
    - On disk or on Redis server following the layer's configuration. The disk cache is asynchronous, using `tokio::fs`

If a Redis connection is provided, either through the environment variable REDISCONN or the --redisconn argument, it will serve as the default cache. Otherwise, disk storage will be employed.

Regarding caching and filter application, it will only be saved when the filter is provided in the layer's configuration and will not be applied when it comes from a request to the server.

By default, the cache files are stored in the "cache" directory located at the root of your project. However, you can also specify a different location for the cache directory as an argument when starting the server. Example:

`./mvt-rs --cache /tmp/cache`


## Installation

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
Usage: mvt-server [OPTIONS]

Options:
  -c, --config <CONFIGDIR>             Directory where config file is placed [default: config]
  -b, --cache <CACHEDIR>               Directory where cache files are placed [default: cache]
  -i, --host <HOST>                    Bind address [default: 0.0.0.0]
  -p, --port <PORT>                    Bind port [default: 5887]
  -d, --dbconn <DBCONN>                Database connection
  -r, --redisconn <REDISCONN>          Redis connection
  -j, --jwtsecret <JWTSECRET>          JWT secret key
  -s, --sessionsecret <SESSIONSECRET>  Session secret key
  -m, --dbpoolmin <DBPOOLMIN>          Minimum database pool size [default: 2]
  -x, --dbpoolmax <DBPOOLMAX>          Maximum database pool size [default: 5]
  -h, --help                           Print help
```

## Example

```
./mvt-server \
  --config config_folder \
  --cache cache_folder \
  --host 127.0.0.1 \
  --port 8000 \
  --dbconn "postgres://user:password@localhost:5432/mydb" \
  --redisconn "redis://127.0.0.1:6379" \
  --jwtsecret "supersecretjwt" \
  --sessionsecret "supersecretsession" \
  --dbpoolmin 5 \
  --dbpoolmax 20
```

## To-Do

- Metadata management
- Improve the API
- Create tutorial
