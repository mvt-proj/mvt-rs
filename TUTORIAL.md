# MVT Server Tutorial

MVT Server allows you to publish geographic layers in vector tile format on an intranet or the internet for consumption by desktop clients like QGIS, or web clients such as MapLibre, OpenLayers, or Leaflet.

MVT Server not only allows you to publish geographic layers in vector tile format, but also includes an administration panel that simplifies the management of publishing your layers and configuring styles.


<div align="center">
  <img src="https://github.com/user-attachments/assets/c7a90392-b180-419e-bc0e-20d3b56ec000" width="40%" />
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
   - [About the Sources](#about-the-sources)
   - [QGIS](#qgis)
   - [Web Clients](#web-clients)
9. [Serving Styles](#serving-styles)
10. [Serving Legends](#serving-legends)
11. [Serving Sprites and Glyphs in MVT Server](#serving-glyphs-and-sprites-in-mvt-server)
   - [Sprites](#serving-sprites)
   - [Glyphs](#serving-glyphs)
---

## Requirements
- Operating System (Freebsd, Linux, Windows)
- Access to a PostgreSQL server with PostGIS version 3.0.0 or higher installed, either local or remote. The **MVT Server** will be able to publish geographic layers as vector tiles.
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
  -m, --mapassets <MAPASSETS>          Directory where map_assets files are placed [default: map_assets]
  -i, --host <HOST>                    Bind address [default: 0.0.0.0]
  -p, --port <PORT>                    Bind port [default: 5800]
  -d, --dbconn <DBCONN>                Database connection
  -n, --dbpoolmin <DBPOOLMIN>          Minimum database pool size [default: 2]
  -x, --dbpoolmax <DBPOOLMAX>          Maximum database pool size [default: 5]
  -r, --redisconn <REDISCONN>          Redis connection
  -j, --jwtsecret <JWTSECRET>          JWT secret key
  -s, --sessionsecret <SESSIONSECRET>  Session secret key
  -h, --help                           Print help
```

### Example

```
./mvt-server \
  --config config_folder \
  --cache cache_folder \
  --mapassets mapassets_folder \
  --host 127.0.0.1 \
  --port 8000 \
  --dbconn "postgres://user:password@localhost:5432/mydb" \
  --dbpoolmin 5 \
  --dbpoolmax 20 \
  --redisconn "redis://127.0.0.1:6379" \
  --jwtsecret "supersecretjwt" \
  --sessionsecret "supersecretsession"
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
JWTSECRET=supersecretjwt # Used to create and validate JWT tokens
SESSIONSECRET=supersecretsession # Secret key for session management

# Directories
CONFIG=/path_to/config_dir             # Directory path for configuration files
CACHE=/path_to/cache_dir               # Directory path for cache storage
MAPASSETS=/path_to/map_assets_dir      # Directory path for map_assets storage
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
}
```

#### Setting Up a Load Balancer with Nginx

Configure Nginx as a load balancer to distribute traffic across multiple backend servers. Load balancing improves application performance, enhances availability, and ensures fault tolerance.

##### Prerequisites

   - A Unix server with Nginx installed.
   - Three (for example) backend servers running on ports 5800, 5801, and 5802.

It is recommended to use Redis as a cache in this case."

```nginx
http {
    upstream backend_servers {
        server localhost:5800;
        server localhost:5801;
        server localhost:5802;
    }

    server {
        listen 80;
        server_name yourdomain.com;

        location / {
            proxy_pass http://backend_servers;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        }
    }
}
```

## First Use & Authentication

When the server starts for the first time, the necessary components for its configuration will be automatically generated. An initial user with the 'admin' role will be created with the following credentials:

- Email: **admin@mail.com**
- Password: **admin**

The initial access credentials for MVT Server are: email **admin@mail.com** and password **admin**. It is of utmost importance that, upon your first access to the platform, you change this default password to a new, strong password of your choice. This will help protect your server and data from unauthorized access


Access `http://localhost:5800`

To access the MVT Server administration interface, simply enter the address http://localhost:5800 (or the corresponding domain if it is hosted on a remote server) in your web browser. Once there, you can manage your geographic layers, styles, and other server configurations.

![imagen](https://github.com/user-attachments/assets/82a1d638-83c9-4a3d-b92a-1c1c5911d9f8)


![imagen](https://github.com/user-attachments/assets/2ce993cd-5bc3-42c4-be23-311bca4bbd7c)

### MVT Server Administration Panel

The MVT Server administration panel is an essential tool for managing all aspects of publishing your geographic layers as vector tiles. Through an intuitive web interface, you'll have control over:

#### 1. Groups (User Roles)

- Creation and Management: Define user groups or roles with different levels of access and permissions. This allows you to control who can perform administrative tasks, publish layers, create styles, etc.
- Permission Assignment: Assign specific permissions to each group to granularly control access to the server's various functionalities.

#### 2. Users

- Creation and Management: Create new user accounts and manage existing ones.
- Role Assignment: Assign users to specific groups to determine their permissions and level of access.
- Administrative Users: Only users belonging to the "admin" group (or another that is configured as such) will have the ability to perform administrative tasks, such as managing users, groups, categories, catalog, and styles.

#### 3. Categories

- Logical Organization: Categories act as namespaces to organize your layers and styles logically. This is especially useful when working with a large number of layers, as it allows you to keep them organized and easy to find.


#### 4. Catalog (Layer Publishing)

- Layer Declaration: This is the central section of the administrator. Here you define and declare the geographic layers you want to publish as vector tiles.
- Layer Configuration: Specify the data source for each layer, projections, and other relevant parameters.
- Publishing: Once configured, the layers will be available for publishing as vector tiles.

#### 5. Styles

- Creation and Management: Define and manage rendering styles for your layers. Styles determine how the layers will appear on the map (colors, symbols, labels, etc.).
- Style Publishing: Publish the created styles so they can be used by clients like QGIS.
- Style Language: MVT Server likely supports a style language like the Maplibre Style Specification, which allows you to define complex and custom styles.

#### In summary

The MVT Server administration panel gives you complete control over the publication of your geographic layers as vector tiles. From managing users and permissions to the detailed configuration of layers and styles, this tool allows you to create and maintain interactive and personalized maps efficiently.

![imagen](https://github.com/user-attachments/assets/accf44c6-644f-48fd-933a-9b2f65b2dd59)



## Serving a data layer

1. Go to the "Catalog" menu
2. Select "Add Layer"
3. Fill out the form

![imagen](https://github.com/user-attachments/assets/53e36cec-57b3-411d-a0ac-d032b812b57b)


The "Name" field must contain a single word preferably in lowercase. In "Alias", you can enter a more descriptive label.

The form allows you to list available schemas in the PostgreSQL database. After selecting a schema, the tables (geographic layers) will be displayed. Finally, once a table is selected, its fields will be shown. It is recommended to publish only the necessary fields.

It is also advisable to properly configure ZMin and ZMax to improve performance. For example, setting ZMin = 0 for a small locality layer is unnecessary. After adding the layer, you can use the map to assign appropriate zoom values.

Most of the following fields can be left with their default values.

When setting up the cache, consider how frequently the layer updates:

- For layers that change infrequently, it is recommended to set Cache = 0 (infinite cache duration).
- The cache can be cleared or purged at any time using the corresponding button.
- Each layer manages its own cache expiration independently.


![imagen](https://github.com/user-attachments/assets/8868309a-5b31-4f3f-b916-1f667dd656b0)


### Testing the Layer

You can check if the parameters entered in the form are correct and if the layer has been successfully published by using the "Map" button.

![imagen](https://github.com/user-attachments/assets/532e617d-7db5-4041-b0cf-84c7af764183)


## Consuming Services

### About the Sources

This server provides access to *vector tiles* through three types of *sources*:

1. Single-layer source
2. Multi-layer source
3. Category-based source

#### 1. Retrieving Tiles from a Single Layer

To get *vector tiles* from a single layer, use the following route:

**Source:**
```
http://127.0.0.1:5887/services/tiles/category:layer_name/{z}/{x}/{y}.pbf
```

---

#### 2. Retrieving Tiles from Multiple Layers

To combine multiple layers into a single *tile*, use this route:

**Source:**
```
http://127.0.0.1:5887/services/tiles/multi/category_1:layer_name_1,category_2:layer_name_2/{z}/{x}/{y}.pbf
```

🔹 *This endpoint returns a composite tile containing both `"layer_name_1"` and `"layer_name_2"` layers.*

**Notes:**
- Multiple layers can be specified using commas (`,`).
- Useful for displaying combined data in the client.

---

#### 3. Retrieving Tiles by Category

To fetch all layers that belong to a specific category, use the following route:

**Source:**
```
http://127.0.0.1:5887/services/tiles/category/category_1/{z}/{x}/{y}.pbf
```

🔹 *This endpoint returns a tile containing all layers in the `"category_1"` category.*

---

#### Summary and Final Notes

---

- Each layer within a composite tile follows its own rules regarding visibility, publishing, caching, etc.
- Leveraging the server's built-in caching capabilities, the composition is performed at the server level rather than in the database..

---

| Source Type | Base Route | Example |
|------------|-----------|---------|
| **Single layer** | `/services/tiles/{layer}/{z}/{x}/{y}.pbf` | `/services/tiles/rivers/12/2345/3210.pbf` |
| **Multiple layers** | `/services/tiles/multi/{layers}/{z}/{x}/{y}.pbf` | `/services/tiles/multi/rivers,roads/12/2345/3210.pbf` |
| **By category** | `/services/tiles/category/{category}/{z}/{x}/{y}.pbf` | `/services/tiles/category/hydrography/12/2345/3210.pbf` |

This system offers flexibility in working with *vector tiles*, allowing both individual layer access and dynamic layer composition.

---

### QGIS
1. Add Source Vector Layer (click with the right button)
2. New Generic Connection
3. Source URL: copy de url from published layer
4. URL Style: It will be seen later, for now leave empty

![imagen](https://github.com/user-attachments/assets/5479944a-6a52-443f-8518-b88c04f5f75c)

![imagen](https://github.com/user-attachments/assets/c16021d4-7d99-4d6d-b622-035a6d6c20b5)

![imagen](https://github.com/user-attachments/assets/8a6e3daa-4b6f-4877-97d4-e5e6184b35f8)

![imagen](https://github.com/user-attachments/assets/e6e9e9ad-743c-4269-bef6-cae1335d8755)



### Web Clients


This section provides examples of how to consume vector tiles from the **MVT Server** using different mapping libraries: **MapLibre GL JS**, **OpenLayers**, and **Leaflet**.


#### MapLibre GL JS
[View Example](examples/maplibre.html)

This example demonstrates how to integrate vector tiles into a **MapLibre GL JS** map. The best approach is to use **MapLibre styles**, which allow for better layer management and styling flexibility. The example loads three separate sources for polygons, lines, and points:
- **Polygons:** `public:polygons_example`
- **Lines:** `public:lines_example`
- **Points:** `public:points_example`

Alternatively, a single source can be used to load all three layers at once from:
```
http://127.0.0.1:5887/services/tiles/category/public/{z}/{x}/{y}.pbf
```

#### OpenLayers
[View Example](examples/openlayers.html)

This example illustrates how to render vector tiles using **OpenLayers**. It loads the same three sources for polygons, lines, and points while also supporting the combined source for improved efficiency.

#### Leaflet
[View Example](examples/leaflet.html)

This example showcases how to use **Leaflet** with vector tiles. Since Leaflet does not natively support vector tiles, it utilizes plugins to correctly render the data from the MVT Server.

Each example is configured to fetch tiles from:
```
http://127.0.0.1:5887/services/tiles/public:{layer}/{z}/{x}/{y}.pbf
```
where `{layer}` can be:
- `polygons_example`
- `lines_example`
- `points_example`

or use the combined source:
```
http://127.0.0.1:5887/services/tiles/category/public/{z}/{x}/{y}.pbf
```
for all three layers.

These examples provide a starting point for integrating vector tiles into your web mapping applications.




## Serving Styles

### Introduction
The MVT Server can also serve styles that define how vector tiles are rendered. These styles can be consumed in different ways:

1. **For rendering in QGIS:** Styles are applied at the layer level, specifying how a layer should be rendered with colors, labels, symbols, and color scales.

2. **For use in MapLibre:** Styles define a complete "project," including sources, layers, metadata, layer styles, sprites, glyphs, zoom levels, and map center. More details can be found in the [MapLibre Style Specification](https://maplibre.org/maplibre-style-spec/).


## Serving Legends

### Introduction

This feature allows you to serve legends based on the styles defined in the previous section, using the [maplibre-legends](https://github.com/mvt-proj/maplibre-legend) library, which is part of the MVT Server ecosystem.

The legend service is particularly useful for integration with data visualization software.

You can request:
- Individual legends by passing the layer ID
- Combined legends
- Legends with or without titles
- Legends that include or exclude raster layers

**More documentation: coming soon**


## Serving Glyphs and Sprites in MVT Server

### Introduction

In MVT Server, sprites and glyphs are essential for rendering vector tiles with custom icons and fonts. This section explains how to structure your assets and configure your MapLibre style to use them correctly.

### Directory Structure

Your assets should be organized as follows:
```
map_assets
├── glyphs
└── sprites
    ├── fa-brand
    │   ├── sprite.json
    │   └── sprite.png
    ├── fa-regular
    │   ├── sprite.json
    │   ├── sprite.png
    │   ├── sprite@2x.json
    │   └── sprite@2x.png
    ├── fa-solid
    │   ├── sprite.json
    │   └── sprite.png
    ├── maplibre
    │   ├── sprite.json
    │   ├── sprite.png
    │   ├── sprite@2x.json
    │   └── sprite@2x.png
    └── maptiler
        ├── sprite.json
        ├── sprite.png
        ├── sprite@2x.json
        └── sprite@2x.png
```

### Serving Sprites

Sprites are served dynamically by MVT Server. Each sprite set is accessible via a URL like this:

`http://127.0.0.1:5887/services/sprites/{sprite_name}/sprite`

For example, to use the maplibre sprite set:

`http://127.0.0.1:5887/services/sprites/maplibre/sprite`

To configure this in your MapLibre style JSON:
```
{
  "version": 8,
  "sprite": "http://127.0.0.1:5887/services/sprites/maplibre/sprite",
  "sources": { ... },
  "layers": [ ... ]
}
```

This tells MapLibre to fetch the sprite JSON and images from your MVT Server.

### Creating Custom Sprites with Spreet

To create your own sprite sets, you can use [Spreet](https://github.com/flother/spreet), a simple tool for generating sprite sheets and metadata from individual images.

### Serving Glyphs

#### Creating Glyphs for MVT Server

This tutorial will guide you through the process of generating glyphs for the **MVT Server** using **fontnik**. Glyphs allow the map server to render text labels properly.

##### 1. Setting Up the Project

Create a new project directory and install `fontnik`:

```sh
$ mkdir glyphs-project
$ cd glyphs-project
$ npm install fontnik
# or using pnpm
$ pnpm install fontnik
```

##### 2. Downloading a Font

Download a font of your choice. In this example, we will use **EmblemaOne** from Google Fonts:

[Google Fonts - Emblema One](https://fonts.google.com/specimen/Emblema+One)

Extract the downloaded ZIP file and move `EmblemaOne-Regular.ttf` into the `glyphs-project` directory.

##### 3. Generating Glyphs

Create a directory to store the glyphs:

```sh
$ mkdir -p glyphs/EmblemaOne-Regular
```

Run the following commands to generate glyph files for different Unicode ranges:

```sh
$ node -e "require('fontnik').range({font: require('fs').readFileSync('EmblemaOne-Regular.ttf'), start: 0, end: 255}, (err, data) => require('fs').writeFileSync('glyphs/EmblemaOne-Regular/0-255.pbf', data))"

$ node -e "require('fontnik').range({font: require('fs').readFileSync('EmblemaOne-Regular.ttf'), start: 256, end: 511}, (err, data) => require('fs').writeFileSync('glyphs/EmblemaOne-Regular/256-511.pbf', data))"

$ node -e "require('fontnik').range({font: require('fs').readFileSync('EmblemaOne-Regular.ttf'), start: 512, end: 767}, (err, data) => require('fs').writeFileSync('glyphs/EmblemaOne-Regular/512-767.pbf', data))"
```

###### Resulting Directory Structure

After running these commands, your `glyphs` directory should have the following structure:

```
glyphs/
└── EmblemaOne-Regular/
    ├── 0-255.pbf
    ├── 256-511.pbf
    └── 512-767.pbf
```

##### 4. Deploying Glyphs to MVT Server

Move or copy the `EmblemaOne-Regular` directory into your **MVT Server's** glyphs directory:

```sh
$ mv glyphs/EmblemaOne-Regular /path/to/map_assets/glyphs/
```

MVT Server will now be able to serve the glyphs.

##### 5. Configuring MapLibre to Use the Glyphs

In your **MapLibre** style JSON, add the glyphs path in the root:

```json
{
  "glyphs": "http://127.0.0.1:5800/services/glyphs/{fontstack}/{range}.pbf"
}
```

In the **layout** section, specify the font name where needed:

```json
"text-font": ["EmblemaOne-Regular"]
```

###### Important Note
The current version of the MVT Server supports only one font in the array. This is because the server ensures the font's existence beforehand through the administration panel.

The glyphs available on the server can be viewed from the Glyphs menu.

---

You have now successfully created and configured glyphs for your MVT Server! 🎉



### Conclusion

By properly structuring your assets and configuring your MapLibre style, you can serve custom sprites and, soon, glyphs with your MVT Server. This setup allows for scalable and customizable vector tile rendering.
