#!/bin/bash

# Script para incrementar automáticamente la versión del paquete
# Incrementa el número de release en Cargo.toml
# Ejemplo: 0.1.0-1 → 0.1.0-2

CARGO_FILE="/home/estuardodardon/workspace/app/bifrost/gate/Cargo.toml"

# Leer la versión actual
CURRENT_VERSION=$(grep '^version = ' "$CARGO_FILE" | head -1 | cut -d'"' -f2)

# Separar base (0.1.0) y release number (1, 2, etc)
if [[ "$CURRENT_VERSION" =~ ^(.*)-(.*) ]]; then
    BASE_VERSION="${BASH_REMATCH[1]}"
    RELEASE_NUM="${BASH_REMATCH[2]}"
else
    BASE_VERSION="$CURRENT_VERSION"
    RELEASE_NUM=0
fi

# Incrementar número de release
NEW_RELEASE_NUM=$((RELEASE_NUM + 1))
NEW_VERSION="${BASE_VERSION}-${NEW_RELEASE_NUM}"

# Actualizar Cargo.toml
sed -i "0,/^version = \"$CURRENT_VERSION\"/s//version = \"$NEW_VERSION\"/" "$CARGO_FILE"

# Solo output la nueva versión para el script llamador
echo "$NEW_VERSION"
