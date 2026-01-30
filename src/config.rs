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
pub struct Settings {
    pub server: ServerSettings,
    pub tls: TlsSettings,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TlsSettings {
    pub enabled: bool,
    pub cert_path: String,
    pub key_path: String,
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