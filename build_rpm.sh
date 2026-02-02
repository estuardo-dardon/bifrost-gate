#!/bin/bash

TEMP_DIR=$(mktemp -d)
cd "$TEMP_DIR"
mkdir -p {SOURCES,SPECS,BUILD,RPMS,SRPMS}

# Copiar archivos fuente
cp /home/estuardodardon/workspace/app/bifrost/gate/target/release/bifrost-gate SOURCES/
cp /home/estuardodardon/workspace/app/bifrost/gate/config.toml SOURCES/
cp /home/estuardodardon/workspace/app/bifrost/gate/bifrost.service SOURCES/

# Crear spec file
cat > SPECS/bifrost-gate.spec << 'SPEC_EOF'
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
install -m 755 %{_sourcedir}/bifrost-gate %{buildroot}/usr/bin/
install -m 600 %{_sourcedir}/config.toml %{buildroot}/etc/bifrost/
install -m 644 %{_sourcedir}/bifrost.service %{buildroot}/lib/systemd/system/

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
SPEC_EOF

# Construir RPM
rpmbuild -bb SPECS/bifrost-gate.spec --define "_topdir $TEMP_DIR" 2>&1 | grep -E "(Executing|Processing|Wrote)"

# Copiar el RPM al proyecto
RPM_FILE=$(find "$TEMP_DIR/RPMS" -name "*.rpm" -type f)
if [ -f "$RPM_FILE" ]; then
    cp "$RPM_FILE" /home/estuardodardon/workspace/app/bifrost/gate/target/debian/
    echo "✓ RPM creado: $(basename $RPM_FILE)"
    ls -lh "$RPM_FILE"
else
    echo "✗ No se creó el RPM"
fi

# Limpiar
rm -rf "$TEMP_DIR"
