/*
 * Bifröst-Gate: Agente de monitoreo para StrongSwan.
 * Copyright (C) 2026 Estuardo Dardón.
 * * Este programa es software libre: puedes redistribuirlo y/o modificarlo
 * bajo los términos de la Licencia Pública General Affero de GNU tal como
 * fue publicada por la Free Software Foundation, ya sea la versión 3 de
 * la Licencia, o (a tu elección) cualquier versión posterior.
 */
 
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ApiKeyRecord {
    pub id: i64,
    pub user_name: String,
    pub api_key: String,
    pub is_active: bool,
    pub created_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DocsUserRecord {
    pub id: i64,
    pub username: String,
    pub is_active: bool,
    pub created_at: String,
}

pub async fn init_db() -> SqlitePool {
    // Ruta de DB configurable por entorno para operaciones/diagnóstico.
    // En Linux por defecto usamos /var/lib/bifrost para ser compatible con systemd + ProtectSystem=strict.
    let db_path = resolve_db_path();
    ensure_db_parent_dir(&db_path);

    let db_url = format!("sqlite://{}", db_path);

    // Crea el archivo de base de datos si no existe
    let options = SqliteConnectOptions::from_str(&db_url)
        .expect("No se pudo construir URL de SQLite")
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(options)
        .await
        .unwrap_or_else(|e| panic!("No se pudo abrir la base SQLite en '{}': {}", db_path, e));

    // Crear tabla de alertas
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS alerts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message TEXT NOT NULL,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
        )"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Crear tabla para API keys por usuario
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS api_keys (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_name TEXT NOT NULL,
            api_key TEXT NOT NULL UNIQUE,
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Crear tabla para usuarios de documentación (Basic Auth)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS docs_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )"
    )
    .execute(&pool)
    .await
    .unwrap();

    pool
}

fn resolve_db_path() -> String {
    if let Ok(path) = env::var("BIFROST_DB_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    #[cfg(target_os = "linux")]
    {
        return "/var/lib/bifrost/bifrost.db".to_string();
    }

    #[cfg(not(target_os = "linux"))]
    {
        "bifrost.db".to_string()
    }
}

fn ensure_db_parent_dir(db_path: &str) {
    let path = Path::new(db_path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
}

#[allow(dead_code)]
pub async fn is_valid_api_key(pool: &SqlitePool, api_key: &str) -> Result<bool, sqlx::Error> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM api_keys WHERE api_key = ? AND is_active = 1 LIMIT 1"
    )
    .bind(api_key)
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some())
}

#[allow(dead_code)]
pub async fn create_api_key_for_user(pool: &SqlitePool, user_name: &str) -> Result<String, sqlx::Error> {
    let key = format!("bfg_{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO api_keys (user_name, api_key, is_active) VALUES (?, ?, 1)"
    )
    .bind(user_name)
    .bind(&key)
    .execute(pool)
    .await?;

    Ok(key)
}

#[allow(dead_code)]
pub async fn seed_api_key_if_missing(
    pool: &SqlitePool,
    user_name: &str,
    api_key: &str,
) -> Result<bool, sqlx::Error> {
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM api_keys WHERE is_active = 1"
    )
    .fetch_one(pool)
    .await?;

    if count.0 > 0 {
        return Ok(false);
    }

    sqlx::query(
        "INSERT INTO api_keys (user_name, api_key, is_active) VALUES (?, ?, 1)"
    )
    .bind(user_name)
    .bind(api_key)
    .execute(pool)
    .await?;

    Ok(true)
}

#[allow(dead_code)]
pub async fn count_active_api_keys(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM api_keys WHERE is_active = 1"
    )
    .fetch_one(pool)
    .await?;

    Ok(count.0)
}

#[allow(dead_code)]
pub async fn list_api_keys(pool: &SqlitePool) -> Result<Vec<ApiKeyRecord>, sqlx::Error> {
    let rows: Vec<(i64, String, String, i64, String)> = sqlx::query_as(
        "SELECT id, user_name, api_key, is_active, created_at FROM api_keys ORDER BY id DESC"
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, user_name, api_key, is_active, created_at)| ApiKeyRecord {
            id,
            user_name,
            api_key,
            is_active: is_active == 1,
            created_at,
        })
        .collect())
}

#[allow(dead_code)]
pub async fn set_api_key_active(pool: &SqlitePool, api_key: &str, active: bool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE api_keys SET is_active = ? WHERE api_key = ?"
    )
    .bind(if active { 1 } else { 0 })
    .bind(api_key)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn delete_api_key(pool: &SqlitePool, api_key: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM api_keys WHERE api_key = ?")
        .bind(api_key)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[allow(dead_code)]
pub async fn verify_docs_user_credentials(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> Result<bool, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT password_hash FROM docs_users WHERE username = ? AND is_active = 1 LIMIT 1"
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    let expected = match row {
        Some((hash,)) => hash,
        None => return Ok(false),
    };

    Ok(expected == hash_password(password))
}

#[allow(dead_code)]
pub async fn list_docs_users(pool: &SqlitePool) -> Result<Vec<DocsUserRecord>, sqlx::Error> {
    let rows: Vec<(i64, String, i64, String)> = sqlx::query_as(
        "SELECT id, username, is_active, created_at FROM docs_users ORDER BY id DESC"
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, username, is_active, created_at)| DocsUserRecord {
            id,
            username,
            is_active: is_active == 1,
            created_at,
        })
        .collect())
}

#[allow(dead_code)]
pub async fn create_docs_user(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO docs_users (username, password_hash, is_active) VALUES (?, ?, 1)"
    )
    .bind(username)
    .bind(hash_password(password))
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn update_docs_user_password(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE docs_users SET password_hash = ? WHERE username = ?"
    )
    .bind(hash_password(password))
    .bind(username)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn set_docs_user_active(
    pool: &SqlitePool,
    username: &str,
    active: bool,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE docs_users SET is_active = ? WHERE username = ?"
    )
    .bind(if active { 1 } else { 0 })
    .bind(username)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn delete_docs_user(pool: &SqlitePool, username: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM docs_users WHERE username = ?")
        .bind(username)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}
