[English](TUTORIAL.md) | 🌐 **Español**

# Tutorial de MVT Server

MVT Server no es solo un servidor de vector tiles.

Es una plataforma de código abierto diseñada para publicar mapas vectoriales directamente desde PostGIS. A través de una interfaz de administración web podés publicar capas, organizarlas en catálogos y categorías, gestionar usuarios y permisos, configurar estilos MapLibre, servir leyendas, sprites y glifos, monitorear la plataforma y exponer servicios de vector tiles listos para producción sin depender de archivos de configuración complejos.

## Flujo de Trabajo Típico
![MVT-Server](docs/mvt-server.svg)

## Índice
1. [Requisitos](#requisitos)
2. [Instalación](#instalación)
3. [Configuración](#configuración)
4. [Primer Inicio e Inicio de Sesión](#primer-inicio-e-inicio-de-sesión)
5. [El Panel de Administración](#el-panel-de-administración)
6. [Publicando Tu Primera Capa](#publicando-tu-primera-capa)
7. [Consumiendo Tiles](#consumiendo-tiles)
   - [Fuentes de Tiles](#fuentes-de-tiles)
   - [TileJSON (Descubrimiento de Servicios)](#tilejson-descubrimiento-de-servicios)
   - [QGIS](#qgis)
   - [Clientes Web](#clientes-web)
8. [Estilos](#estilos)
   - [Sirviendo Estilos](#sirviendo-estilos)
   - [Sprites](#sprites)
   - [Glifos](#glifos)
   - [Leyendas](#leyendas)
9. [Filtrado Avanzado](#filtrado-avanzado)
10. [Caché](#caché)
    - [Deshabilitar la Caché (Solo para Testing)](#deshabilitar-la-caché-solo-para-testing)
11. [Despliegue en Producción](#despliegue-en-producción)
12. [Monitoreo y Métricas](#monitoreo-y-métricas)
---

## Requisitos

- Un sistema operativo soportado por Rust: Linux, FreeBSD, macOS o Windows.
- Acceso a un servidor PostgreSQL con PostGIS 3.0.0 o superior, local o remoto. Las capas geográficas que publiques se leerán desde aquí.
- Un puerto libre para el servidor (por defecto: `5887`).

## Instalación

Por ahora, la única opción es descargar el código y compilarlo manualmente; en el futuro se ofrecerán binarios para distintos sistemas operativos. Para compilar el servidor, asegurate de tener [Rust instalado](https://www.rust-lang.org/tools/install) en tu sistema.

```sh
# Clonar el repositorio
git clone https://github.com/mvt-proj/mvt-rs.git
cd mvt-rs

# Compilar para producción
cargo build --release
```

El binario se genera en `target/release/mvt-server`. Podés moverlo a donde quieras — solo asegurate de que pueda encontrar su archivo de configuración (siguiente sección).

> **¿Preferís contenedores?** El repositorio incluye una configuración completa de Docker (MVT Server + PostGIS + Redis) en [`docker-example/`](docker-example/DOCKER_README.md).

## Configuración

MVT Server lee su configuración desde un único archivo `config.yaml`. Hay una referencia completamente comentada en [`config.example.yaml`](config.example.yaml); copialo a `config/config.yaml` y ajustá los valores.

Una configuración completa se ve así (los ajustes opcionales están comentados):

```yaml
server:
  host: "0.0.0.0"
  port: 5887
  # Public base URL used to build absolute URLs (e.g. in TileJSON responses).
  # Set it when running behind a proxy or load balancer.
  # public_url: "https://tiles.example.com"

# At least one entry named "default" is required.
postgres_databases:
  pool_min: 2
  pool_max: 5
  default: "postgres://user:password@host:5432/database"
  # foo: "postgres://user:password@host:5432/database_foo"

database:
  sqlite_path: "mvtrs.db"
  # redis_url: "redis://localhost:6379"   # omit to use the disk cache

# Both secrets must be at least 32 characters long.
security:
  jwt_secret: "change-me-to-a-random-secret-at-least-32-chars-long"
  session_secret: "change-me-to-another-random-secret-at-least-32-chars"
  session_duration_minutes: 20   # session TTL (default: 20)

paths:
  config: "config"
  cache: "cache"
  assets: "map_assets"
  plugins: "plugins"   # directory scanned for Lua plugin files at startup

# Multi-instance setups only. A single server runs as "standalone" (default);
# the other modes (shared | owner | client) are covered in docs/clustering.md.
cluster:
  mode: "standalone"
  # config_watch_interval_secs: 10
  # cache_invalidation_extra_delay_secs: 5
  # owner_url: "https://owner-host:5887"   # required when mode = client
  # shared_secret: "change-me"             # required when mode = owner or client
```

Algunas notas:

- `postgres_databases` puede contener varias conexiones con nombre; cada capa elige de cuál lee. La entrada `default` es obligatoria.
- `database.sqlite_path` es el archivo SQLite interno donde MVT Server guarda su propia configuración (usuarios, grupos, catálogo, estilos). La ruta es relativa a `paths.config` y el archivo se crea automáticamente en el primer inicio.
- `database.redis_url` cambia la caché de tiles de disco a Redis — ver [Caché](#caché).
- `paths.plugins` apunta al directorio de plugins Lua — ver [docs/plugins.md](docs/plugins.md).
- `cluster` solo importa cuando corrés varias instancias detrás de un balanceador de carga — ver [docs/clustering.md](docs/clustering.md). Cualquier modo que no sea standalone requiere una caché Redis compartida.
- Cada ajuste también puede definirse como variable de entorno con el prefijo `MVT_` y `__` como separador de subclave, por ejemplo `MVT_SERVER__PORT=5887`.
- Cuando corrás detrás de un proxy o balanceador de carga, configurá `server.public_url` para que las URLs absolutas (por ejemplo, en las respuestas de TileJSON) usen tu dominio público.

### Orden de prioridad

El servidor busca su archivo de configuración en este orden (de mayor a menor prioridad):

1. Argumento de línea de comandos: `--config /path/to/config.yaml`
2. Ruta por defecto: `config/config.yaml` (relativa al directorio de trabajo)

Los valores individuales se resuelven así: argumentos CLI > variables de entorno `MVT_*` > archivo YAML > valores por defecto.

> **¿Actualizando desde una versión anterior a la 0.18.0?** El archivo `.env` ya no es compatible. Movés sus valores a `config.yaml` usando la estructura de arriba.

## Primer Inicio e Inicio de Sesión

Iniciá el servidor:

```sh
./target/release/mvt-server --config config/config.yaml
```

En el primer inicio, MVT Server inicializa todo lo que necesita: crea su base de datos SQLite interna y una cuenta de administrador inicial con las siguientes credenciales:

- Email: **admin@example.com**
- Contraseña: **admin**

Abrí `http://localhost:5887` en tu navegador (o el dominio correspondiente si el servidor está alojado remotamente) e iniciá sesión.

![Login](docs/login.png)

> **Importante:** cambiá la contraseña por defecto inmediatamente después de tu primer inicio de sesión. Dejarla como `admin` expone tu servidor y tus datos a accesos no autorizados.

Después de iniciar sesión llegás a la página principal, desde donde se accede al panel de administración:

![Home](docs/home.png)

## El Panel de Administración

El panel de administración es donde se gestiona toda la plataforma. Está organizado en cinco secciones principales:

### Grupos (Roles de Usuario)

Los grupos definen roles con distintos niveles de acceso. Creá grupos y asignales permisos para controlar quién puede realizar tareas administrativas, publicar capas o crear estilos. Las capas también pueden restringirse para que solo los miembros de ciertos grupos puedan consumirlas.

### Usuarios

Creá y gestioná cuentas de usuario, y asigná cada usuario a uno o más grupos. Solo los usuarios que pertenecen al grupo "admin" pueden realizar tareas administrativas como gestionar usuarios, grupos, categorías, el catálogo y los estilos.

### Categorías

Las categorías funcionan como espacios de nombres que organizan capas y estilos de forma lógica. También forman parte de cada URL de tile (`category:layer_name`), y son especialmente útiles cuando se trabaja con una gran cantidad de capas.

### Catálogo (Publicación de Capas)

La sección central del panel: aquí declarás las capas geográficas a publicar como vector tiles — su fuente de datos, campos, rango de zoom, política de caché y permisos de acceso. La siguiente sección lo explica paso a paso.

![Catalog](docs/catalog.png)

### Estilos

Definí y gestioná estilos de renderizado siguiendo la [MapLibre Style Specification](https://maplibre.org/maplibre-style-spec/): colores, símbolos, etiquetas, escalas de color. Los estilos publicados pueden ser consumidos por clientes como QGIS y MapLibre — cubierto en [Estilos](#estilos).

## Publicando Tu Primera Capa

1. Andá al menú **Catálogo**
2. Hacé clic en **Agregar Capa**
3. Completá el formulario

![Publish Layer](docs/publish_layer.png)

El campo **Name** debe contener una sola palabra, preferentemente en minúsculas. **Alias** acepta una etiqueta más descriptiva.

El formulario lista los esquemas disponibles en la base de datos PostgreSQL. Después de seleccionar un esquema, se muestran sus tablas (capas geográficas); una vez seleccionada una tabla, se muestran sus campos. Se recomienda publicar solo los campos que realmente necesitás.

También es recomendable configurar bien **ZMin** y **ZMax** para mejorar el rendimiento — por ejemplo, no tiene sentido poner ZMin = 0 para una capa de una localidad pequeña. Después de agregar la capa, podés usar la vista de mapa para encontrar los valores de zoom apropiados.

La mayoría de los campos restantes pueden dejarse con sus valores por defecto.

Al configurar la caché, considerá con qué frecuencia cambian los datos de la capa:

- **Cache** se expresa en segundos; cada capa gestiona su propia expiración de forma independiente.
- Para capas que cambian con poca frecuencia, poné **Cache = 0**: los tiles cacheados nunca expiran.
- La caché de una capa puede limpiarse en cualquier momento con el botón correspondiente — más sobre esto en [Caché](#caché).

### Probando la Capa

Usá el botón **Map** para verificar que los parámetros ingresados en el formulario son correctos y que la capa se está sirviendo.

![Testing Layer](docs/testing_layer.png)

## Consumiendo Tiles

Tu capa está publicada — ahora vamos a consumirla desde distintos clientes. MVT Server expone *vector tiles* a través de tres tipos de *fuentes*, además de un documento TileJSON por capa para que los clientes se configuren automáticamente.

### Fuentes de Tiles

1. Fuente de una sola capa
2. Fuente multi-capa
3. Fuente basada en categoría

#### 1. Obteniendo Tiles de una Sola Capa

Para obtener *vector tiles* de una sola capa, usá la siguiente ruta:

**Fuente:**
```
http://127.0.0.1:5887/services/tiles/category:layer_name/{z}/{x}/{y}.pbf
```

---

#### 2. Obteniendo Tiles de Múltiples Capas

Para combinar múltiples capas en un solo *tile*, usá esta ruta:

**Fuente:**
```
http://127.0.0.1:5887/services/tiles/multi/category_1:layer_name_1,category_2:layer_name_2/{z}/{x}/{y}.pbf
```

🔹 *Este endpoint devuelve un tile compuesto que contiene las capas `"layer_name_1"` y `"layer_name_2"`.*

**Notas:**
- Se pueden especificar múltiples capas separadas por comas (`,`).
- Útil para mostrar datos combinados en el cliente.

---

#### 3. Obteniendo Tiles por Categoría

Para obtener todas las capas que pertenecen a una categoría específica, usá la siguiente ruta:

**Fuente:**
```
http://127.0.0.1:5887/services/tiles/category/category_1/{z}/{x}/{y}.pbf
```

🔹 *Este endpoint devuelve un tile que contiene todas las capas de la categoría `"category_1"`.*

---

#### Resumen

| Tipo de Fuente | Ruta Base | Ejemplo |
|------------|-----------|---------|
| **Una sola capa** | `/services/tiles/{layer}/{z}/{x}/{y}.pbf` | `/services/tiles/rivers/12/2345/3210.pbf` |
| **Múltiples capas** | `/services/tiles/multi/{layers}/{z}/{x}/{y}.pbf` | `/services/tiles/multi/rivers,roads/12/2345/3210.pbf` |
| **Por categoría** | `/services/tiles/category/{category}/{z}/{x}/{y}.pbf` | `/services/tiles/category/hydrography/12/2345/3210.pbf` |

Notas:

- Cada capa dentro de un tile compuesto sigue sus propias reglas de visibilidad, publicación y caché.
- La composición se realiza a nivel del servidor (aprovechando la caché integrada) en lugar de en la base de datos.

### TileJSON (Descubrimiento de Servicios)

Cada capa publicada también expone un documento [TileJSON 3.0.0](https://github.com/mapbox/tilejson-spec/tree/master/3.0.0), para que los clientes (MapLibre, QGIS, OpenLayers) puedan descubrir la URL de tiles, el rango de zoom, los límites y el esquema de campos sin configuración manual.

**Índice de capas disponibles:**
```
http://127.0.0.1:5887/services/tilejson
```
Devuelve un array JSON con `id`, `name`, `description` y `tilejson_url` para cada capa publicada visible para el usuario que hace la solicitud.

**Documento por capa:**
```
http://127.0.0.1:5887/services/tilejson/category:layer_name.json
```
Devuelve el documento TileJSON de esa capa:

```json
{
  "tilejson": "3.0.0",
  "tiles": ["http://127.0.0.1:5887/services/tiles/category:layer_name/{z}/{x}/{y}.pbf"],
  "vector_layers": [
    {
      "id": "layer_name",
      "minzoom": 0,
      "maxzoom": 22,
      "fields": { "id": "int4", "name": "Column comment or type name" }
    }
  ],
  "name": "Layer alias",
  "scheme": "xyz",
  "minzoom": 0,
  "maxzoom": 22,
  "bounds": [-63.08, -31.44, -63.01, -31.39],
  "center": [-63.05, -31.42, 11.0]
}
```

**Notas:**
- `name` proviene del alias de la capa (o de su nombre si no hay alias); `description` proviene de la descripción de la capa.
- Cada entrada de `fields` se describe con el comentario de la columna en PostgreSQL cuando está definido (`COMMENT ON COLUMN ...`), o con el nombre de su tipo en caso contrario.
- El control de acceso replica el del endpoint de tiles: solo se sirven capas publicadas, y las capas restringidas por grupo requieren autenticación (404 / 403 en caso contrario).
- Detrás de un proxy o balanceador de carga, configurá `server.public_url` (ver [Configuración](#configuración)) para que las URLs del documento usen tu dominio público.

---

### QGIS

1. En el panel Browser, hacé clic derecho en **Vector Tiles** y elegí **New Generic Connection**
2. Dale un nombre a la conexión
3. **URL**: pegá la URL de tiles de la capa publicada, por ejemplo `http://127.0.0.1:5887/services/tiles/category:layer_name/{z}/{x}/{y}.pbf`
4. Configurá **Min. Zoom Level** y **Max. Zoom Level** según la capa
5. **Style URL** se puede dejar vacío por ahora — los estilos se cubren en [Estilos](#estilos)

> **Nota:** la conexión genérica integrada de QGIS solo acepta la plantilla
> de tiles XYZ (`.../{z}/{x}/{y}.pbf`), no una URL de TileJSON. Aun así, el
> documento TileJSON de la capa (`http://.../services/tilejson/category:layer_name.json`)
> es útil acá: te da la URL exacta de tiles para pegar, además de los valores
> de Min/Max Zoom para el diálogo de conexión y el esquema de campos de la
> capa. Plugins como MapTiler pueden consumir URLs de TileJSON directamente.

![QGis Connetion](docs/qgis_connection.png)

![QGIS Render](docs/qgis_render.png)

### Clientes Web

Esta sección ofrece ejemplos de cómo consumir vector tiles desde **MVT Server** usando distintas bibliotecas de mapas: **MapLibre GL JS**, **OpenLayers** y **Leaflet**.

#### MapLibre GL JS
[Ver Ejemplo](examples/maplibre.html)

Este ejemplo muestra cómo integrar vector tiles en un mapa de **MapLibre GL JS**. El mejor enfoque es usar **estilos MapLibre**, que permiten una mejor gestión de capas y más flexibilidad de estilo. El ejemplo carga tres fuentes separadas para polígonos, líneas y puntos:
- **Polígonos:** `public:polygons_example`
- **Líneas:** `public:lines_example`
- **Puntos:** `public:points_example`

Alternativamente, se puede usar una sola fuente para cargar las tres capas a la vez desde:
```
http://127.0.0.1:5887/services/tiles/category/public/{z}/{x}/{y}.pbf
```

También se puede definir una fuente a partir del documento TileJSON de la capa en lugar de escribir el array `tiles` a mano — MapLibre toma automáticamente la URL de tiles, el rango de zoom y los límites:
```js
map.addSource("polygons", {
  type: "vector",
  url: "http://127.0.0.1:5887/services/tilejson/public:polygons_example.json"
});
```

#### OpenLayers
[Ver Ejemplo](examples/openlayers.html)

Este ejemplo ilustra cómo renderizar vector tiles usando **OpenLayers**. Carga las mismas tres fuentes para polígonos, líneas y puntos, y también soporta la fuente combinada para mayor eficiencia.

#### Leaflet
[Ver Ejemplo](examples/leaflet.html)

Este ejemplo muestra cómo usar **Leaflet** con vector tiles. Dado que Leaflet no soporta vector tiles de forma nativa, utiliza plugins para renderizar correctamente los datos de MVT Server.

Cada ejemplo está configurado para obtener tiles desde:
```
http://127.0.0.1:5887/services/tiles/public:{layer}/{z}/{x}/{y}.pbf
```
donde `{layer}` puede ser:
- `polygons_example`
- `lines_example`
- `points_example`

o usar la fuente combinada:
```
http://127.0.0.1:5887/services/tiles/category/public/{z}/{x}/{y}.pbf
```
para las tres capas.

Estos ejemplos son un punto de partida para integrar vector tiles en tus aplicaciones de mapas web.

## Estilos

Hasta ahora el mapa muestra geometría cruda. Esta sección cubre todo lo relacionado con su apariencia: estilos, los sprites y glifos que referencian esos estilos, y las leyendas generadas a partir de ellos.

### Sirviendo Estilos

MVT Server sirve estilos que definen cómo se renderizan los vector tiles. Pueden consumirse de dos formas:

1. **En QGIS:** los estilos se aplican a nivel de capa, especificando colores, etiquetas, símbolos y escalas de color.
2. **En MapLibre:** los estilos definen un "proyecto" completo, incluyendo fuentes, capas, metadatos, sprites, glifos, niveles de zoom y centro del mapa. Ver la [MapLibre Style Specification](https://maplibre.org/maplibre-style-spec/).

Los estilos se crean y publican desde la sección **Styles** del panel de administración.

![Styles](docs/styles.png)
![New Layer Style](docs/new_layer_style.png)

### Sprites

Los sprites agrupan los íconos que usa un estilo en una sola imagen más un índice JSON. Tus assets deben organizarse así bajo `paths.assets`:

#### Estructura de Directorios

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

#### Sirviendo Sprites

Los sprites son servidos dinámicamente por MVT Server. Cada conjunto de sprites es accesible mediante una URL como esta:

`http://127.0.0.1:5887/services/map_assets/sprites/{sprite_name}/sprite`

Por ejemplo, para usar el conjunto de sprites maplibre:

`http://127.0.0.1:5887/services/map_assets/sprites/maplibre/sprite`

Para configurar esto en el JSON de tu estilo MapLibre:
```
{
  "version": 8,
  "sprite": "http://127.0.0.1:5887/services/map_assets/sprites/maplibre/sprite",
  "sources": { ... },
  "layers": [ ... ]
}
```

Esto le indica a MapLibre que obtenga el JSON y las imágenes de sprites desde tu MVT Server.

#### Creando Sprites Personalizados con Spreet

Para crear tus propios conjuntos de sprites, podés usar [Spreet](https://github.com/flother/spreet), una herramienta simple para generar hojas de sprites y metadatos a partir de imágenes individuales.

### Glifos

Este tutorial te guía a través del proceso de generación de glifos para **MVT Server** usando **fontnik**. Los glifos permiten que el servidor de mapas renderice correctamente las etiquetas de texto.

#### 1. Preparando el Proyecto

Creá un nuevo directorio de proyecto e instalá `fontnik`:

```sh
$ mkdir glyphs-project
$ cd glyphs-project
$ npm install fontnik
# o usando pnpm
$ pnpm install fontnik
```

#### 2. Descargando una Fuente

Descargá una fuente de tu elección. En este ejemplo usaremos **EmblemaOne** de Google Fonts:

[Google Fonts - Emblema One](https://fonts.google.com/specimen/Emblema+One)

Extraé el archivo ZIP descargado y movés `EmblemaOne-Regular.ttf` al directorio `glyphs-project`.

#### 3. Generando Glifos

Creá un directorio para guardar los glifos:

```sh
$ mkdir -p glyphs/EmblemaOne-Regular
```

Ejecutá los siguientes comandos para generar los archivos de glifos para distintos rangos Unicode:

```sh
$ node -e "require('fontnik').range({font: require('fs').readFileSync('EmblemaOne-Regular.ttf'), start: 0, end: 255}, (err, data) => require('fs').writeFileSync('glyphs/EmblemaOne-Regular/0-255.pbf', data))"

$ node -e "require('fontnik').range({font: require('fs').readFileSync('EmblemaOne-Regular.ttf'), start: 256, end: 511}, (err, data) => require('fs').writeFileSync('glyphs/EmblemaOne-Regular/256-511.pbf', data))"

$ node -e "require('fontnik').range({font: require('fs').readFileSync('EmblemaOne-Regular.ttf'), start: 512, end: 767}, (err, data) => require('fs').writeFileSync('glyphs/EmblemaOne-Regular/512-767.pbf', data))"
```

##### Estructura de Directorios Resultante

Después de ejecutar estos comandos, tu directorio `glyphs` debería tener la siguiente estructura:

```
glyphs/
└── EmblemaOne-Regular/
    ├── 0-255.pbf
    ├── 256-511.pbf
    └── 512-767.pbf
```

#### 4. Desplegando Glifos en MVT Server

Movés o copiá el directorio `EmblemaOne-Regular` al directorio de glifos de tu **MVT Server**:

```sh
$ mv glyphs/EmblemaOne-Regular /path/to/map_assets/glyphs/
```

MVT Server ahora podrá servir los glifos.

#### 5. Configurando MapLibre para Usar los Glifos

En el JSON de tu estilo **MapLibre**, agregá la ruta de glifos en la raíz:

```json
{
  "glyphs": "http://127.0.0.1:5887/services/map_assets/glyphs/{fontstack}/{range}.pbf"
}
```

En la sección **layout**, especificá el nombre de la fuente donde sea necesario:

```json
"text-font": ["EmblemaOne-Regular"]
```

##### Nota Importante
La versión actual de MVT Server soporta solo una fuente en el array. Esto se debe a que el servidor verifica de antemano la existencia de la fuente a través del panel de administración.

Los glifos disponibles en el servidor pueden verse desde el menú Glyphs.

### Leyendas

MVT Server puede servir leyendas generadas a partir de estilos publicados, usando la biblioteca [maplibre-legend](https://github.com/mvt-proj/maplibre-legend), parte del ecosistema de MVT Server. El servicio de leyendas es particularmente útil para integrarse con software de visualización de datos.

Podés solicitar:

- Leyendas individuales pasando el ID de la capa
- Leyendas combinadas
- Leyendas con o sin títulos
- Leyendas que incluyen o excluyen capas raster

<!-- screenshot: legends output, individual and combined -->

**Más documentación: próximamente**

## Filtrado Avanzado

Más allá de servir capas completas, MVT Server soporta filtrado directamente desde la URL de origen mediante parámetros de consulta (query parameters). Los filtros se traducen dinámicamente en cláusulas SQL `WHERE`, lo que permite mostrar distintos subconjuntos de datos según la consulta del usuario — sin modificar el backend ni exponer la lógica de la base de datos.

---

### Sintaxis de Filtros

El formato de filtro soporta tres modos lógicos y varios operadores similares a SQL.

#### Operadores

| Sufijo        | Equivalente SQL |
|---------------|----------------|
| `__eq` (por defecto) | `=`          |
| `__ne`         | `<>`           |
| `__gt`         | `>`            |
| `__gte`        | `>=`           |
| `__lt`         | `<`            |
| `__lte`        | `<=`           |
| `__like`       | `LIKE`         |
| `__ilike`       | `ILIKE`         |
| `__in`         | `IN` (valores separados por coma) |

#### Modos Lógicos

| Prefijo        | Lógica |
|---------------|-------|
| *(ninguno)*      | `AND` |
| `or__`        | `OR`  |
| `not__`       | `NOT` |

---

### URLs de Ejemplo

```text
/services/tiles/public:states/{z}/{x}/{y}.pbf?or__name__in='FOO','BAR'&or__id__in=6,9,22,24
/services/tiles/public:vtr2024/{z}/{x}/{y}.pbf?or__vur_foo__gte=9000&or__vur_bar__gte=11160000
```

Estas generan cláusulas WHERE como:

```sql
WHERE (name = ANY(ARRAY['FOO','BAR']) OR id = ANY(ARRAY[6,9,22,24]))
```

y

```sql
WHERE (vur_foo >= $1 OR vur_bar >= $2)
```

---

### `filter` definido por el administrador (filtro estático)

En el panel de configuración de la capa, los administradores pueden definir un **filtro SQL fijo** en el campo `filter`. Este filtro se aplica **antes** que cualquier parámetro de consulta dinámico.

Por ejemplo, si el administrador definió:

```sql
status = 'public'
```

y el usuario envía:

```
?or__category__eq='roads'
```

el SQL final será:

```sql
WHERE status = 'public' AND (category = $1)
```

---

### Libertad en los Parámetros de Consulta

En la versión actual, los usuarios pueden especificar **cualquier campo** en la cadena de consulta. No hay restricción sobre qué columnas se pueden consultar. Esto hace que el sistema sea muy flexible, pero también significa que:

> **Deberías controlar la exposición de datos a nivel de capa**, no mediante filtros.

Podría ser deseable en futuras versiones restringir qué campos están permitidos en los filtros, pero esto no está planeado ni garantizado actualmente.

---

### Resumen

- Combiná filtros estáticos (`filter`) y dinámicos (parámetros de consulta).
- Expresá condiciones lógicas usando el AND por defecto, `or__` y `not__`.
- Vincula de forma segura la entrada del usuario para prevenir inyección SQL (excepto `IN`, que actualmente usa literales en línea).
- Compatible con QGIS, MapLibre y clientes web.

### Filtrado programable (plugins)

Más allá de los parámetros de consulta, MVT Server soporta plugins Lua que pueden inspeccionar cada solicitud de tile (usuario, grupos, zoom, cadena de consulta) e inyectar filtros SQL adicionales — útil para control de acceso y seguridad a nivel de fila. Ver [docs/plugins.md](docs/plugins.md).

## Caché

Generar un tile cuesta una consulta a la base de datos, por lo que MVT Server cachea cada tile que sirve. Hay dos backends disponibles:

- **Caché en disco** (por defecto): los tiles se almacenan en el directorio configurado en `paths.cache`. No requiere servicios adicionales — ideal para una configuración de instancia única.
- **Redis**: se habilita configurando `database.redis_url` en `config.yaml`. Es necesario cuando varias instancias corren detrás de un balanceador de carga, para que todas compartan la misma caché y las invalidaciones lleguen a todos los nodos (ver [Despliegue en Producción](#despliegue-en-producción)).

```yaml
database:
  sqlite_path: "mvtrs.db"
  redis_url: "redis://localhost:6379"   # omit to use the disk cache
```

Cuánto tiempo viven los tiles se decide por capa, con dos campos del formulario de capa:

- **Cache** (en segundos): cuánto tiempo se sirve un tile desde la caché antes de regenerarse. `0` significa que los tiles cacheados nunca expiran — recomendado para capas que cambian poco.
- **Delete cache on start**: limpia la caché de la capa cada vez que el servidor arranca.

Editar una capa invalida automáticamente sus tiles cacheados, y la caché de cada capa también puede limpiarse manualmente desde el Catálogo con su botón de purga.

### Deshabilitar la Caché (Solo para Testing)

El flag de línea de comandos `--no-cache` deshabilita por completo la caché de tiles: cada request regenera el tile desde la base de datos, y no se lee ni se escribe nada en Redis/disco.

```sh
cargo run -- --no-cache
```

Está pensado para desarrollo local y tests donde necesitás observar la generación fresca de tiles en cada request. **Nunca uses `--no-cache` en producción** — convierte cada request de tile en una consulta completa a la base de datos, lo que va a saturarla bajo tráfico real.

## Despliegue en Producción

Para uso en producción, corré MVT Server detrás de un proxy inverso como Nginx: puede terminar TLS y comprimir los tiles antes de que salgan de tu red.

### Proxy inverso Nginx

Configuración de ejemplo (`/etc/nginx/sites-available/application.conf`):

```nginx
server {
    listen 80;
    server_name yourdomain.com;

    # Enable gzip compression for vector tiles and API responses.
    # .pbf tiles compress 60-80% on average, significantly reducing bandwidth.
    gzip on;
    gzip_types application/x-protobuf application/octet-stream application/json;
    gzip_min_length 256;
    gzip_proxied any;
    gzip_vary on;

    location / {
        proxy_pass http://localhost:5887;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

Recordá configurar `server.public_url` en `config.yaml` para que las URLs absolutas (por ejemplo, en las respuestas de TileJSON) usen tu dominio público.

### Escalando horizontalmente

Para distribuir tráfico entre varias instancias de MVT Server — balanceo de carga, caché Redis compartida, sincronización de configuración entre nodos — ver [docs/clustering.md](docs/clustering.md). Para una configuración en contenedores con PostGIS y Redis incluidos, ver [docker-example/](docker-example/DOCKER_README.md).

## Monitoreo y Métricas

MVT Server incluye un panel de monitoreo integrado con visualización de métricas en tiempo real. El servidor expone tanto un panel web como un endpoint de métricas compatible con Prometheus.

### Accediendo al Panel

Navegá a `/admin/monitor/dashboard` para ver métricas del servidor en tiempo real, incluyendo:

- **Uso de CPU**: porcentaje de utilización de CPU del proceso (soporta jails de FreeBSD mediante fallback a getrusage)
- **Memoria**: uso de memoria residente en GB
- **RPS (Requests Per Second)**: throughput de solicitudes en tiempo real
- **Latencia**: tiempo de la última solicitud y tiempos de respuesta promedio en milisegundos
- **Rendimiento de Caché**: aciertos y fallos de caché por segundo

![Dashboard](docs/dashboard.png)

El panel se actualiza cada 5 segundos mediante Server-Sent Events (SSE) y muestra datos históricos en gráficos interactivos.

### Métricas de Prometheus

Todas las métricas están disponibles en formato Prometheus en `/api/monitor/metrics`:

```
mvt_server_process_cpu_percent
mvt_server_process_memory_bytes
mvt_server_requests_total
mvt_server_cache_hits_total
mvt_server_cache_misses_total
mvt_server_last_request_latency_seconds
mvt_server_avg_request_latency_seconds
```

Estas métricas pueden ser recolectadas por Prometheus o cualquier sistema de monitoreo compatible para almacenamiento a largo plazo y alertas.

**Nota**: en entornos restringidos como jails de FreeBSD, las métricas de CPU recurren automáticamente a `getrusage()` cuando `sysinfo` no está disponible.
