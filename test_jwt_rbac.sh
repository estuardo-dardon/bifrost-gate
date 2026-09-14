#!/bin/bash
# Script de pruebas de integración para Autenticación Híbrida y JWT RBAC en Bifröst Gate.
# Copyright (C) 2026 Estuardo Dardón.

set -e

PORT=8089
DB_FILE="test_gate.db"

echo "=== 1. Limpiando ambiente de prueba ==="
rm -f "$DB_FILE" "$DB_FILE-shm" "$DB_FILE-wal"

echo "=== 3. Creando archivo de configuración temporal para pruebas ==="
cat << EOF > config.test.toml
[server]
host = "127.0.0.1"
port = 8089

[tls]
enabled = false
cert_path = "certs/cert.pem"
key_path = "certs/key.pem"

[auth]
enabled = true
header_name = "x-api-key"
bootstrap_user = "admin"
bootstrap_api_key = "debug-local-bootstrap-key-please-change-2026"

[auth.jwt]
enabled = true
default_secret = "shared-jwt-secret-between-gate-and-end-please-change"

[database]
db_path = "$(pwd)/$DB_FILE"

[logging]
service_level = 0
worker_level = 0
use_journalctl = false
EOF

echo "=== 4. Iniciando Bifröst Gate para crear base de datos ==="
BIFROST_CONFIG=config.test.toml ./target/debug/bifrost-gate > var/log/gate_init.log 2>&1 &
GATE_PID=$!

echo "Esperando a que la base de datos se inicialice..."
sleep 2

echo "Deteniendo el servidor temporalmente..."
kill $GATE_PID
wait $GATE_PID 2>/dev/null || true

echo "=== 5. Sembrando API Keys de prueba ==="
sqlite3 "$(pwd)/$DB_FILE" "INSERT INTO api_keys (user_name, api_key, is_active, allowed_scopes, jwt_secret) VALUES ('admin', 'key-all-permissions', 1, '[\"*\"]', 'jwt-secret-all');"
sqlite3 "$(pwd)/$DB_FILE" "INSERT INTO api_keys (user_name, api_key, is_active, allowed_scopes, jwt_secret) VALUES ('operator', 'key-restricted-permissions', 1, '[\"connections:read\"]', 'jwt-secret-restricted');"

echo "=== 6. Reiniciando Bifröst Gate con la base de datos sembrada ==="
BIFROST_CONFIG=config.test.toml ./target/debug/bifrost-gate > var/log/gate_test.log 2>&1 &
GATE_PID=$!

# Asegurar que matamos al servidor al salir del script
cleanup() {
    echo "Deteniendo Bifröst Gate..."
    kill $GATE_PID 2>/dev/null || true
    wait $GATE_PID 2>/dev/null || true
    rm -f config.test.toml
}
trap cleanup EXIT

echo "Esperando a que el servidor esté listo..."
sleep 2

# Generar los tokens
TOKEN_ALL=$(./target/debug/gen_token jwt-secret-all admin "*")
TOKEN_WRONG=$(./target/debug/gen_token wrong-secret admin "*")
TOKEN_RESTRICTED=$(./target/debug/gen_token jwt-secret-restricted operator "connections:read")

echo "=== 5. Ejecutando Pruebas de Integración ==="

echo -n "Test 1: Petición sin API Key -> "
CODE=$(curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:$PORT/api/connections)
if [ "$CODE" -eq 401 ]; then
    echo "OK (401)"
else
    echo "FALLÓ (Esperado 401, recibido $CODE)"
    exit 1
fi

echo -n "Test 2: Petición con API Key inválida -> "
CODE=$(curl -s -H "x-api-key: invalid-key" -o /dev/null -w "%{http_code}" http://127.0.0.1:$PORT/api/connections)
if [ "$CODE" -eq 401 ]; then
    echo "OK (401)"
else
    echo "FALLÓ (Esperado 401, recibido $CODE)"
    exit 1
fi

echo -n "Test 3: Petición con API Key válida pero sin Token JWT -> "
CODE=$(curl -s -H "x-api-key: key-all-permissions" -o /dev/null -w "%{http_code}" http://127.0.0.1:$PORT/api/connections)
if [ "$CODE" -eq 401 ]; then
    echo "OK (401)"
else
    echo "FALLÓ (Esperado 401, recibido $CODE)"
    exit 1
fi

echo -n "Test 4: Petición con API Key válida + Token JWT firmado con clave incorrecta -> "
CODE=$(curl -s -H "x-api-key: key-all-permissions" -H "Authorization: Bearer $TOKEN_WRONG" -o /dev/null -w "%{http_code}" http://127.0.0.1:$PORT/api/connections)
if [ "$CODE" -eq 401 ]; then
    echo "OK (401)"
else
    echo "FALLÓ (Esperado 401, recibido $CODE)"
    exit 1
fi

echo -n "Test 5: Petición con API Key válida + Token JWT válido (Todos los permisos) -> "
CODE=$(curl -s -H "x-api-key: key-all-permissions" -H "Authorization: Bearer $TOKEN_ALL" -o /dev/null -w "%{http_code}" http://127.0.0.1:$PORT/api/connections)
if [ "$CODE" -eq 200 ] || [ "$CODE" -eq 501 ]; then
    echo "OK ($CODE)"
else
    echo "FALLÓ (Esperado 200/501, recibido $CODE)"
    exit 1
fi

echo -n "Test 6: Petición de lectura con API Key restringida + Token JWT válido -> "
CODE=$(curl -s -H "x-api-key: key-restricted-permissions" -H "Authorization: Bearer $TOKEN_RESTRICTED" -o /dev/null -w "%{http_code}" http://127.0.0.1:$PORT/api/connections)
if [ "$CODE" -eq 200 ] || [ "$CODE" -eq 501 ]; then
    echo "OK ($CODE)"
else
    echo "FALLÓ (Esperado 200/501, recibido $CODE)"
    exit 1
fi

echo -n "Test 7: Petición de escritura con API Key restringida + Token JWT válido (Debe retornar 403) -> "
CODE=$(curl -s -X POST -H "x-api-key: key-restricted-permissions" -H "Authorization: Bearer $TOKEN_RESTRICTED" -H "Content-Type: application/json" -d '{"name":"test-conn","config":{}}' -o /dev/null -w "%{http_code}" http://127.0.0.1:$PORT/api/connections)
if [ "$CODE" -eq 403 ]; then
    echo "OK (403)"
else
    echo "FALLÓ (Esperado 403, recibido $CODE)"
    exit 1
fi

echo "=== ¡Todas las pruebas de JWT RBAC pasaron exitosamente! ==="
