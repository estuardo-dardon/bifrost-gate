# Multi-stage Dockerfile para Pruebas e Integración de Bifröst Gate con StrongSwan
FROM rust:slim-bookworm AS builder

WORKDIR /usr/src/bifrost-gate

# Instalar dependencias de compilación
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    sqlite3 \
    libsqlite3-dev \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copiar código fuente
COPY . .

# Compilar los binarios principales y auxiliares
RUN cargo build --bin bifrost-gate --bin gen_token --bin bifrostctl

# --- Etapa Final / Runner ---
FROM debian:bookworm-slim

ENV DEBIAN_FRONTEND=noninteractive

# Instalar StrongSwan, swanctl, sqlite3, curl, bash y utilidades de depuración (incluyendo joe como editor según reglas del proyecto)
RUN apt-get update && apt-get install -y --no-install-recommends \
    strongswan \
    strongswan-swanctl \
    charon-systemd \
    sqlite3 \
    curl \
    ca-certificates \
    iproute2 \
    iptables \
    net-tools \
    joe \
    && rm -rf /var/lib/apt/lists/*

# Crear directorios necesarios para StrongSwan (swanctl) y Bifröst
RUN mkdir -p /etc/swanctl/conf.d \
    /etc/swanctl/x509 \
    /etc/swanctl/x509ca \
    /etc/swanctl/private \
    /var/lib/bifrost \
    /var/lib/bifrost/tmp \
    /var/log/bifrost

WORKDIR /app

# Copiar los binarios construidos desde la etapa anterior
COPY --from=builder /usr/src/bifrost-gate/target/debug/bifrost-gate /app/bifrost-gate
COPY --from=builder /usr/src/bifrost-gate/target/debug/gen_token /app/gen_token
COPY --from=builder /usr/src/bifrost-gate/target/debug/bifrostctl /app/bifrostctl
COPY --from=builder /usr/src/bifrost-gate/test_jwt_rbac.sh /app/test_jwt_rbac.sh

COPY docker-entrypoint.sh /app/docker-entrypoint.sh

# Dar permisos de ejecución
RUN chmod +x /app/bifrost-gate /app/gen_token /app/bifrostctl /app/test_jwt_rbac.sh /app/docker-entrypoint.sh

# Crear configuración por defecto para entorno containerizado
RUN cat << 'EOF' > /app/config.toml
[server]
host = "0.0.0.0"
port = 2704

[tls]
enabled = false
cert_path = ""
key_path = ""

[auth]
enabled = true
header_name = "x-api-key"
bootstrap_user = "admin"
bootstrap_api_key = "docker-debug-bootstrap-key-2026"

[auth.jwt]
enabled = true
default_secret = "docker-jwt-secret-shared-with-gate"

[database]
db_path = "/var/lib/bifrost/bifrost.db"

[strongswan]
lock_path = "/var/lib/bifrost/swanctl.lock"
tmp_dir = "/var/lib/bifrost/tmp"
conf_dir = "/etc/swanctl"

[logging]
service_level = 1
worker_level = 1
service_access_log = "/var/log/bifrost/service-access.log"
service_error_log = "/var/log/bifrost/service-error.log"
worker_log = "/var/log/bifrost/worker.log"
use_journalctl = false
channel_capacity = 1000
rotate_size_mb = 10
EOF

# Exponer el puerto de la API / Métricas
EXPOSE 2704

# Script de arranque con verificación e inicialización automática de DB/usuarios
ENTRYPOINT ["/app/docker-entrypoint.sh"]
