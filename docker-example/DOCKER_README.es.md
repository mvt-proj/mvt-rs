[English](DOCKER_README.md) | 🌐 **Español**

# MVT-Server Docker Setup

Este proyecto incluye configuración completa de Docker para ejecutar MVT-RS con PostgreSQL+PostGIS y Redis.

## Requisitos

- Docker
- Docker Compose

## Estructura de archivos

```
mvt-rs/
├── .dockerignore              # Archivos excluidos del build (raíz del repo)
├── src/, Cargo.toml, ...      # Código fuente (contexto del build)
└── docker-example/
    ├── Dockerfile              # Imagen multi-stage para MVT-RS
    ├── docker-compose.yml      # Orquestación de servicios
    ├── DOCKER_README.md        # Este archivo
    ├── config/                 # Configuraciones de la aplicación (se crea al iniciar)
    ├── cache/                  # Directorio de caché (se crea al iniciar)
    └── map_assets/              # Recursos de mapas (se crea al iniciar)
```

Todos los comandos de este documento se ejecutan parado en `docker-example/`.
El `docker-compose.yml` apunta el contexto de build a la raíz del repo
(`context: ..`), así que no hace falta mover ni copiar archivos.

## Servicios incluidos

### 1. mvt-server
- **Puerto interno**: 5887 (mapeado externamente a 5880 en docker-compose.yml)
- **Descripción**: Aplicación principal MVT-RS
- **Dependencias**: PostgreSQL, Redis

### 2. postgres
- **Puerto**: 5432
- **Imagen**: postgis/postgis:15-3.4
- **Base de datos**: mvtdb
- **Usuario**: mvtuser
- **Contraseña**: mvtpass

### 3. redis
- **Puerto**: 6379
- **Imagen**: redis:7-alpine
- **Funcionalidad**: Caché para tiles

### 4. adminer (opcional)
- **Puerto**: 8080
- **Funcionalidad**: Administrador web de base de datos

## Comandos básicos

### Iniciar todos los servicios
```bash
docker-compose up -d
```

### Ver logs
```bash
# Todos los servicios
docker-compose logs -f

# Solo MVT-RS
docker-compose logs -f mvt-server

# Solo PostgreSQL
docker-compose logs -f postgres
```

### Detener servicios
```bash
docker-compose down
```

### Reconstruir la aplicación
```bash
docker-compose build --no-cache mvt-server
docker-compose up -d
```

### Acceder al contenedor
```bash
# MVT-RS
docker-compose exec mvt-server bash

# PostgreSQL
docker-compose exec postgres psql -U mvtuser -d mvtdb
```

## Configuración

MVT Server admite tanto un archivo `config.yaml` centralizado como variables de
entorno, y pueden combinarse: la prioridad es `config.yaml` > variables de
entorno > valores por defecto. El `docker-compose.yml` de este ejemplo
configura todo por variables de entorno (sección `environment:` del servicio
`mvt-server`) para que funcione sin pasos extra.

Si preferís centralizar la configuración en un archivo en vez de env vars,
copiá `config.example.yaml` (en la raíz del repo) a `docker-example/config/config.yaml`
y ajustá los valores — al estar montado en `/app/config/config.yaml` dentro del
contenedor, tomará prioridad sobre las variables de entorno del compose:

```yaml
server:
  host: "0.0.0.0"
  port: 5887

database:
  sqlite_path: "mvtrs.db"
  redis_url: "redis://redis:6379"

postgres_databases:
  pool_min: 5
  pool_max: 20
  default: "postgres://mvtuser:mvtpass@postgres:5432/mvtdb"

security:
  jwt_secret: "supersecretjwt-changeme-in-production"
  session_secret: "supersecretsession-changeme-in-production"

paths:
  config: "/app/config"
  cache: "/app/cache"
  assets: "/app/map_assets"
```

### Personalización

