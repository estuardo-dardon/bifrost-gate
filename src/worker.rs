/*
 * Bifröst-Gate: Agente de monitoreo para StrongSwan.
 * Copyright (C) 2026 Estuardo Dardón.
 * * Este programa es software libre: puedes redistribuirlo y/o modificarlo
 * bajo los términos de la Licencia Pública General Affero de GNU tal como
 * fue publicada por la Free Software Foundation, ya sea la versión 3 de
 * la Licencia, o (a tu elección) cualquier versión posterior.
 */
 
use std::sync::{Arc, RwLock};
use tokio::time::{sleep, Duration};
use sqlx::SqlitePool;
use crate::models::BifrostTopology;
use crate::engine;

pub async fn start_heimdall_worker(
    topology_state: Arc<RwLock<BifrostTopology>>,
    pool: SqlitePool,
) {
    println!("👀 Heimdall (Worker) ha empezado su guardia en un módulo independiente...");
    
    // Obtenemos una copia inicial para comparar
    let mut last_topo = {
        let state = topology_state.read().unwrap();
        state.clone()
    };

    loop {
        // Esperar 10 segundos
        sleep(Duration::from_secs(10)).await;

        // Obtener nueva topología (Real en Linux, Mock en Windows)
        let new_topo = engine::generate_current_topology().await;

        // Detectar cambios mediante la lógica de engine.rs
        let alerts = engine::detect_status_changes(&last_topo, &new_topo);
        
        for alert_msg in alerts {
            println!("💾 Registrando en DB: {}", alert_msg);
            
            // Guardar en SQLite
            let res = sqlx::query("INSERT INTO alerts (message) VALUES (?)")
                .bind(alert_msg)
                .execute(&pool)
                .await;

            if let Err(e) = res {
                eprintln!("❌ Error al guardar alerta: {}", e);
            }
        }

        // Actualizar el estado global compartido con la API
        {
            let mut state_write = topology_state.write().unwrap();
            *state_write = new_topo.clone();
        }
        
        last_topo = new_topo;
    }
}