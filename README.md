# Bifrost-Gate

![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)

**Bifrost-Gate** es un agente de monitoreo y operacion para StrongSwan/IPsec escrito en Rust.
Expone una API HTTP/HTTPS para topologia, metricas, control de peers/servicio y administracion de conexiones, secretos y certificados.

## Caracteristicas

- API REST con `axum`.
- Soporte TLS nativo (`rustls`) para exponer API en HTTPS.
- Worker Heimdall en segundo plano para deteccion periodica de estado.
- Persistencia local con SQLite (eventos, API keys, usuarios de docs).
- Metricas Prometheus en `/metrics`.
- Documentacion interactiva con Swagger UI (`/api/docs`) y ReDoc (`/api/tryme`).
- CLI administrativa `bifrostctl` para API keys, usuarios de documentacion y catalogo de codigos de respuesta (i18n).

## Requisitos

- Rust estable (toolchain actual del proyecto).
- Linux + StrongSwan para operaciones reales (`swanctl`, `systemctl`, certificados).
- OpenSSL para generacion/lectura de certificados.
- SQLite3.

## Configuracion

Usa `config.toml` (ver `config.toml.example`). Secciones principales:

- `[server]`: `host`, `port`.
- `[tls]`: `enabled`, `cert_path`, `key_path`.
- `[auth]`: `enabled`, `header_name`, `bootstrap_user`, `bootstrap_api_key`.
- `[logging]`: niveles y rutas de logs.

Valores importantes:

- Header API key por defecto: `x-api-key`.
- Si `auth.enabled = true`, los endpoints protegidos requieren API key valida en DB.

## Ejecucion

Desarrollo:

```bash
cargo run
```

Compilacion:

```bash
cargo build
```

Con systemd (instalacion del paquete):

```bash
sudo systemctl start bifrost-gate
sudo systemctl status bifrost-gate
```

## Endpoints principales

Base: `http://<host>:<port>` o `https://<host>:<port>` segun TLS.

- `GET /api/topology`: topologia actual.
- `GET /metrics`: metricas Prometheus.
- `POST /api/peers/{peer_name}/up`: inicia IKE + CHILD SA.
- `POST /api/peers/{peer_name}/down`: termina IKE/SA.
- `POST /api/strongswan/start`: inicia servicio StrongSwan.
- `POST /api/strongswan/stop`: detiene servicio StrongSwan.

CRUD de conexiones:

- `GET /api/connections`
- `POST /api/connections`
- `GET /api/connections/{connection_name}`
- `PUT /api/connections/{connection_name}`
- `DELETE /api/connections/{connection_name}`
- `POST /api/connections/{connection_name}/certificate` (adjunta certificado de usuario a conexion)

CRUD de secretos:

- `GET /api/secrets`
- `POST /api/secrets`
- `GET /api/secrets/{secret_name}`
- `PUT /api/secrets/{secret_name}`
- `DELETE /api/secrets/{secret_name}`

CRUD de certificados:

- CA: `/api/certificates/ca` y `/api/certificates/ca/{ca_name}`
- Usuario: `/api/certificates/user` y `/api/certificates/user/{cert_name}`

Documentacion:

- `GET /api/docs` (Swagger UI)
- `GET /api/tryme` (ReDoc)

## Autenticacion

Hay dos flujos de autenticacion:

- API de servicio: API key en header (por defecto `x-api-key`).
- UI de documentacion (`/api/docs`, `/api/tryme`): Basic Auth con usuarios de docs en SQLite.

## CLI administrativa (`bifrostctl`)

`bifrostctl` requiere privilegios de root.

API keys:

```bash
sudo bifrostctl apikey list
sudo bifrostctl apikey create <user_name>
sudo bifrostctl apikey enable <api_key>
sudo bifrostctl apikey disable <api_key>
sudo bifrostctl apikey delete <api_key>
```

Usuarios para docs (Basic Auth):

```bash
sudo bifrostctl docs-user list
sudo bifrostctl docs-user create <username> <password>
sudo bifrostctl docs-user passwd <username> <new_password>
sudo bifrostctl docs-user enable <username>
sudo bifrostctl docs-user disable <username>
sudo bifrostctl docs-user delete <username>
sudo bifrostctl docs-user grant-responses-manage <username>
sudo bifrostctl docs-user revoke-responses-manage <username>
```

Permisos de usuarios docs para `response_codes`:

- `view`: puede consultar (`GET`) catalogo, PDF y `whoami`.
- `manage`: puede consultar y administrar (`POST/PUT/DELETE`) codigos/traducciones.

Catalogo de codigos de respuesta (i18n):

```bash
sudo bifrostctl response list [lang]
sudo bifrostctl response add <code> <type> <message_en>
sudo bifrostctl response set-en <code> <message_en>
sudo bifrostctl response set-type <code> <type>
sudo bifrostctl response set-lang <code> <lang> <message>
sudo bifrostctl response del-lang <code> <lang>
sudo bifrostctl response delete <code>
```

Ejemplos:

```bash
sudo bifrostctl response add 30000 Auth "API Key Required"
sudo bifrostctl response set-lang 30000 es "API Key requerida"
sudo bifrostctl response list es
```

## Ejemplo rapido de llamada

```bash
curl -H "x-api-key: <API_KEY>" http://127.0.0.1:3000/api/topology
```

## Licencia

Este proyecto esta bajo GNU Affero General Public License v3 (AGPL-3.0-or-later).

Para consultas comerciales o soporte especializado:

- Bifrost-Gate maintainer (`maintainer@example.com`)

