#!/bin/bash
# Script para configurar permisos de logging en desarrollo
# Uso: ./setup_logging.sh

echo "🔧 Configurando sistema de logging para Bifröst-Gate"
echo ""

# Verificar si se ejecuta con permisos de administrador
if [[ $EUID -ne 0 ]]; then
   echo "⚠️  Este script necesita permisos de administrador (sudo)"
   echo "   Ejecuta: sudo ./setup_logging.sh"
   exit 1
fi

# Crear directorio de logs
echo "📁 Creando directorio /var/log..."
mkdir -p /var/log
chmod 755 /var/log

# Crear archivos de log
echo "📝 Creando archivos de log..."
touch /var/log/bifrost/service.log
touch /var/log/bifrost/worker.log

# Establecer permisos apropiados
echo "🔐 Estableciendo permisos..."
chmod 666 /var/log/bifrost/service.log
chmod 666 /var/log/bifrost/worker.log

# Opcional: cambiar propietario si es necesario
# Obtener el usuario actual (sin sudo)
ORIGINAL_USER="${SUDO_USER:-$(whoami)}"
if [ "$ORIGINAL_USER" != "root" ]; then
    echo "👤 Cambiando propietario a $ORIGINAL_USER..."
    chown "$ORIGINAL_USER:$ORIGINAL_USER" /var/log/bifrost/service.log
    chown "$ORIGINAL_USER:$ORIGINAL_USER" /var/log/bifrost/worker.log
fi

# Verificar journalctl
echo ""
echo "🔍 Verificando disponibilidad de journalctl..."
if command -v journalctl &> /dev/null; then
    echo "✅ journalctl está disponible"
    echo "   Los logs se enviarán a systemd"
else
    echo "ℹ️  journalctl no disponible"
    echo "   Los logs se escribirán a archivos"
fi

# Verificar logger command
echo ""
echo "🔍 Verificando comando logger..."
if command -v logger &> /dev/null; then
    echo "✅ logger está disponible"
else
    echo "⚠️  logger no disponible"
    echo "   Instala: sudo apt-get install bsd-mailx (o similar)"
fi

echo ""
echo "✅ Configuración completada!"
echo ""
echo "📋 Estado de los archivos:"
ls -la /var/log/bifrost/*.log 2>/dev/null || echo "   Archivos aún no creados (se crearán en tiempo de ejecución)"

echo ""
echo ""
echo "📊 Para ver los logs:"
echo "   journalctl -t bifrost-gate -f     # Si journalctl está disponible"
echo "   tail -f /var/log/bifrost/*.log    # O usa los archivos"
