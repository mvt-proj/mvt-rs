[English](README.md) | 🌐 **Español**

# MVT Server

## Una Plataforma. Todos los Recursos Cartográficos.

*Una Plataforma de Publicación Cartográfica de Código Abierto*

<div align="center">
  <img src="https://github.com/user-attachments/assets/f7726fd2-bd84-463b-8389-44d6a43fcef5" width="40%" />
</div>

**MVT Server** convierte datos de PostGIS en servicios cartográficos completos y listos para producción — no solo vector tiles.

Capas, mapas, estilos, leyendas, glifos y sprites se publican, organizan y operan desde un solo lugar: un único binario en Rust con una interfaz de administración web moderna detrás. Sin archivos de configuración editados a mano, sin usar herramientas distintas para cada recurso.

---

## ¿Qué Puede Publicar MVT Server?

| Recurso | Descripción |
|---|---|
| 🗺 Capas de Vector Tiles | Publica tablas y vistas de PostGIS como servicios MVT |
| 🧭 TileJSON | Documento TileJSON 3.0.0 por capa más un índice de descubrimiento, para configurar clientes sin configuración manual |
| 🌍 Mapas | Mapas MapLibre completos compuestos por múltiples capas |
| 🎨 Estilos de Capa | Estilos reutilizables para capas individuales |
| 📖 Leyendas | Leyendas dinámicas |
| 🔤 Glifos | Hosting de fuentes |
| 🎯 Sprites | Hosting de íconos |

Cada recurso de la tabla anterior se gestiona desde la misma interfaz web — creado, versionado y servido sin tocar un archivo de configuración.

---

## Control de Publicación Detallado

Publicar desde PostGIS no es todo o nada. Cada capa se ajusta individualmente, desde la interfaz de administración:

- **Qué tablas o vistas se publican** — nada se expone a menos que lo publiques explícitamente.
- **Qué campos de esa capa viajan en el tile** — y en qué orden.
- **Política de caché por capa** — cada capa define su propio max-age de caché, independientemente del resto.
- **Límite de registros por solicitud** — limita cuántos registros puede devolver una solicitud de tile, por capa.
- **Filtros SQL y rango de zoom por capa** — controla qué registros se sirven y en qué niveles de zoom.
- **Control de acceso por capa** — restringe una capa a grupos de usuarios específicos.

Los tiles que servís llevan exactamente los datos que querés — ni más, ni menos.

---

## Viéndolo en Acción

Dos clips, un único flujo de trabajo continuo — desde una tabla de PostGIS sin procesar hasta un mapa con estilo en QGIS, sin tocar un archivo de configuración.

### 1. Publicar una capa y consumirla como servicio de vector tiles

Publicá una tabla de PostGIS como capa MVT desde la interfaz de administración y luego conectate a ella en vivo desde QGIS como fuente de Vector Tiles.



https://github.com/user-attachments/assets/cb4071d2-39fa-4ccb-99a2-d97bad72e599



### 2. Publicar un estilo y aplicarlo a esa capa

Creá un estilo MapLibre desde la interfaz de administración y asignalo a la capa publicada en el paso 1 — la misma capa, ahora con cartografía.



https://github.com/user-attachments/assets/220999be-fb03-4c8d-85c0-b520a4037eb7



---

## Despliegue

MVT Server soporta despliegues tanto **Standalone** como en **Cluster**.

### Standalone
- Un único ejecutable
- Caché local o Redis compartido
- Instalación simple

### Cluster
- Múltiples instancias
- PostgreSQL/PostGIS compartido
- Caché Redis compartido
- Soporte de balanceador de carga

Ver [docs/clustering.md](docs/clustering.md).

---

## Filosofía del Proyecto

Publicar mapas vectoriales debería ser tan simple como publicar una aplicación web.

El ecosistema geoespacial de código abierto ya ofrece herramientas excelentes para servir vector tiles, implementar estándares OGC y exponer APIs geoespaciales. MVT Server se enfoca en un desafío distinto:

> **Una única plataforma para publicar, gestionar y operar cada recurso cartográfico — directamente desde PostGIS.**

Esa es la idea detrás del eslogan: no se trata de sumar un servidor de tiles más al ecosistema, sino de darle a cada recurso que un mapa necesita (capas, estilos, leyendas, glifos, sprites) un único hogar, con una interfaz web en lugar de archivos de configuración editados a mano.

---

## El Ecosistema Geoespacial de Código Abierto

El ecosistema geoespacial es rico en excelente software de código abierto. Cada proyecto tiene su propia filosofía y fortalezas, y muchos se complementan entre sí en lugar de competir.

MVT Server está diseñado para integrarse naturalmente con tecnologías como:

- PostGIS
- MapLibre
- QGIS
- OpenLayers
- Leaflet
- Redis
- Nginx
- Prometheus

Su objetivo no es reemplazar las herramientas existentes, sino simplificar la publicación y administración de servicios de mapas vectoriales.

---

## Capacidades de la Plataforma

Más allá de *qué* publica MVT Server (ver la tabla anterior), esto es *cómo* opera como plataforma:

### Fuentes y Composición

- Múltiples bases de datos PostgreSQL.
- Fuentes de una sola capa, multi-capa y basadas en categoría.
- Composición de capas.
- Control detallado por capa: campos publicados y su orden, filtro SQL, rango de zoom, caché, límites de registros, grupos permitidos.

### Administración

- Interfaz de administración web moderna.
- Catálogo de capas.
- Categorías.
- Gestión de usuarios y grupos.
- Autenticación mediante JWT o Basic Auth.

### Infraestructura

- Caché Redis o en disco.
- Control de caché a nivel de capa.
- Panel de monitoreo.
- Métricas de Prometheus.
- Sistema de plugins Lua.
- Construido en Rust para rendimiento y confiabilidad.

---

## Consejos de Rendimiento

- Habilitá gzip para los vector tiles.
- Configurá la caché por capa.
- Usá Redis cuando corras múltiples instancias.

---

## Primeros Pasos

```sh
git clone https://github.com/mvt-proj/mvt-rs.git
cd mvt-rs
cargo build --release
./target/release/mvt-rs
```

Luego abrí la interfaz de administración web en `http://localhost:<port>` *(confirmar/reemplazar con el puerto por defecto real)* para conectar tu base de datos PostGIS y publicar tu primera capa.

Ver [TUTORIAL.es.md](TUTORIAL.es.md) para instrucciones completas de instalación, configuración, publicación de capas, estilos MapLibre, integración con QGIS, monitoreo y clustering.

---

## Licencia

MVT Server está licenciado bajo la [licencia BSD-3-Clause](https://github.com/mvt-proj/mvt-rs#BSD-3-Clause-1-ov-file).

---

## Apoyo

Si MVT Server ayuda a tu organización, considerá apoyar el proyecto.

❤️ Gracias por ayudar a mantener el proyecto activo.