Para personalizar valores puntuales sin tocar el `Dockerfile` ni el `config.yaml`,
modificá directamente la sección `environment:` del servicio `mvt-server` en
`docker-example/docker-compose.yml` (por ejemplo, para cambiar los secretos de
JWT/sesión o la cadena de conexión a Postgres).

## Acceso a los servicios

Una vez iniciados los servicios:

- **MVT-RS**: http://localhost:5880
- **Adminer**: http://localhost:8080
- **PostgreSQL**: localhost:5432
- **Redis**: localhost:6379

## Configuración de datos geoespaciales

### Conectar a PostgreSQL y configurar datos

1. Accede a Adminer: http://localhost:8080
   - **Sistema**: PostgreSQL
   - **Servidor**: postgres
   - **Usuario**: mvtuser
   - **Contraseña**: mvtpass
   - **Base de datos**: mvtdb

2. Crear tabla de ejemplo:
```sql
-- Crear tabla de ejemplo
CREATE TABLE public.example_points (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    description TEXT,
    geom GEOMETRY(POINT, 4326)
);

-- Insertar datos de ejemplo
INSERT INTO public.example_points (name, description, geom) VALUES 
('Punto 1', 'Descripción del punto 1', ST_GeomFromText('POINT(-58.3816 -34.6037)', 4326)),
('Punto 2', 'Descripción del punto 2', ST_GeomFromText('POINT(-58.3700 -34.6100)', 4326));

-- Crear índice espacial
CREATE INDEX idx_example_points_geom ON public.example_points USING GIST (geom);
```

### Configurar capa en MVT-RS

1. Accede a la interfaz web: http://localhost:5880
2. Configura una nueva capa con los datos de ejemplo
3. Publica la capa como vector tiles

## Volúmenes persistentes

Los datos se mantienen en volúmenes Docker:

- `postgres_data`: Datos de PostgreSQL
- `redis_data`: Datos de Redis
- `./config`: Configuraciones de MVT-RS
- `./cache`: Caché de tiles
- `./map_assets`: Recursos de mapas

## Problemas comunes

### 1. Puerto ocupado
```bash
# Verificar puertos en uso
netstat -tlnp | grep :5880

# Cambiar el puerto externo en docker-compose.yml (el interno siempre es 5887)
ports:
  - "5881:5887"  # Cambiar 5880 por otro puerto externo
```

### 2. Problemas de permisos
```bash
# Dar permisos a directorios
chmod -R 755 config cache map_assets
```

### 3. Reiniciar servicios
```bash
# Reiniciar solo MVT-RS
docker-compose restart mvt-server

# Reiniciar todo
docker-compose restart
```

## Producción

Para producción, asegúrate de:

1. **Cambiar secretos** en `docker-compose.yml`
2. **Usar variables de entorno** en lugar de valores hardcodeados
3. **Configurar proxy reverso** (nginx, traefik)
4. **Habilitar SSL/TLS**
5. **Configurar backups** de PostgreSQL
6. **Monitorear recursos** y logs

### Ejemplo de producción con nginx

```yaml
# docker-compose.prod.yml (parado en docker-example/)
services:
  mvt-server:
    build:
      context: ..
      dockerfile: docker-example/Dockerfile
    environment:
      MVT_POSTGRES_DATABASES__DEFAULT: "${DB_CONNECTION_STRING}"
      MVT_SECURITY__JWT_SECRET: "${JWT_SECRET}"
      MVT_SECURITY__SESSION_SECRET: "${SESSION_SECRET}"
    networks:
      - internal
    # No exponer puertos directamente
    
  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
      - ./certs:/etc/nginx/certs:ro
    depends_on:
      - mvt-server
    networks:
      - internal

networks:
  internal:
    driver: bridge
```

## Soporte

Si tienes problemas:

1. Revisa los logs: `docker-compose logs -f`
2. Verifica el estado: `docker-compose ps`
3. Consulta la documentación original del proyecto
4. Abre un issue en el repositorio del proyecto
