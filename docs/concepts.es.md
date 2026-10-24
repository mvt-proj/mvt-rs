[English](concepts.md) | 🌐 **Español**

# Conceptos de MVT Server

MVT Server organiza todo lo que publica alrededor de cinco ideas: **Base de datos →
Capa → Estilo → Mapa → Cliente**. Esta página te da un modelo mental rápido antes de
meterte en [TUTORIAL.es.md](../TUTORIAL.es.md) para las instrucciones paso a paso.

## Base de datos

Una **Base de datos** es una conexión PostgreSQL/PostGIS registrada con un nombre en
`postgres_databases` (`config.yaml` o las variables de entorno
`MVT_POSTGRES_DATABASES__*`). La mayoría de las instalaciones solo necesitan una,
llamada `default`, pero podés registrar varias — útil cuando tus datos geográficos
viven en más de una instancia de PostGIS. Cada capa que publiques queda asociada a
exactamente una base de datos.

## Capa

Una **Capa** (Layer) es una tabla o vista de PostGIS publicada como servicio de
vector tiles. La publicación es explícita y granular: elegís qué campos viajan en el
tile y en qué orden, un filtro SQL opcional, un rango de zoom, una política de caché
por capa, un límite de registros, y qué grupos de usuarios pueden acceder a ella. Nada
se expone hasta que lo publicás desde la interfaz de administración. Las capas se
organizan en categorías, y varias capas pueden combinarse en requests de tiles
multi-capa o por categoría.

## Estilo

Un **Estilo** (Style) es un documento de estilo MapLibre — la cartografía (colores,
símbolos, comportamiento por zoom) aplicada a una o más capas. Los estilos referencian
sus fuentes de datos con un placeholder `mvt://` en vez de una URL real atada al
entorno, así el mismo estilo funciona sin cambios en cualquier entorno; MVT Server
reescribe esos placeholders a URLs absolutas de tiles/sprite/glyphs recién cuando el
estilo se sirve realmente.

## Mapa

Un **Mapa** (Map) es un Estilo renderizado como un mapa MapLibre completo e
interactivo — lo que obtenés al abrir `/mapview/{style_id}` en un navegador. Como un
estilo puede listar varias capas como fuentes, un solo mapa puede componer varias
capas en un único producto cartográfico, listo para compartir o embeber tal cual.

## Cliente

Un **Cliente** es cualquier cosa que consuma lo que publica MVT Server: QGIS, una
aplicación web con MapLibre GL JS u OpenLayers, Leaflet, o cualquier herramienta
compatible con OGC. Los clientes típicamente arrancan desde TileJSON (para descubrir
la capa sin configuración manual) o una URL de estilo, y desde ahí consumen tiles,
sprites, glyphs y leyendas directamente — sin construir URLs a mano.

---

Uniendo todo: una **base de datos** contiene la geometría cruda, una **capa** publica
una parte de ella bajo tu control, un **estilo** le da a una o más capas un tratamiento
cartográfico, un **mapa** es ese estilo renderizado como producto terminado, y un
**cliente** es quien lo consume. Ver el diagrama de flujo de trabajo al principio de
[TUTORIAL.es.md](../TUTORIAL.es.md) para el mismo recorrido de punta a punta.
