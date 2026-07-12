# Layout
home = Inicio
catalog = Catálogo
styles = Estilos
sprites =  Iconos
glyphs = Glifos
tutorial = Tutorial
login = Iniciar Sesión
logout = Cerrar Sesión
password = Contraseña
groups = Grupos
users = Usuarios
categories = Categorías
metadata = Metadatos
metrics = Métricas

# Pages
welcome = ¡Bienvenido a MVT Server!
welcome-admin = ¡Modo Administrador!
index-subtitle = Serví capas de PostGIS como Mapbox Vector Tiles con caché, autenticación y gestión de estilos.
home-capabilities = Capacidades
home-cta-catalog = Ver Catálogo
home-cta-styles = Ver Estilos
home-cta-admin = Panel de Administración
feature-1 = Teselas Vectoriales
feature-1-desc = Serví tablas y vistas de PostGIS como teselas .pbf con caché en Redis o disco.
feature-2 = Servidor de Estilos
feature-2-desc = Publicá y compartí estilos MapLibre/Mapbox desde el servidor de estilos integrado.
feature-3 = Control de Acceso
feature-3-desc = Gestioná usuarios y grupos con autenticación JWT y permisos por capa.
feature-4 = Múltiples Bases de Datos
feature-4-desc = Conectá múltiples bases de datos PostGIS simultáneamente y serví capas de cada una de forma independiente.
feature-5 = Plugins Lua
feature-5-desc = Inyectá filtros SQL personalizados en tiempo de ejecución mediante scripts Lua — por capa o por categoría, con acceso al usuario, grupos y nivel de zoom.
feature-6 = Metadatos (próximamente)
feature-6-desc = Servidor de metadatos compatible con ISO 19115 para la documentación de datos espaciales.

catalog-of-layers = Catálogo de capas publicadas
list-of-styles = Listado de Estilos
list-of-sprites = Listado de Iconos
list-of-glyps = Listado de Glifos

# Admin
change-password = Cambiar Contraseña

list-of-groups = Listado de Grupos
add-group = AGREGAR GRUPO
new-group = Nuevo Grupo
create-group = Crear Grupo
update-group = Actualizar Grupo
edit-group = Editar Grupo
confirm-delete-group = ¿Está seguro que quiere eliminar este grupo?

list-of-users = Listado de Usuarios
add-user = AGREGAR USUARIO
new-user = Nuevo Usuario
create-user = Crear Usuario
update-user = Actualizar Usuario
edit-user = Editar Usuario
confirm-delete-user = ¿Está seguro que quiere eliminar este usuario?

list-of-categories = Listado de Categorías
add-category = AGREGAR CATEGORÍA
new-category = Nueva Categoría
create-category = Crear Categoría
update-category = Actualizar Categoría
edit-category = Editar Categoría
confirm-delete-category = ¿Está seguro que quiere eliminar esta categoría?

add-style = AGREGAR ESTILO
new-style = Nuevo Estilo
create-style = Crear Estilo
update-style = Actualizar Estilo
edit-style = Editar Estilo
confirm-delete-style = ¿Está seguro que quiere eliminar este estilo?
apply-style = Aplicar
full-style-button = Insertar ejemplo completo
partial-style-button = Insertar ejemplo capa

add-layer = PUBLICAR CAPA
new-layer = Nueva Capa
create-layer = Crear Capa
update-layer = Actualizar Capa
edit-layer = Editar Capa
confirm-delete-cache = ¿Está seguro que quiere eliminar el caché de esta capa?
confirm-delete-layer = ¿Está seguro que quiere eliminar esta capa?

# Common
filter = Filtro
back = Volver
help = Ayuda
configuration = Configuración
copy = Copiar
map = Mapa
legends = Leyendas
info = Información
edit = Editar
switch-published = Publicar / Despublicar
delete-cache = Eliminar Cache
delete = Eliminar
open-json = Abrir JSON
open-png = Abrir PNG


# Maps
zoom-level = Nivel de Zoom
center = Centro


# Tables
new-password = Nueva Contraseaña
email = Correo Electrónico
username = Nombre de Usuario
first-name = Nombre
last-name = Apellido
name = Nombre
info-name-format = Solo minúsculas, números y guión bajo. Los espacios se reemplazan por _ y los acentos se quitan automáticamente
layer-name = Nombre Capa
category = Categoría
description = Descripción
style = Estilo
invalid-json = Formato JSON inválido. Por favor, compruebe la sintaxis.
style-lint-valid = El estilo es válido según el spec de MapLibre
style-lint-errors = Errores del spec de MapLibre
geometry = Geometría
points = Puntos
lines = Líneas
polygons = Polígonos
alias = Alias
database = Base de Datos
schema = Esquema
table = Tabla
fields = Campos
sql-mode = Modo SQL
geom = Geom
srid = SRID
buffer = Buffer
extent = Extent
zmin = Zoom mínimo
zmax = Zoom máximo
zmax-change-buffer-extent = Zoom para cambiar Buffer/Extent
buffer-higher-zoom = Buffer (para zoom elevado)
extent-higher-zoom = Extent (para zoom elevado)
clip-geom = Clip Geom
delete-cache-on-start = Borrar caché al iniciar el servidor
max-cache-age = Máxima edad del caché
info-time-in-seconds = El tiempo debe ingresarse en segundos
info-value-infinity = Un valor de 0 significa edad infinita
max-records = Máxima cantidad de registros
info-max-records = Máxima cantidad de registros a recuperar. Usando 0 ignora esta directiva.
published = Publicada
allowed-groups = Grupos Permitidos
info-empty-allowed-groups = Si está vacío, todos los grupos están permitidos
yes = Sí
no = No
up = Subir
down = Bajar
cancel = Cancelar
no-users-found = No se encontraron usuarios
no-groups-found = No se encontraron grupos
no-categories-found = No se encontraron categorías
plugins = Plugins
no-plugins-found = No hay plugins instalados
plugins-dir-hint = Colocá archivos .lua en el directorio de plugins para comenzar.
plugin-layer = Capa
plugin-category = Categoría
show-code = Ver código
