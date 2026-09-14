#!/bin/bash
set -e

DB_FILE="/var/lib/bifrost/bifrost.db"

# 1. Asegurar que el directorio de la base de datos existe
mkdir -p /var/lib/bifrost

# 2. Inicializar la estructura de la base de datos si no existe o si está vacía
if [ ! -f "$DB_FILE" ]; then
    echo "[Entrypoint] La base de datos '$DB_FILE' no existe. Inicializando esquema y usuarios por defecto..."
    # Ejecutamos bifrost-gate brevemente o dejamos que cree las tablas al arrancar
fi

# 3. Iniciar el servicio en segundo plano un momento para aplicar migraciones si es necesario
/app/bifrost-gate &
PID=$!

echo "[Entrypoint] Esperando a que el esquema de la base de datos esté listo..."
for i in {1..30}; do
    if [ -f "$DB_FILE" ]; then
        # Verificar si las tablas existen
        TABLES=$(sqlite3 "$DB_FILE" "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='docs_users';" 2>/dev/null || echo "0")
        if [ "$TABLES" -ge 1 ]; then
            echo "[Entrypoint] Base de datos y tablas inicializadas."
            break
        fi
    fi
    sleep 1
done

# Detener la instancia temporal de bifrost-gate para realizar la siembra de usuarios de forma segura
kill $PID 2>/dev/null || true
wait $PID 2>/dev/null || true

# 4. Verificar y sembrar usuario de Documentación (/api/docs, /howto) si no existe
DOCS_USER_COUNT=$(sqlite3 "$DB_FILE" "SELECT COUNT(*) FROM docs_users WHERE username='docsadmin';" 2>/dev/null || echo "0")
if [ "$DOCS_USER_COUNT" -eq 0 ]; then
    echo "[Entrypoint] Creando usuario por defecto para documentación ('docsadmin')..."
    /app/bifrostctl docs-user create docsadmin DocsPassword123
    /app/bifrostctl docs-user grant-responses-manage docsadmin
else
    echo "[Entrypoint] Usuario de documentación 'docsadmin' ya existe."
fi

# 5. Verificar y sembrar usuario de Métricas / Administración (/metrics) si no existe
API_USER_COUNT=$(sqlite3 "$DB_FILE" "SELECT COUNT(*) FROM api_users WHERE username='prometheus';" 2>/dev/null || echo "0")
if [ "$API_USER_COUNT" -eq 0 ]; then
    echo "[Entrypoint] Creando usuario por defecto para métricas/Prometheus ('prometheus')..."
    /app/bifrostctl api-user create prometheus SecretPassword123
else
    echo "[Entrypoint] Usuario de métricas 'prometheus' ya existe."
fi

echo "[Entrypoint] Verificación de base de datos completada. Iniciando Bifröst Gate..."
exec /app/bifrost-gate
