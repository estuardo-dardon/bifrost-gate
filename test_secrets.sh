#!/bin/bash
# Script para probar el ciclo completo de la API de Secrets en Bifröst Gate.
#
# Uso:
#   ./test_secrets.sh --api-key <TU_API_KEY>
#   o exportando la variable BIFROST_API_KEY:
#   export BIFROST_API_KEY="bfg_..." && ./test_secrets.sh

set -e

BASE_URL="http://localhost:2704"
API_KEY="${BIFROST_API_KEY}"

# Parsear argumentos
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --api-key) API_KEY="$2"; shift ;;
        --base-url) BASE_URL="$2"; shift ;;
        *) echo "Parámetro desconocido: $1"; exit 1 ;;
    esac
    shift
done

if [ -z "$API_KEY" ]; then
    echo "⚠️  ADVERTENCIA: No se especificó --api-key ni BIFROST_API_KEY."
    echo "Usando API key de prueba autogenerada (probablemente retornará 401 Unauthorized a menos que uses bypass)."
    API_KEY="bfg_$(uuidgen | tr '[:upper:]' '[:lower:]' | sed 's/-//g')"
fi

echo "============================================="
echo "       TEST DE CICLO CRUD DE SECRETS"
echo "============================================="
echo "Base URL: $BASE_URL"
echo "API Key:  ${API_KEY:0:8}...${API_KEY: -4}"
echo "============================================="
echo

# Función helper para llamadas HTTP
call_api() {
    local method="$1"
    local path="$2"
    local data="$3"
    
    if [ -n "$data" ]; then
        curl -s -X "$method" "$BASE_URL$path" \
             -H "Content-Type: application/json" \
             -H "x-api-key: $API_KEY" \
             -d "$data"
    else
        curl -s -X "$method" "$BASE_URL$path" \
             -H "x-api-key: $API_KEY"
    fi
}

# 1. Crear un secreto IKE válido (POST)
echo "1. Creando secreto IKE válido..."
IKE_PAYLOAD='{
  "secret_type": "ike",
  "config": {
    "secret": "PasswordSegura123!",
    "ids": ["1.1.1.1", "2.2.2.2"]
  }
}'

CREATE_RES=$(call_api "POST" "/api/secrets/test-ike-crud" "$IKE_PAYLOAD")
echo "Respuesta POST:"
echo "$CREATE_RES" | jq . 2>/dev/null || echo "$CREATE_RES"
echo

# 2. Listar secretos (GET /api/secrets)
echo "2. Listando secretos administrados..."
LIST_RES=$(call_api "GET" "/api/secrets")
echo "Respuesta GET /api/secrets:"
echo "$LIST_RES" | jq . 2>/dev/null || echo "$LIST_RES"
echo

# 3. Leer el secreto creado (GET /api/secrets/test-ike-crud)
echo "3. Leyendo el secreto creado (debe tener el secret redacted)..."
READ_RES=$(call_api "GET" "/api/secrets/test-ike-crud")
echo "Respuesta GET /api/secrets/test-ike-crud:"
echo "$READ_RES" | jq . 2>/dev/null || echo "$READ_RES"
echo

# 4. Actualizar el secreto (PUT /api/secrets/test-ike-crud)
echo "4. Actualizando el secreto (cambiando IDs y secret)..."
UPDATE_PAYLOAD='{
  "secret_type": "ike",
  "config": {
    "secret": "NuevaPasswordSegura987!",
    "id": "3.3.3.3"
  }
}'

UPDATE_RES=$(call_api "PUT" "/api/secrets/test-ike-crud" "$UPDATE_PAYLOAD")
echo "Respuesta PUT:"
echo "$UPDATE_RES" | jq . 2>/dev/null || echo "$UPDATE_RES"
echo

# 5. Volver a leer para verificar round-trip de ID normalizado a single "id"
echo "5. Verificando normalización del ID en lectura..."
READ_AGAIN_RES=$(call_api "GET" "/api/secrets/test-ike-crud")
echo "Respuesta GET (verificar si 'id' es string y no array de 1 elemento):"
echo "$READ_AGAIN_RES" | jq . 2>/dev/null || echo "$READ_AGAIN_RES"
echo

# 6. Eliminar el secreto (DELETE /api/secrets/test-ike-crud)
echo "6. Eliminando el secreto..."
DELETE_RES=$(call_api "DELETE" "/api/secrets/test-ike-crud")
echo "Respuesta DELETE:"
echo "$DELETE_RES" | jq . 2>/dev/null || echo "$DELETE_RES"
echo

# 7. Pruebas de Validación
echo "=== PRUEBAS DE VALIDACIÓN ==="
echo

# Validación: Secret muy corto (< 8 chars)
echo "7a. Validación: Secret de longitud menor a 8 caracteres..."
SHORT_PAYLOAD='{
  "secret_type": "ike",
  "config": {
    "secret": "123",
    "id": "1.1.1.1"
  }
}'
SHORT_RES=$(call_api "POST" "/api/secrets/test-short" "$SHORT_PAYLOAD")
echo "Respuesta (debe fallar indicando longitud mínima):"
echo "$SHORT_RES" | jq . 2>/dev/null || echo "$SHORT_RES"
echo

# Validación: Falta de ID en IKE
echo "7b. Validación: Sin ID para tipo IKE..."
NO_ID_PAYLOAD='{
  "secret_type": "ike",
  "config": {
    "secret": "PasswordSegura123!"
  }
}'
NO_ID_RES=$(call_api "POST" "/api/secrets/test-noid" "$NO_ID_PAYLOAD")
echo "Respuesta (debe fallar indicando requerimiento específico de ID/IDs por tipo):"
echo "$NO_ID_RES" | jq . 2>/dev/null || echo "$NO_ID_RES"
echo

# Validación: Caracteres inválidos en nombre
echo "7c. Validación: Caracteres inválidos en el nombre..."
INVALID_NAME_RES=$(call_api "POST" "/api/secrets/nombre_con_espacios%20y%20simbolos" "$IKE_PAYLOAD")
echo "Respuesta (debe fallar indicando caracteres no válidos):"
echo "$INVALID_NAME_RES" | jq . 2>/dev/null || echo "$INVALID_NAME_RES"
echo

echo "============================================="
echo "       PRUEBAS FINALIZADAS"
echo "============================================="