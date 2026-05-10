/*
 * Bifröst-Gate: Agente de monitoreo para StrongSwan.
 * Copyright (C) 2026 Estuardo Dardón.
 * * Este programa es software libre: puedes redistribuirlo y/o modificarlo
 * bajo los términos de la Licencia Pública General Affero de GNU tal como
 * fue publicada por la Free Software Foundation, ya sea la versión 3 de
 * la Licencia, o (a tu elección) cualquier versión posterior.
 */
 
use serde::Deserialize;
use config::{Config, ConfigError, File};

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // Algunos campos solo se usan en bifrost-gate, no en bifrostctl
pub struct Settings {
    pub server: ServerSettings,
    pub tls: TlsSettings,
    pub auth: AuthSettings,
    pub database: DatabaseSettings,
    pub strongswan: Option<StrongSwanSettings>,
    pub logging: LoggingSettings,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // Algunos campos solo se usan en bifrost-gate, no en bifrostctl
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // Algunos campos solo se usan en bifrost-gate, no en bifrostctl
pub struct TlsSettings {
    pub enabled: bool,
    pub cert_path: String,
    pub key_path: String,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // Algunos campos solo se usan en bifrost-gate, no en bifrostctl
pub struct AuthSettings {
    /// Si true, la API exige API key en cada request protegida.
    pub enabled: bool,
    /// Header HTTP donde se envía la key, p.ej. "x-api-key".
    pub header_name: Option<String>,
    /// Usuario para sembrar la primera API key en la DB (si no existen keys).
    pub bootstrap_user: Option<String>,
    /// API key inicial para bootstrap (se usa solo si no existen keys activas).
    pub bootstrap_api_key: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseSettings {
    /// Ruta a la base de datos SQLite
    /// En Linux por defecto: /var/lib/bifrost/bifrost.db
    /// En otros sistemas: bifrost.db
    pub db_path: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct StrongSwanSettings {
    /// Ruta del lock file para swanctl.
    /// Se puede configurar en config.toml o con la variable de entorno BIFROST_SWANCTL_LOCK_PATH.
    pub lock_path: Option<String>,
    /// Directorio donde se crean los archivos temporales (.tmp) antes del rename atómico.
    /// Útil cuando el proceso no tiene permisos de escritura en /etc/swanctl/conf.d/.
    /// Si no se especifica, los .tmp se crean en el mismo directorio que el archivo destino.
    /// Ejemplo: tmp_dir = "/var/lib/bifrost/tmp"
    pub tmp_dir: Option<String>,
    /// Directorio base de la instalación de swanctl.
    /// Todos los subdirectorios (conf.d/, x509/, x509ca/, private/) se derivan de aquí.
    /// Por defecto: /etc/swanctl en producción.
    /// Se puede sobrescribir con la variable de entorno BIFROST_STRONGSWAN_CONF_DIR.
    pub conf_dir: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // Algunos campos solo se usan en bifrost-gate, no en bifrostctl
pub struct LoggingSettings {
    /// Nivel de log para el servicio (0=off, 1=info, 3=errors)
    pub service_level: u8,
    /// Nivel de log para los workers (0=off, 1=info, 3=errors)
    pub worker_level: u8,
    /// Si true, usa journalctl; si false, usa archivos
    pub use_journalctl: Option<bool>,
    /// Ruta del archivo de access log del servicio
    pub service_access_log: Option<String>,
    /// Ruta del archivo de error log del servicio
    pub service_error_log: Option<String>,
    /// Ruta del archivo de log de los workers
    pub worker_log: Option<String>,
    /// Capacidad del canal asíncrono (número de mensajes en cola antes de aplicar backpressure)
    pub channel_capacity: Option<usize>,
    /// Tamaño en MB para rotar archivos de log
    pub rotate_size_mb: Option<u64>,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let s = Config::builder()
            // Busca un archivo llamado config.toml, config.json, etc.
            .add_source(File::with_name("config"))
            // Permite sobrescribir valores con variables de entorno
            // Ejemplo: BIFROST_SERVER_PORT=443
            .add_source(config::Environment::with_prefix("BIFROST"))
            .build()?;

        s.try_deserialize()
    }
}