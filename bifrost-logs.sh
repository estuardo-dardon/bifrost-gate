#!/usr/bin/env bash
# Script de monitoreo de logs de Bifröst-Gate
# Uso: ./monitor_logs.sh [option]

set -e

# Colores
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Rutas de logs (personalizar según config.toml)
ACCESS_LOG="${1:-/var/log/bifrost-service-access.log}"
ERROR_LOG="${2:-/var/log/bifrost-service-error.log}"
WORKER_LOG="${3:-/var/log/bifrost-worker.log}"

# Funciones de utilidad
print_header() {
    echo -e "${BLUE}════════════════════════════════════════${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}════════════════════════════════════════${NC}"
}

print_section() {
    echo -e "${YELLOW}▶ $1${NC}"
}

# Verificar que los archivos existen
check_logs() {
    print_header "Verificando Archivos de Log"
    
    for log in "$ACCESS_LOG" "$ERROR_LOG" "$WORKER_LOG"; do
        if [ -f "$log" ]; then
            size=$(du -h "$log" | cut -f1)
            lines=$(wc -l < "$log")
            echo -e "${GREEN}✓${NC} $log ($size, $lines líneas)"
        else
            echo -e "${RED}✗${NC} $log (NO ENCONTRADO)"
        fi
    done
    echo
}

# Mostrar estadísticas
show_stats() {
    print_header "Estadísticas de Logs"
    
    print_section "Access Log"
    echo "Total de requests: $(grep -c "API_REQUEST" "$ACCESS_LOG" 2>/dev/null || echo "0")"
    echo "Líneas totales: $(wc -l < "$ACCESS_LOG")"
    echo "Tamaño: $(du -h "$ACCESS_LOG" | cut -f1)"
    echo
    
    print_section "Error Log"
    total_errors=$(grep -c "\[ERROR\]\|\[EXCEPTION\]" "$ERROR_LOG" 2>/dev/null || echo "0")
    echo "Total de errores: $total_errors"
    echo "Líneas totales: $(wc -l < "$ERROR_LOG")"
    echo "Tamaño: $(du -h "$ERROR_LOG" | cut -f1)"
    echo
    
    print_section "Worker Log"
    echo "Eventos del worker: $(grep -c "WORKER" "$WORKER_LOG" 2>/dev/null || echo "0")"
    echo "Líneas totales: $(wc -l < "$WORKER_LOG")"
    echo "Tamaño: $(du -h "$WORKER_LOG" | cut -f1)"
    echo
}

# Mostrar últimos eventos
show_recent() {
    print_header "Últimos Eventos"
    
    print_section "Últimas 5 líneas de Access Log"
    tail -5 "$ACCESS_LOG"
    echo
    
    print_section "Últimas 5 líneas de Error Log"
    tail -5 "$ERROR_LOG"
    echo
    
    print_section "Últimas 5 líneas de Worker Log"
    tail -5 "$WORKER_LOG"
    echo
}

# Mostrar resumen de errores
show_errors() {
    print_header "Resumen de Errores"
    
    echo -e "${YELLOW}Errores por tipo:${NC}"
    echo "ERROR: $(grep -c "\[ERROR\]" "$ERROR_LOG" 2>/dev/null || echo "0")"
    echo "EXCEPTION: $(grep -c "\[EXCEPTION\]" "$ERROR_LOG" 2>/dev/null || echo "0")"
    echo
    
    echo -e "${YELLOW}Primeros 5 errores:${NC}"
    head -5 "$ERROR_LOG"
    echo
}

# Analizar requests por endpoint
analyze_requests() {
    print_header "Análisis de Requests"
    
    echo -e "${YELLOW}Total de requests:${NC}"
    grep -c "API_REQUEST" "$ACCESS_LOG" 2>/dev/null || echo "0"
    echo
    
    echo -e "${YELLOW}Requests por endpoint:${NC}"
    grep "API_REQUEST" "$ACCESS_LOG" 2>/dev/null | \
        awk -F'API_REQUEST: ' '{print $2}' | \
        awk '{print $1" "$2}' | \
        sort | uniq -c | sort -rn || echo "No hay data"
    echo
    
    echo -e "${YELLOW}Status codes:${NC}"
    grep "Status:" "$ACCESS_LOG" 2>/dev/null | \
        awk -F'Status: ' '{print $2}' | \
        awk '{print $1}' | \
        sort | uniq -c | sort -rn || echo "No hay data"
    echo
}

# Monitoreo en tiempo real
monitor_live() {
    print_header "Monitoreo en Tiempo Real"
    echo -e "${YELLOW}Presiona Ctrl+C para salir${NC}"
    echo
    
    # Crear un archivo temporal para FIFO
    ACCESS_FIFO=$(mktemp -u)
    ERROR_FIFO=$(mktemp -u)
    WORKER_FIFO=$(mktemp -u)
    
    mkfifo "$ACCESS_FIFO" "$ERROR_FIFO" "$WORKER_FIFO"
    
    # Iniciar tail en cada FIFO
    tail -f "$ACCESS_LOG" > "$ACCESS_FIFO" 2>/dev/null &
    tail -f "$ERROR_LOG" > "$ERROR_FIFO" 2>/dev/null &
    tail -f "$WORKER_LOG" > "$WORKER_FIFO" 2>/dev/null &
    
    # Mostrar en tres columnas
    paste -d '|' <(sed 's/^/ACCESS: /g' "$ACCESS_FIFO") \
                  <(sed 's/^/ERROR:  /g' "$ERROR_FIFO") \
                  <(sed 's/^/WORKER: /g' "$WORKER_FIFO")
    
    # Limpiar
    rm -f "$ACCESS_FIFO" "$ERROR_FIFO" "$WORKER_FIFO"
}

# Menú principal
show_menu() {
    echo -e "${GREEN}╔════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║   Bifröst-Gate - Monitor de Logs       ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════╝${NC}"
    echo
    echo "1) Verificar archivos de log"
    echo "2) Ver estadísticas"
    echo "3) Ver eventos recientes"
    echo "4) Ver resumen de errores"
    echo "5) Analizar requests"
    echo "6) Monitoreo en tiempo real (Access)"
    echo "7) Monitoreo en tiempo real (Error)"
    echo "8) Monitoreo en tiempo real (Worker)"
    echo "9) Todo lo anterior"
    echo "0) Salir"
    echo
}

# Main
case "${1:-menu}" in
    1|check)
        check_logs
        ;;
    2|stats)
        check_logs
        show_stats
        ;;
    3|recent)
        show_recent
        ;;
    4|errors)
        show_errors
        ;;
    5|analyze)
        analyze_requests
        ;;
    6|monitor-access)
        print_header "Monitoreo: Access Log"
        tail -f "$ACCESS_LOG"
        ;;
    7|monitor-error)
        print_header "Monitoreo: Error Log"
        tail -f "$ERROR_LOG"
        ;;
    8|monitor-worker)
        print_header "Monitoreo: Worker Log"
        tail -f "$WORKER_LOG"
        ;;
    9|all)
        check_logs
        show_stats
        show_recent
        show_errors
        analyze_requests
        ;;
    menu)
        show_menu
        echo -n "Selecciona opción (0-9): "
        read -r option
        echo
        case "$option" in
            0) echo -e "${GREEN}Saliendo...${NC}"; exit 0 ;;
            *) $0 "$option" ;;
        esac
        ;;
    *)
        echo -e "${RED}Opción no válida${NC}"
        echo "Uso: $0 [1-9|menu|check|stats|recent|errors|analyze|monitor-access|monitor-error|monitor-worker|all]"
        exit 1
        ;;
esac
