# mvt server Tutorial

mvt-server allows you to publish geographic layers in vector tile format on an intranet or the internet for consumption by desktop clients like QGIS, or web clients such as MapLibre, OpenLayers, or Leaflet.

mvt-server not only allows you to publish geographic layers in vector tile format, but also includes an administration panel that simplifies the management of publishing your layers and configuring styles.

<div align="center">
  <img src="https://github.com/user-attachments/assets/2436d908-e8e0-417e-97bb-957e1e0fcfaf" width="40%" />
</div>

## Table of Contents
1. [Requirements](#requirements)
2. [Installation / Compilation](#installation--compilation)
3. [Running the Application](#running-the-application)
   - [Desktop Environment](#desktop-environment)
   - [Server with Nginx](#server-with-nginx)
4. [First Use & Authentication](#first-use--authentication)
5. [Configuration](#configuration)
   - [Environment Variables](#environment-variables)
   - [Command Arguments](#command-arguments)
6. [Publishing Layers & Styles](#publishing-layers--styles)
7. [Consuming Services](#consuming-services)
   - [Web Clients](#web-clients)
   - [QGIS](#qgis)
8. [Troubleshooting](#troubleshooting)
9. [Additional Resources](#additional-resources)

---

## Requirements
- Operating System (Freebsd, Linux, Windows)
- Access to a PostgreSQL server with PostGIS version 3.0.0 or higher installed, either local or remote. The **mvt server** will be able to publish geographic layers as vector tiles.
- Port `5800` available (or configurable)

## Installation / Compilation

For now, the only option is to download the code and compile it manually. In the future, binaries will be provided for different operating systems. To compile the server, ensure you have Rust installed on your system.

https://www.rust-lang.org/tools/install


Then, you can compile and run the project as follows:

```sh
# Clone the repository
git clone https://github.com/mvt-proj/mvt-rs.git
# Navigate to the project directory
cd mvt-rs

# Compile for production
cargo build --release
```

The binary will be generated in the **/target/release/** directory.

You can move it to another location if needed, but remember that the environment variables must be set either in the shell or in the .env file. Alternatively, you can start the server by passing the required arguments.

## Running the Application

### Arguments

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
  -a, --saltstring <SALTSTRING>        Salt string for password hashing
  -h, --help                           Print help
```

### Example

```
./mvt-server \
  --config config_folder \
  --cache cache_folder \
  --host 127.0.0.1 \
  --port 8000 \
  --dbconn "postgres://my_user:my_password@localhost:5432/mydb" \
  --redisconn "redis://127.0.0.1:6379" \
  --jwtsecret "supersecretjwt" \
  --sessionsecret "supersecretsession" \
  --dbpoolmin 5 \
  --dbpoolmax 20 \
  --saltstring "randomsalt"
```

### Environment Variables (.env)

**Make sure to create a `.env` file at the root of your project with the following variables:**

```sh
# Database connection URL (PostgreSQL)
DBCONN=postgres://user:pass@host:port/db

# Connection pool size
POOLSIZEMIN=3   # Minimum size of the connection pool
POOLSIZEMAX=5   # Maximum size of the connection pool

# Server settings
IPHOST=0.0.0.0  # The IP address where the server will listen
PORT=5800       # The port on which the server will run

# Redis connection (optional, overrides disk cache if provided)
REDISCONN=redis://127.0.0.1:6379

# Security settings
SALTSTRING=randomsalt    # Used for Argon2 password hashing
JWTSECRET=supersecretjwt # Used to create and validate JWT tokens
SESSIONSECRET=supersecretsession # Secret key for session management

# Directories
CONFIG=config  # Directory path for configuration files
CACHE=cache    # Directory path for cache storage
```

Remember the `.env` file has to kept secure and not shared in public repositories.


### Server with Nginx
Example reverse proxy configuration (`/etc/nginx/sites-available/application.conf`):
```nginx
server {
    listen 80;
    server_name yourdomain.com;
    
    location / {
        proxy_pass http://localhost:5800;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
    
    # For WebSockets if needed
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
}
```

## First Use & Authentication

When the server starts for the first time, the necessary components for its configuration will be automatically generated. An initial user with the 'admin' role will be created with the following credentials:

- Email: **admin@mail.com**
- Password: **admin**

The initial access credentials for mvt-server are: email **admin@mail.com** and password **admin**. It is of utmost importance that, upon your first access to the platform, you change this default password to a new, strong password of your choice. This will help protect your server and data from unauthorized access


Access `http://localhost:8000`

To access the mvt-server administration interface, simply enter the address http://localhost:5800 (or the corresponding domain if it is hosted on a remote server) in your web browser. Once there, you can manage your geographic layers, styles, and other server configurations.

![imagen](https://github.com/user-attachments/assets/82a1d638-83c9-4a3d-b92a-1c1c5911d9f8)


![imagen](https://github.com/user-attachments/assets/2ce993cd-5bc3-42c4-be23-311bca4bbd7c)

### mvt-server Administration Panel

The mvt-server administration panel is an essential tool for managing all aspects of publishing your geographic layers as vector tiles. Through an intuitive web interface, you'll have control over:

#### 1. Groups (User Roles)

    Creation and Management: Define user groups or roles with different levels of access and permissions. This allows you to control who can perform administrative tasks, publish layers, create styles, etc.    
    Permission Assignment: Assign specific permissions to each group to granularly control access to the server's various functionalities.

#### 2. Users

    Creation and Management: Create new user accounts and manage existing ones.
    Role Assignment: Assign users to specific groups to determine their permissions and level of access.
    Administrative Users: Only users belonging to the "admin" group (or another that is configured as such) will have the ability to perform administrative tasks, such as managing users, groups, categories, catalog, and styles.

#### 3. Categories

    Logical Organization: Categories act as namespaces to organize your layers and styles logically. This is especially useful when working with a large number of layers, as it allows you to keep them organized and easy to find.
    

#### 4. Catalog (Layer Publishing)

    Layer Declaration: This is the central section of the administrator. Here you define and declare the geographic layers you want to publish as vector tiles.
    Layer Configuration: Specify the data source for each layer, projections, and other relevant parameters.
    Publishing: Once configured, the layers will be available for publishing as vector tiles.

#### 5. Styles

    Creation and Management: Define and manage rendering styles for your layers. Styles determine how the layers will appear on the map (colors, symbols, labels, etc.).
    Style Publishing: Publish the created styles so they can be used by clients like QGIS.
    Style Language: mvt-server likely supports a style language like the Maplibre Style Specification, which allows you to define complex and custom styles.

#### In summary

The mvt-server administration panel gives you complete control over the publication of your geographic layers as vector tiles. From managing users and permissions to the detailed configuration of layers and styles, this tool allows you to create and maintain interactive and personalized maps efficiently.

![imagen](https://github.com/user-attachments/assets/accf44c6-644f-48fd-933a-9b2f65b2dd59)





## Configuration
[Installation / Compilation](#installation--compilation)

## Publishing Layers & Styles
1. Load data in GeoPackage or PostGIS format
2. Generate tileset:
   ```bash
   ./application --publish-layer my_layer --format mvt --zoom-levels 0-14
   ```
3. Associate Mapbox GL style:
   ```json
   {
     "version": 8,
     "sources": {
       "my-layer": {
         "type": "vector",
         "url": "http://server/tiles/my_layer.json"
       }
     },
     "layers": [...]
   }
   ```

## Consuming Services

### Web Clients
**MapLibre GL JS**:
```javascript
map.addSource('my-layer', {
  type: 'vector',
  url: 'http://yourdomain/tiles/my_layer'
});

map.addLayer({
  id: 'main-layer',
  source: 'my-layer',
  'source-layer': 'data',
  type: 'fill',
  paint: {'fill-color': '#ff0000'}
});
```

**OpenLayers**:
```javascript
const vectorLayer = new VectorTileLayer({
  source: new VectorTileSource({
    format: new MVT(),
    url: '/tiles/{z}/{x}/{y}.pbf'
  })
});
```

### QGIS
1. Layers → Add Vector Layer
2. Source: `XYZ Tiles`
3. URL: `http://server/tiles/{z}/{x}/{y}.pbf`
4. Style: Load generated `.qml` file

## Troubleshooting
**Common Issue**: Tiles not visible
```bash
# Verify tile generation
curl -I http://localhost:8080/tiles/0/0/0.pbf

# Check application logs
journalctl -u application-service --since "5 minutes ago"
```

## Additional Resources
- [Example Repository](https://github.com/your-examples/demos)
- [Mapbox GL Style Spec Documentation](https://docs.mapbox.com/mapbox-gl-js/style-spec/)
- [QGIS Style Templates](resource_link)





