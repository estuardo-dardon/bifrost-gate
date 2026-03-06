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
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/etc/bifrost
mkdir -p %{buildroot}/lib/systemd/system
mkdir -p %{buildroot}/var/lib/bifrost
install -m 755 %{_sourcedir}/bifrost-gate %{buildroot}/usr/bin/bifrost-gate
install -m 755 %{_sourcedir}/bifrostctl %{buildroot}/usr/bin/bifrostctl
install -m 600 %{_sourcedir}/config.toml %{buildroot}/etc/bifrost/config.toml
install -m 644 %{_sourcedir}/bifrost.service %{buildroot}/lib/systemd/system/bifrost.service

%files
/usr/bin/bifrost-gate
/usr/bin/bifrostctl
/etc/bifrost/config.toml
/lib/systemd/system/bifrost.service
%dir /var/lib/bifrost

%post
mkdir -p /var/lib/bifrost
chmod 700 /var/lib/bifrost
systemctl daemon-reload 2>/dev/null || true

%changelog
* Mon Feb 02 2026 Estuardo Dardón <estuardo@example.com> - 0.1.0-1
- Initial release
