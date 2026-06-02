# Multi-stage build para optimizar el tamaño final
FROM rust:1.95 as builder

# Instalar dependencias del sistema
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Establecer directorio de trabajo
WORKDIR /app

# Copiar archivos de dependencias
COPY Cargo.toml Cargo.lock ./
# Copiar build.rs si existe
COPY build.rs* ./

# Crear directorio src dummy para cachear dependencias
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Compilar dependencias (cacheable)
RUN cargo build --release && rm -rf src

# Copiar código fuente
COPY src ./src
COPY migrations ./migrations
COPY templates ./templates
COPY static ./static
COPY locales ./locales

RUN rm -f target/release/mvt-server target/release/deps/mvt_server* target/release/deps/mvt-server*

# Compilar aplicación
RUN touch src/main.rs && cargo build --release

# Etapa final - imagen más pequeña
FROM debian:bookworm-slim

# Instalar dependencias runtime
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Crear usuario no-root
RUN useradd -m -s /bin/bash mvtuser

# Establecer directorio de trabajo
WORKDIR /app

# Copiar binario compilado
COPY --from=builder /app/target/release/mvt-server .

# Copiar archivos necesarios
COPY --from=builder /app/templates ./templates
COPY --from=builder /app/static ./static
COPY --from=builder /app/locales ./locales
COPY --from=builder /app/migrations ./migrations

# Crear directorios necesarios
RUN mkdir -p config cache map_assets

# Cambiar propiedad a mvtuser
RUN chown -R mvtuser:mvtuser /app

# Cambiar a usuario no-root
USER mvtuser

# Exponer puerto
EXPOSE 5887

# Variables de entorno por defecto
ENV MVT_SERVER__HOST=0.0.0.0
ENV MVT_SERVER__PORT=5887
ENV MVT_PATHS__CONFIG=/app/config
ENV MVT_PATHS__CACHE=/app/cache
ENV MVT_PATHS__ASSETS=/app/map_assets
ENV MVT_DATABASE__POOL_MIN=5
ENV MVT_DATABASE__POOL_MAX=20

# Comando para ejecutar la aplicación
CMD ["./mvt-server"]
