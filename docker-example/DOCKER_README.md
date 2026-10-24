🌐 **English** | [Español](DOCKER_README.es.md)

# MVT-Server Docker Setup

This directory includes a complete Docker setup to run MVT-RS with PostgreSQL+PostGIS and Redis.

## Requirements

- Docker
- Docker Compose

## File layout

```
mvt-rs/
├── .dockerignore              # Files excluded from the build (repo root)
├── src/, Cargo.toml, ...      # Source code (build context)
└── docker-example/
    ├── Dockerfile              # Multi-stage image for MVT-RS
    ├── docker-compose.yml      # Service orchestration
    ├── DOCKER_README.md        # This file
    ├── config/                 # Application config (created on first run)
    ├── cache/                  # Tile cache directory (created on first run)
    └── map_assets/              # Map assets (created on first run)
```

Every command in this document is run from inside `docker-example/`.
`docker-compose.yml` points its build context at the repo root
(`context: ..`), so there's no need to move or copy any files.

## Included services

### 1. mvt-server
- **Internal port**: 5887 (mapped to 5880 externally in docker-compose.yml)
- **Description**: The main MVT-RS application
- **Depends on**: PostgreSQL, Redis

### 2. postgres
- **Port**: 5432
- **Image**: postgis/postgis:15-3.4
- **Database**: mvtdb
- **User**: mvtuser
- **Password**: mvtpass

### 3. redis
- **Port**: 6379
- **Image**: redis:7-alpine
- **Purpose**: Tile cache

### 4. adminer (optional)
- **Port**: 8080
- **Purpose**: Web-based database admin

## Basic commands

### Start all services
```bash
docker-compose up -d
```

### View logs
```bash
# All services
docker-compose logs -f

# MVT-RS only
docker-compose logs -f mvt-server

# PostgreSQL only
docker-compose logs -f postgres
```

### Stop services
```bash
docker-compose down
```

### Rebuild the application
```bash
docker-compose build --no-cache mvt-server
docker-compose up -d
```

### Access a container
```bash
# MVT-RS
docker-compose exec mvt-server bash

# PostgreSQL
docker-compose exec postgres psql -U mvtuser -d mvtdb
```

## Configuration

MVT Server supports both a centralized `config.yaml` file and environment
variables, and the two can be combined: the priority order is `config.yaml` >
environment variables > defaults. This example's `docker-compose.yml`
configures everything through environment variables (the `environment:`
section of the `mvt-server` service) so it works out of the box with no
extra steps.

If you'd rather centralize configuration in a file instead of env vars, copy
`config.example.yaml` (at the repo root) to `docker-example/config/config.yaml`
and adjust the values — since it's mounted at `/app/config/config.yaml` inside
the container, it takes priority over the environment variables set in the
compose file:

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

### Customization

To tweak individual values without touching the `Dockerfile` or a
`config.yaml`, edit the `environment:` section of the `mvt-server` service
directly in `docker-example/docker-compose.yml` (for example, to change the
JWT/session secrets or the Postgres connection string).

## Accessing the services

Once the services are up:

- **MVT-RS**: http://localhost:5880
- **Adminer**: http://localhost:8080
- **PostgreSQL**: localhost:5432
- **Redis**: localhost:6379

## Setting up geospatial data

### Connect to PostgreSQL and configure data

1. Open Adminer: http://localhost:8080
   - **System**: PostgreSQL
   - **Server**: postgres
   - **User**: mvtuser
   - **Password**: mvtpass
   - **Database**: mvtdb

2. Create a sample table:
```sql
-- Create a sample table
CREATE TABLE public.example_points (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    description TEXT,
    geom GEOMETRY(POINT, 4326)
);

-- Insert sample data
INSERT INTO public.example_points (name, description, geom) VALUES 
('Point 1', 'Description of point 1', ST_GeomFromText('POINT(-58.3816 -34.6037)', 4326)),
('Point 2', 'Description of point 2', ST_GeomFromText('POINT(-58.3700 -34.6100)', 4326));

-- Create a spatial index
CREATE INDEX idx_example_points_geom ON public.example_points USING GIST (geom);
```

### Configure a layer in MVT-RS

1. Open the web UI: http://localhost:5880
2. Configure a new layer using the sample data
3. Publish the layer as vector tiles

## Persistent volumes

Data is kept in Docker volumes:

- `postgres_data`: PostgreSQL data
- `redis_data`: Redis data
- `./config`: MVT-RS configuration
- `./cache`: Tile cache
- `./map_assets`: Map assets

## Common issues

### 1. Port already in use
```bash
# Check which ports are in use
netstat -tlnp | grep :5880

# Change the external port in docker-compose.yml (the internal one is always 5887)
ports:
  - "5881:5887"  # Change 5880 to another external port
```

### 2. Permission issues
```bash
# Grant permissions on the directories
chmod -R 755 config cache map_assets
```

### 3. Restart services
```bash
# Restart MVT-RS only
docker-compose restart mvt-server

# Restart everything
docker-compose restart
```

## Production

For production, make sure to:

1. **Change the secrets** used in `docker-compose.yml`
2. **Use environment variables** instead of hardcoded values
3. **Set up a reverse proxy** (nginx, traefik)
4. **Enable SSL/TLS**
5. **Set up PostgreSQL backups**
6. **Monitor resources** and logs

### Production example with nginx

```yaml
# docker-compose.prod.yml (run from docker-example/)
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
    # Don't expose ports directly
    
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

## Support

If you run into trouble:

1. Check the logs: `docker-compose logs -f`
2. Check the status: `docker-compose ps`
3. Check the project's main documentation
4. Open an issue in the project's repository
