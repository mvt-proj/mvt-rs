# mvt server: A Simple Vector Tile Server

This is a simple and high-speed vector tile server developed in Rust, using the Salvo web framework. It provides an efficient way to serve vector geospatial data over the web.

Requires a PostgreSQL server with PostGIS version 3.0.0 or higher, either local or remote. It relies on the use of the PostGIS function ST_AsMVT. More information can be found at https://postgis.net/docs/en/ST_AsMVT.html.

<div align="center">
  <img src="https://github.com/user-attachments/assets/a7b743de-5775-4c47-b5c5-adb1378d03ef" width="40%" />
</div>

**mvt server** will allow you to publish any table or view with a geometry field as vector tiles through the definition in the layers' configuration. It will also enable the publication of styles to be used in tools like QGIS. These styles define how layers are displayed, including visualization scales, colors, labels, and other visual elements. This functionality ensures seamless integration with GIS tools, guaranteeing that vector tiles are rendered consistently and as intended across different platforms.



## Environment Variables (.env)

The server uses environment variables for its configuration. Make sure to create a `.env` file at the root of your project with the following variables:

- `DBCONN`: The connection URL for the PostgreSQL database.
  Example: `postgres://user:pass@host/db`

- `POOLSIZEMIN`: Minimum size of the connection pool. Example: `3`

- `POOLSIZEMAX`: Maximum size of the connection pool. Example: `5`

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
  -i, --host <HOST>            Bind address
  -p, --port <PORT>            Bind port
  -d, --dbconn <DBCONN>        Database connection
  -r, --redisconn <REDISCONN>  Redis connection
  -j, --jwtsecret <JWTSECRET>  JWT secret key
  -h, --help                   Print help
```
