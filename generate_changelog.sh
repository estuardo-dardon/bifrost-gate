#!/bin/bash

# Script para generar changelog automáticamente antes de cada release
# Basado en git commits con formato Conventional Commits

CONFIG_FILE="/home/estuardodardon/workspace/app/bifrost/gate/cliff.toml"
OUTPUT_FILE="/home/estuardodardon/workspace/app/bifrost/gate/CHANGELOG.md"

echo "📝 Generando CHANGELOG automáticamente..."

# Generar changelog usando git-cliff
git-cliff --config "$CONFIG_FILE" > "$OUTPUT_FILE"

if [ $? -eq 0 ]; then
    echo "✓ CHANGELOG.md actualizado exitosamente"
    # Mostrar primeras líneas del changelog
    head -15 "$OUTPUT_FILE"
else
    echo "✗ Error al generar changelog"
    exit 1
fi
