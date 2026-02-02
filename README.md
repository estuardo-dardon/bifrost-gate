# 🌈 Bifröst-Gate

**Bifröst-Gate** es un agente de monitoreo ligero y seguro escrito en Rust, diseñado para supervisar túneles VPN IPsec (StrongSwan) mediante el protocolo VICI. Es el corazón del ecosistema Bifröst, proporcionando datos en tiempo real y alertas de red a la interfaz **Bifröst-View**.

![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)

## ✨ Características

- 🛡️ **Seguridad Nativa**: Comunicación cifrada mediante TLS (HTTPS) utilizando `rustls`.
- ⚡ **Heimdall Worker**: Hilo de fondo independiente que vigila el estado de la red de forma periódica.
- 💾 **Persistencia**: Registro histórico de caídas y eventos en una base de datos SQLite local.
- ⚙️ **Configuración Flexible**: Gestión mediante archivo `config.toml` y soporte para variables de entorno.
- 🐧 **Optimizado para Linux**: Integración directa con StrongSwan a través de `rsvici`.
- 🪟 **Modo Desarrollo**: Capacidad de simulación (Mocking) para pruebas locales en Windows o macOS.

## 🚀 Instalación Rápida

### Requisitos previos
- Rust (MSRV 1.75+)
- StrongSwan (en entornos de producción Linux)
- SQLite3


🛠️ Uso
Para iniciar el agente:

```bash
sudo systemctl start bifrost
```

El servidor estará disponible por defecto en el puerto definido en su configuración. Puede verificar el estado de la topología mediante la API: GET /api/topology

⚖️ **Licencia y Comercialización**
Este proyecto está bajo la licencia GNU Affero General Public License v3 (AGPL-3.0).

**¿Qué significa esto?**

**Libertad:** Puedes usar, modificar y distribuir este software gratuitamente.

**Reciprocidad:** Si modificas el software y lo ofreces a través de una red (SaaS), debes liberar el código fuente de tus cambios bajo la misma licencia.

**Uso Comercial:** Si su organización desea integrar Bifröst en un producto propietario o no desea cumplir con los términos de la AGPL, ofrecemos licencias comerciales personalizadas y soporte técnico especializado.

Para consultas sobre licencias comerciales, implementaciones a medida o soporte prioritario, por favor contacta a: Estuardo Dardón (estuardodardon@office.com).

