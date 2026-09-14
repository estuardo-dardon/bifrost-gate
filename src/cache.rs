/*
 * Bifröst-Gate: Agente de monitoreo para StrongSwan.
 * Copyright (C) 2026 Estuardo Dardón.
 *
 * Módulo de Caché Redis con soporte para Invalidation y Failover transparente.
 */

use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use tracing::{error, info, warn};
use std::sync::Arc;
use crate::config::RedisSettings;

#[derive(Clone)]
pub struct CacheService {
    client: Option<Arc<MultiplexedConnection>>,
    default_ttl: u64,
}

impl CacheService {
    pub async fn new(settings: Option<&RedisSettings>) -> Self {
        let default_ttl = settings.and_then(|s| s.ttl_seconds).unwrap_or(60);

        if let Some(s) = settings {
            if s.enabled {
                match redis::Client::open(s.url.as_str()) {
                    Ok(client) => match client.get_multiplexed_tokio_connection().await {
                        Ok(cm) => {
                            info!(target: "bifrost_gate::cache", "Conexión a Redis establecida exitosamente ({})", s.url);
                            return Self {
                                client: Some(Arc::new(cm)),
                                default_ttl,
                            };
                        }
                        Err(e) => {
                            error!(target: "bifrost_gate::cache", "Error creando conexión multiplexada de Redis: {}", e);
                        }
                    },
                    Err(e) => {
                        error!(target: "bifrost_gate::cache", "Error en URL de Redis ({}): {}", s.url, e);
                    }
                }
            }
        }

        info!(target: "bifrost_gate::cache", "Caché Redis desactivada o en modo bypass.");
        Self {
            client: None,
            default_ttl,
        }
    }

    /// Obtiene un valor serializado en JSON desde la caché de Redis.
    pub async fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let client = self.client.as_ref()?;
        let mut conn = (**client).clone();

        let val: Option<String> = match conn.get(key).await {
            Ok(v) => v,
            Err(e) => {
                warn!(target: "bifrost_gate::cache", "Error al consultar clave '{}' en Redis: {}", key, e);
                return None;
            }
        };

        if let Some(json_data) = val {
            match serde_json::from_str::<T>(&json_data) {
                Ok(val) => Some(val),
                Err(e) => {
                    warn!(target: "bifrost_gate::cache", "Error al deserializar clave '{}': {}", key, e);
                    None
                }
            }
        } else {
            None
        }
    }

    /// Guarda un valor serializado en JSON en la caché con un TTL (en segundos).
    pub async fn set<T: serde::Serialize>(&self, key: &str, value: &T, custom_ttl: Option<u64>) {
        let client = match self.client.as_ref() {
            Some(c) => c,
            None => return,
        };

        let ttl = custom_ttl.unwrap_or(self.default_ttl);
        let json_data = match serde_json::to_string(value) {
            Ok(s) => s,
            Err(e) => {
                error!(target: "bifrost_gate::cache", "Error al serializar valor para clave '{}': {}", key, e);
                return;
            }
        };

        let mut conn = (**client).clone();
        let res: Result<(), redis::RedisError> = conn.set_ex(key, json_data, ttl).await;
        if let Err(e) = res {
            warn!(target: "bifrost_gate::cache", "Error guardando clave '{}' en Redis: {}", key, e);
        }
    }

    /// Elimina claves especificadas o patrones de claves en Redis para invalidación.
    #[allow(dead_code)]
    pub async fn del(&self, key: &str) {
        let client = match self.client.as_ref() {
            Some(c) => c,
            None => return,
        };

        let mut conn = (**client).clone();
        let res: Result<(), redis::RedisError> = conn.del(key).await;
        if let Err(e) = res {
            warn!(target: "bifrost_gate::cache", "Error eliminando clave '{}' en Redis: {}", key, e);
        }
    }

    /// Elimina todas las claves que coincidan con un patrón (ej. "topology:*", "response_codes:*").
    pub async fn invalidate_pattern(&self, pattern: &str) {
        let client = match self.client.as_ref() {
            Some(c) => c,
            None => return,
        };

        let mut conn = (**client).clone();
        let keys: Result<Vec<String>, redis::RedisError> = conn.keys(pattern).await;
        if let Ok(keys_list) = keys {
            if !keys_list.is_empty() {
                let _: Result<(), redis::RedisError> = conn.del(keys_list).await;
            }
        }
    }
}
