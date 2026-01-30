/*
 * Bifröst-Gate: Agente de monitoreo para StrongSwan.
 * Copyright (C) 2026 Estuardo Dardón.
 * * Este programa es software libre: puedes redistribuirlo y/o modificarlo
 * bajo los términos de la Licencia Pública General Affero de GNU tal como
 * fue publicada por la Free Software Foundation, ya sea la versión 3 de
 * la Licencia, o (a tu elección) cualquier versión posterior.
 */
 
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::str::FromStr;

pub async fn init_db() -> SqlitePool {
    // Crea el archivo de base de datos si no existe
    let options = SqliteConnectOptions::from_str("sqlite://bifrost.db")
        .unwrap()
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(options).await.unwrap();

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

    pool
}
