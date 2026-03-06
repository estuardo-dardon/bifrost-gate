#!/bin/bash

# Colores
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # Sin color

VERSION="0.1.0"
RELEASE="1"
ARCH="x86_64"
PACKAGE_NAME="bifrost-gate"
DESCRIPTION="Bifröst-Gate: Agente de monitoreo para StrongSwan"
MAINTAINER="Estuardo Dardón <estuardo@example.com>"

# Directorios
WORK_DIR="/tmp/bifrost-package"
mkdir -p "$WORK_DIR"

# Compilar release
echo -e "${BLUE}Compilando en modo release...${NC}"
cargo build --release --bins

# Crear estructura para .deb
echo -e "${BLUE}Creando estructura Debian...${NC}"
DEB_ROOT="$WORK_DIR/bifrost-deb-root"
rm -rf "$DEB_ROOT"
mkdir -p "$DEB_ROOT"/{usr/bin,etc/bifrost/certs,lib/systemd/system,var/lib/bifrost}

# Copiar archivos
cp target/release/bifrost-gate "$DEB_ROOT/usr/bin/"
cp target/release/bifrostctl "$DEB_ROOT/usr/bin/"
cp config.toml "$DEB_ROOT/etc/bifrost/"
cp bifrost.service "$DEB_ROOT/lib/systemd/system/"
chmod 600 "$DEB_ROOT/etc/bifrost/config.toml"

# Crear estructura de control Debian
mkdir -p "$DEB_ROOT/DEBIAN"
cat > "$DEB_ROOT/DEBIAN/control" << EOF
Package: $PACKAGE_NAME
Version: $VERSION-$RELEASE
Architecture: amd64
Maintainer: $MAINTAINER
Description: $DESCRIPTION
Homepage: https://github.com/estuardodardon/bifrost
Depends: libc6 (>= 2.38), sqlite3
Section: utils
Priority: optional
EOF

cat > "$DEB_ROOT/DEBIAN/postinst" << 'EOF'
#!/bin/bash
set -e
mkdir -p /var/lib/bifrost
chmod 700 /var/lib/bifrost
systemctl daemon-reload
echo "Bifröst-Gate instalado correctamente."
EOF

chmod 755 "$DEB_ROOT/DEBIAN/postinst"

# Crear .deb
echo -e "${BLUE}Generando paquete Debian...${NC}"
dpkg-deb --build "$DEB_ROOT" target/debian/bifrost-gate_${VERSION}-${RELEASE}_amd64.deb

# Crear estructura para .rpm
echo -e "${BLUE}Creando estructura RPM...${NC}"
RPM_ROOT="$WORK_DIR/bifrost-rpm-root"
rm -rf "$RPM_ROOT"
mkdir -p "$RPM_ROOT"/{usr/bin,etc/bifrost/certs,lib/systemd/system,var/lib/bifrost}

# Copiar archivos
cp target/release/bifrost-gate "$RPM_ROOT/usr/bin/"
cp target/release/bifrostctl "$RPM_ROOT/usr/bin/"
cp config.toml "$RPM_ROOT/etc/bifrost/"
cp bifrost.service "$RPM_ROOT/lib/systemd/system/"
chmod 600 "$RPM_ROOT/etc/bifrost/config.toml"

# Crear especificación RPM
mkdir -p "$WORK_DIR/SPECS"
cat > "$WORK_DIR/SPECS/bifrost-gate.spec" << 'EOF'
Name:           bifrost-gate
Version:        0.1.0
Release:        1%{?dist}
Summary:        Bifröst-Gate: Agente de monitoreo para StrongSwan

License:        AGPL-3.0-or-later
URL:            https://github.com/estuardodardon/bifrost

%description
Bifröst-Gate es un agente de monitoreo para la plataforma StrongSwan VPN.

%install
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/etc/bifrost/certs
mkdir -p %{buildroot}/lib/systemd/system
mkdir -p %{buildroot}/var/lib/bifrost
cp /tmp/bifrost-package/bifrost-rpm-root/usr/bin/bifrost-gate %{buildroot}/usr/bin/
cp /tmp/bifrost-package/bifrost-rpm-root/usr/bin/bifrostctl %{buildroot}/usr/bin/
cp /tmp/bifrost-package/bifrost-rpm-root/etc/bifrost/config.toml %{buildroot}/etc/bifrost/
cp /tmp/bifrost-package/bifrost-rpm-root/lib/systemd/system/bifrost.service %{buildroot}/lib/systemd/system/

%files
/usr/bin/bifrost-gate
/usr/bin/bifrostctl
/etc/bifrost/config.toml
/lib/systemd/system/bifrost.service
%dir /var/lib/bifrost

%post
mkdir -p /var/lib/bifrost
chmod 700 /var/lib/bifrost
systemctl daemon-reload

%changelog
* Mon Feb 02 2026 Estuardo Dardón <estuardo@example.com> - 0.1.0-1
- Initial release
EOF

echo -e "${GREEN}✓ Paquete Debian creado:${NC} target/debian/bifrost-gate_${VERSION}-${RELEASE}_amd64.deb"
echo -e "${GREEN}✓ Archivos preparados para RPM${NC}"
echo ""
echo "Instalación del paquete Debian:"
echo "  sudo dpkg -i target/debian/bifrost-gate_${VERSION}-${RELEASE}_amd64.deb"
echo ""
echo "Para generar el RPM, necesitas rpmbuild:"
echo "  sudo apt-get install rpm"
echo "  rpmbuild -bb $WORK_DIR/SPECS/bifrost-gate.spec"
