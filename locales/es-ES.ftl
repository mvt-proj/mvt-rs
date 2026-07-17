# Layout
home = Inicio
catalog = Catálogo
styles = Estilos
sprites = Sprites
glyphs = Glifos
tutorial = Tutorial
login = Iniciar sesión
logout = Cerrar sesión
password = Contraseña
groups = Grupos
users = Usuarios
categories = Categorías
metadata = Metadatos
metrics = Métricas

# Pages
welcome = ¡Bienvenido a MVT Server!
welcome-admin = ¡Modo Administrador!
index-subtitle = Sirve capas de PostGIS como Mapbox Vector Tiles con caché, autenticación y gestión de estilos.
home-capabilities = Capacidades
home-cta-catalog = Ver Catálogo
home-cta-styles = Ver Estilos
home-cta-admin = Panel de Administración
feature-1 = Teselas Vectoriales
feature-1-desc = Sirve tablas y vistas de PostGIS como teselas .pbf con caché en Redis o disco.
feature-2 = Servidor de Estilos
feature-2-desc = Publica y comparte estilos MapLibre/Mapbox desde el servidor de estilos integrado.
feature-3 = Control de Acceso
feature-3-desc = Gestiona usuarios y grupos con autenticación JWT y permisos por capa.
feature-4 = Múltiples Bases de Datos
feature-4-desc = Conecta varias bases de datos PostGIS simultáneamente y sirve capas de cada una de forma independiente.
feature-5 = Plugins Lua
feature-5-desc = Inyecta filtros SQL personalizados en tiempo de ejecución mediante scripts Lua — por capa o por categoría, con acceso al usuario, grupos y nivel de zoom.
feature-6 = Metadatos (próximamente)
feature-6-desc = Servidor de metadatos compatible con ISO 19115 para la documentación de datos espaciales.

catalog-of-layers = Catálogo de capas publicadas
list-of-styles = Listado de Estilos
list-of-sprites = Listado de Sprites
list-of-glyps = Listado de Glifos

# Admin
change-password = Cambiar Contraseña

list-of-groups = Listado de Grupos
add-group = AÑADIR GRUPO
new-group = Nuevo Grupo
create-group = Crear Grupo
update-group = Actualizar Grupo
edit-group = Editar Grupo
confirm-delete-group = ¿Seguro que quieres eliminar este grupo?

list-of-users = Listado de Usuarios
add-user = AÑADIR USUARIO
new-user = Nuevo Usuario
create-user = Crear Usuario
update-user = Actualizar Usuario
edit-user = Editar Usuario
confirm-delete-user = ¿Seguro que quieres eliminar este usuario?

list-of-categories = Listado de Categorías
add-category = AÑADIR CATEGORÍA
new-category = Nueva Categoría
create-category = Crear Categoría
update-category = Actualizar Categoría
edit-category = Editar Categoría
confirm-delete-category = ¿Seguro que quieres eliminar esta categoría?

add-style = AÑADIR ESTILO
new-style = Nuevo Estilo
create-style = Crear Estilo
update-style = Actualizar Estilo
edit-style = Editar Estilo
confirm-delete-style = ¿Seguro que quieres eliminar este estilo?
apply-style = Aplicar
full-style-button = Insertar ejemplo completo
partial-style-button = Insertar ejemplo de capa

add-layer = PUBLICAR CAPA
new-layer = Nueva Capa
create-layer = Crear Capa
update-layer = Actualizar Capa
edit-layer = Editar Capa
confirm-delete-cache = ¿Seguro que quieres eliminar la caché de esta capa?
confirm-delete-layer = ¿Seguro que quieres eliminar esta capa?

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
delete-cache = Eliminar Caché
delete = Eliminar
open-json = Abrir JSON
open-png = Abrir PNG

# Maps
zoom-level = Nivel de zoom
center = Centro

# Fields
new-password = Nueva Contraseña
email = Correo Electrónico
username = Nombre de Usuario
first-name = Nombre
last-name = Apellidos
name = Nombre
info-name-format = Solo minúsculas, números y guion bajo. Los espacios se convierten en _ y los acentos se eliminan automáticamente
layer-name = Nombre de la Capa
category = Categoría
description = Descripción
style = Estilo
invalid-json = Formato JSON no válido. Comprueba la sintaxis.
style-lint-valid = El estilo es válido según la especificación de MapLibre
style-lint-errors = Errores de la especificación de MapLibre
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
zmax-change-buffer-extent = Umbral de zoom para nuevo Buffer/Extent
buffer-higher-zoom = Buffer (para zoom más alto)
extent-higher-zoom = Extent (para zoom más alto)
clip-geom = Recortar Geometría
delete-cache-on-start = Eliminar caché al iniciar
max-cache-age = Edad máxima de la caché
info-time-in-seconds = El tiempo debe indicarse en segundos
info-value-infinity = El valor 0 significa infinito
max-records = Número máximo de registros
info-max-records = Máximo de registros a recuperar. Usar 0 ignora esta directiva.
published = Publicado
allowed-groups = Grupos Permitidos
info-empty-allowed-groups = Si está vacío, se permiten todos los grupos
yes = Sí
no = No
up = Subir
down = Bajar
cancel = Cancelar
no-users-found = No se han encontrado usuarios
no-groups-found = No se han encontrado grupos
no-categories-found = No se han encontrado categorías
plugins = Plugins
no-plugins-found = No hay plugins instalados
plugins-dir-hint = Coloca archivos .lua en el directorio de plugins para empezar.
plugin-layer = Capa
plugin-category = Categoría
show-code = Ver código
