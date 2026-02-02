#!/bin/bash

# Script para generar paquetes .deb y .rpm de Bifröst-Gate

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

VERSION="0.1.0"
PACKAGE_NAME="bifrost-gate"

echo -e "${BLUE}=== Bifröst-Gate: Generador de Paquetes ===${NC}\n"

# Compilar en release
echo -e "${BLUE}1. Compilando en modo release...${NC}"
cargo build --release 2>&1 | grep -E "(Compiling bifrost|Finished)"

# Verificar que el binario existe
if [ ! -f target/release/bifrost-gate ]; then
    echo -e "${YELLOW}Error: Binario no encontrado${NC}"
    exit 1
fi

BINARY_SIZE=$(du -h target/release/bifrost-gate | cut -f1)
echo -e "${GREEN}✓ Compilación exitosa (${BINARY_SIZE})${NC}\n"

# Crear directorio de trabajo
WORK_DIR=$(mktemp -d)
echo -e "${BLUE}2. Creando estructura para .deb en ${WORK_DIR}...${NC}"

DEB_ROOT="${WORK_DIR}/bifrost-deb"
mkdir -p "${DEB_ROOT}"/{usr/bin,etc/bifrost,lib/systemd/system,var/lib/bifrost,usr/share/doc/bifrost-gate}

# Copiar archivos
cp target/release/bifrost-gate "${DEB_ROOT}/usr/bin/"
strip "${DEB_ROOT}/usr/bin/bifrost-gate" 2>/dev/null
cp config.toml "${DEB_ROOT}/etc/bifrost/"
cp bifrost.service "${DEB_ROOT}/lib/systemd/system/"
chmod 600 "${DEB_ROOT}/etc/bifrost/config.toml"

# Crear directorio DEBIAN
mkdir -p "${DEB_ROOT}/DEBIAN"

# Control file
cat > "${DEB_ROOT}/DEBIAN/control" << 'EOF'
Package: bifrost-gate
Version: 0.1.0-1
Architecture: amd64
Maintainer: Estuardo Dardón <estuardo@example.com>
Description: Bifröst-Gate: Agente de monitoreo para StrongSwan
Homepage: https://github.com/estuardodardon/bifrost
Depends: libc6 (>= 2.38), sqlite3
Section: utils
Priority: optional
EOF

# Postinst script
cat > "${DEB_ROOT}/DEBIAN/postinst" << 'POSTINST'
#!/bin/sh
set -e
mkdir -p /var/lib/bifrost
chmod 700 /var/lib/bifrost
systemctl daemon-reload 2>/dev/null || true
systemctl enable bifrost-gate.service 2>/dev/null || true
echo "Bifröst-Gate instalado correctamente."
POSTINST

chmod 755 "${DEB_ROOT}/DEBIAN/postinst"

# Construir .deb
echo -e "${BLUE}3. Construyendo paquete Debian...${NC}"
mkdir -p target/debian 2>/dev/null || true
dpkg-deb --build "${DEB_ROOT}" target/debian/bifrost-gate_0.1.0-1_amd64.deb 2>&1 | grep -v "^$"

if [ -f target/debian/bifrost-gate_0.1.0-1_amd64.deb ]; then
    DEB_SIZE=$(du -h target/debian/bifrost-gate_0.1.0-1_amd64.deb | cut -f1)
    echo -e "${GREEN}✓ Paquete Debian creado: bifrost-gate_0.1.0-1_amd64.deb (${DEB_SIZE})${NC}"
else
    echo -e "${YELLOW}⚠ No se pudo crear el paquete .deb${NC}"
fi

# Preparar para RPM
echo -e "\n${BLUE}4. Preparando estructura RPM...${NC}"
RPM_DIR="${WORK_DIR}/rpm-build"
mkdir -p "${RPM_DIR}"/{SOURCES,SPECS,BUILD,RPMS,SRPMS}

cat > "${RPM_DIR}/SPECS/bifrost-gate.spec" << 'SPEC'
Name:           bifrost-gate
Version:        0.1.0
Release:        1%{?dist}
Summary:        Bifröst-Gate: Agente de monitoreo para StrongSwan
License:        AGPL-3.0-or-later
URL:            https://github.com/estuardodardon/bifrost

%description
Bifröst-Gate es un agente de monitoreo para plataformas VPN StrongSwan.

%prep

%build

%install
mkdir -p %{buildroot}/{usr/bin,etc/bifrost,lib/systemd/system,var/lib/bifrost}
cp /tmp/bifrost-build/bifrost-gate %{buildroot}/usr/bin/
cp /tmp/bifrost-build/config.toml %{buildroot}/etc/bifrost/
cp /tmp/bifrost-build/bifrost.service %{buildroot}/lib/systemd/system/

%files
/usr/bin/bifrost-gate
/etc/bifrost/config.toml
/lib/systemd/system/bifrost.service
%dir /var/lib/bifrost

%post
mkdir -p /var/lib/bifrost
chmod 700 /var/lib/bifrost
systemctl daemon-reload 2>/dev/null || true
systemctl enable bifrost-gate.service 2>/dev/null || true

%changelog
* Mon Feb 02 2026 Estuardo Dardón <estuardo@example.com> - 0.1.0-1
- Initial release
SPEC

cp target/release/bifrost-gate "${RPM_DIR}/SOURCES/"
cp config.toml "${RPM_DIR}/SOURCES/"
cp bifrost.service "${RPM_DIR}/SOURCES/"

echo -e "${GREEN}✓ Archivos RPM preparados${NC}"

echo -e "\n${GREEN}=== Resumen ===${NC}"
echo ""
echo "📦 Paquete Debian:"
echo "   Ubicación: target/debian/bifrost-gate_0.1.0-1_amd64.deb"
echo "   Instalar:  sudo dpkg -i target/debian/bifrost-gate_0.1.0-1_amd64.deb"
echo ""
echo "📦 Paquete RPM (para CentOS/RedHat/Fedora):"
echo "   Para construir el RPM, necesitas rpmbuild:"
echo "   1. sudo apt-get install rpm"
echo "   2. rpmbuild -bb ${RPM_DIR}/SPECS/bifrost-gate.spec --define '_topdir ${RPM_DIR}'"
echo ""
echo "📝 Directorio de trabajo: ${WORK_DIR}"
echo ""
