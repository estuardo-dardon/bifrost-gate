use std::collections::HashSet;
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::Value;

use crate::api::types::*;

#[cfg(target_os = "linux")]
const SWANCTL_LOCK_PATH: &str = "/var/lib/bifrost/swanctl.lock";

#[cfg(target_os = "linux")]
async fn with_swanctl_lock<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, String>> + Send>>
        + Send,
    T: Send,
{
    tokio::fs::create_dir_all("/var/lib/bifrost")
        .await
        .map_err(|e| format!("No se pudo crear /var/lib/bifrost: {}", e))?;

    let lock_file = tokio::task::spawn_blocking(|| -> Result<std::fs::File, String> {
        use std::fs::OpenOptions;
        use std::os::unix::io::AsRawFd;
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(SWANCTL_LOCK_PATH)
            .map_err(|e| format!("No se pudo abrir lock '{}': {}", SWANCTL_LOCK_PATH, e))?;

        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if rc != 0 {
            return Err(format!("No se pudo adquirir lock global swanctl (rc={})", rc));
        }
        Ok(file)
    })
    .await
    .map_err(|e| format!("Error interno adquiriendo lock: {}", e))??;

    // Mantener el file vivo mientras corre la operación.
    let _guard = lock_file;
    f().await
}

#[cfg(target_os = "linux")]
async fn write_file_atomic_with_backup(
    path: &PathBuf,
    contents: &str,
    perms: Option<u32>,
) -> Result<Option<String>, String> {
    let backup = match tokio::fs::read_to_string(path).await {
        Ok(existing) => Some(existing),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(format!("No se pudo leer backup de '{}': {}", path.display(), err)),
    };

    let tmp_path = PathBuf::from(format!("{}.tmp", path.to_string_lossy()));
    tokio::fs::write(&tmp_path, contents)
        .await
        .map_err(|e| format!("No se pudo escribir tmp '{}': {}", tmp_path.display(), e))?;

    if let Some(mode) = perms {
        tokio::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(mode))
            .await
            .map_err(|e| format!("No se pudo fijar permisos a '{}': {}", tmp_path.display(), e))?;
    }

    tokio::fs::rename(&tmp_path, path)
        .await
        .map_err(|e| format!("No se pudo aplicar rename atómico a '{}': {}", path.display(), e))?;

    Ok(backup)
}

#[cfg(target_os = "linux")]
async fn restore_backup_or_delete(path: &PathBuf, backup: Option<String>) -> Result<(), String> {
    match backup {
        Some(prev) => tokio::fs::write(path, prev)
            .await
            .map_err(|e| format!("No se pudo restaurar backup '{}': {}", path.display(), e)),
        None => match tokio::fs::remove_file(path).await {
            Ok(_) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(format!("No se pudo eliminar '{}': {}", path.display(), err)),
        },
    }
}

pub async fn list_connections_handler(
    state: crate::AppState,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(ConnectionCrudResponse {
                name: String::new(),
                action: "list".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        match list_managed_connections().await {
            Ok(connections) => (StatusCode::OK, Json(ConnectionListResponse { connections })).into_response(),
            Err(err) => {
                state.logger.error(&format!("Error listando conexiones: {}", err));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ConnectionCrudResponse {
                        name: String::new(),
                        action: "list".to_string(),
                        success: false,
                        message: format!("Error listando conexiones: {}", err),
                    }),
                )
                    .into_response()
            }
        }
    }
}

pub async fn list_secrets_handler(
    state: crate::AppState,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(SecretCrudResponse {
                name: String::new(),
                action: "list".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        match list_managed_secrets().await {
            Ok(secrets) => (StatusCode::OK, Json(SecretListResponse { secrets })).into_response(),
            Err(err) => {
                state.logger.error(&format!("Error listando secrets: {}", err));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(SecretCrudResponse {
                        name: String::new(),
                        action: "list".to_string(),
                        success: false,
                        message: format!("Error listando secrets: {}", err),
                    }),
                )
                    .into_response()
            }
        }
    }
}

pub async fn list_ca_certificates_handler(
    state: crate::AppState,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(CertificateCrudResponse {
                name: String::new(),
                kind: CertificateKind::Ca,
                action: "list".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        match list_managed_certificates(CertificateKind::Ca).await {
            Ok(certificates) => {
                (StatusCode::OK, Json(CertificateListResponse { certificates })).into_response()
            }
            Err(err) => {
                state.logger.error(&format!("Error listando CA: {}", err));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CertificateCrudResponse {
                        name: String::new(),
                        kind: CertificateKind::Ca,
                        action: "list".to_string(),
                        success: false,
                        message: format!("Error listando CA: {}", err),
                    }),
                )
                    .into_response()
            }
        }
    }
}

pub async fn list_user_certificates_handler(
    state: crate::AppState,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(CertificateCrudResponse {
                name: String::new(),
                kind: CertificateKind::User,
                action: "list".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        match list_managed_certificates(CertificateKind::User).await {
            Ok(certificates) => {
                (StatusCode::OK, Json(CertificateListResponse { certificates })).into_response()
            }
            Err(err) => {
                state
                    .logger
                    .error(&format!("Error listando certificados de usuario: {}", err));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CertificateCrudResponse {
                        name: String::new(),
                        kind: CertificateKind::User,
                        action: "list".to_string(),
                        success: false,
                        message: format!("Error listando certificados de usuario: {}", err),
                    }),
                )
                    .into_response()
            }
        }
    }
}

pub async fn connection_read_handler(
    state: crate::AppState,
    connection_name: String,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(ConnectionCrudResponse {
                name: connection_name,
                action: "read".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let name = match sanitize_connection_name(&connection_name) {
            Some(value) => value,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ConnectionCrudResponse {
                        name: connection_name,
                        action: "read".to_string(),
                        success: false,
                        message: "connection_name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        let path = connection_file_path(&name);
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                let config = extract_connection_body(&name, &content).unwrap_or(content);
                (StatusCode::OK, Json(ConnectionResponse { name, config })).into_response()
            }
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    (
                        StatusCode::NOT_FOUND,
                        Json(ConnectionCrudResponse {
                            name,
                            action: "read".to_string(),
                            success: false,
                            message: "Conexión no encontrada".to_string(),
                        }),
                    )
                        .into_response()
                } else {
                    state.logger.error(&format!("Error leyendo conexión: {}", err));
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ConnectionCrudResponse {
                            name,
                            action: "read".to_string(),
                            success: false,
                            message: format!("Error leyendo conexión: {}", err),
                        }),
                    )
                        .into_response()
                }
            }
        }
    }
}

pub async fn connection_upsert_handler(
    state: crate::AppState,
    connection_name: String,
    config: Value,
    update: bool,
) -> impl IntoResponse {
    let action = if update { "update" } else { "create" };

    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(ConnectionCrudResponse {
                name: connection_name,
                action: action.to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let original_name = connection_name.clone();
        let result = with_swanctl_lock(|| {
            Box::pin(async move {
                let name = sanitize_connection_name(&connection_name)
                    .ok_or_else(|| "connection_name inválido".to_string())?;

                let config_body = render_connection_body_from_json(&config)?;
                if config_body.trim().is_empty() {
                    return Err("config es requerido".to_string());
                }

                let path = connection_file_path(&name);
                let exists = tokio::fs::metadata(&path).await.is_ok();
                if !update && exists {
                    return Err("La conexión ya existe".to_string());
                }
                if update && !exists {
                    return Err("La conexión no existe".to_string());
                }

                let conf_text = build_connection_conf(&name, &config_body);
                let backup = write_file_atomic_with_backup(&path, &conf_text, Some(0o644)).await?;

                match reload_swanctl_conns().await {
                    Ok(()) => Ok((name, true, None::<String>)),
                    Err(err) => {
                        let _ = restore_backup_or_delete(&path, backup).await;
                        Ok((name, false, Some(err)))
                    }
                }
            })
        })
        .await;

        match result {
            Ok((name, true, _)) => {
                let code = if update { StatusCode::OK } else { StatusCode::CREATED };
                (code, Json(ConnectionCrudResponse { name, action: action.to_string(), success: true, message: "Conexión guardada y recargada".to_string() })).into_response()
            }
            Ok((name, false, Some(err))) => {
                state.logger.error(&format!("Error recargando conexiones (rollback aplicado): {}", err));
                (StatusCode::BAD_REQUEST, Json(ConnectionCrudResponse { name, action: action.to_string(), success: false, message: format!("Falló reload (se revirtió el cambio): {}", err) })).into_response()
            }
            Ok((name, false, None)) => {
                (StatusCode::BAD_REQUEST, Json(ConnectionCrudResponse { name, action: action.to_string(), success: false, message: "Falló reload (se revirtió el cambio)".to_string() })).into_response()
            }
            Err(message) => {
                (StatusCode::BAD_REQUEST, Json(ConnectionCrudResponse { name: original_name, action: action.to_string(), success: false, message })).into_response()
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn render_connection_body_from_json(config: &Value) -> Result<String, String> {
    let object = config
        .as_object()
        .ok_or_else(|| "config debe ser un objeto JSON".to_string())?;

    if object.is_empty() {
        return Ok(String::new());
    }

    let mut lines = Vec::new();
    render_connection_entries(object, 0, &mut lines)?;
    Ok(lines.join("\n"))
}

#[cfg(target_os = "linux")]
fn render_connection_entries(
    entries: &serde_json::Map<String, Value>,
    indent: usize,
    out: &mut Vec<String>,
) -> Result<(), String> {
    let prefix = " ".repeat(indent);

    for (key, value) in entries {
        if key.trim().is_empty() {
            return Err("config contiene una clave vacía".to_string());
        }

        match value {
            Value::Object(obj) => {
                out.push(format!("{}{} {{", prefix, key));
                render_connection_entries(obj, indent + 2, out)?;
                out.push(format!("{}}}", prefix));
            }
            Value::Array(items) => {
                if items.is_empty() {
                    return Err(format!("config.{} no puede ser una lista vacía", key));
                }

                let mut rendered = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        Value::String(s) => rendered.push(s.clone()),
                        Value::Number(n) => rendered.push(n.to_string()),
                        Value::Bool(b) => rendered.push(b.to_string()),
                        _ => {
                            return Err(format!(
                                "config.{} solo permite strings, numeros o booleanos en listas",
                                key
                            ))
                        }
                    }
                }

                out.push(format!("{}{} = {}", prefix, key, rendered.join(", ")));
            }
            Value::String(s) => out.push(format!("{}{} = {}", prefix, key, s)),
            Value::Number(n) => out.push(format!("{}{} = {}", prefix, key, n)),
            Value::Bool(b) => out.push(format!("{}{} = {}", prefix, key, b)),
            Value::Null => {
                return Err(format!("config.{} no puede ser null", key));
            }
        }
    }

    Ok(())
}

pub async fn connection_delete_handler(
    state: crate::AppState,
    connection_name: String,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(ConnectionCrudResponse {
                name: connection_name,
                action: "delete".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let original_name = connection_name.clone();
        let result = with_swanctl_lock(|| {
            Box::pin(async move {
                let name = sanitize_connection_name(&connection_name)
                    .ok_or_else(|| "connection_name inválido".to_string())?;
                let path = connection_file_path(&name);

                let backup = match tokio::fs::read_to_string(&path).await {
                    Ok(v) => Some(v),
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Err("Conexión no encontrada".to_string()),
                    Err(err) => return Err(format!("Error leyendo conexión: {}", err)),
                };

                tokio::fs::remove_file(&path)
                    .await
                    .map_err(|e| format!("Error eliminando conexión: {}", e))?;

                match reload_swanctl_conns().await {
                    Ok(()) => Ok((name, true, None::<String>)),
                    Err(err) => {
                        let _ = restore_backup_or_delete(&path, backup).await;
                        Ok((name, false, Some(err)))
                    }
                }
            })
        })
        .await;

        match result {
            Ok((name, true, _)) => (
                StatusCode::OK,
                Json(ConnectionCrudResponse { name, action: "delete".to_string(), success: true, message: "Conexión eliminada y recargada".to_string() }),
            )
                .into_response(),
            Ok((name, false, Some(err))) => {
                state.logger.error(&format!("Conexión eliminada pero falló reload (rollback aplicado): {}", err));
                (
                    StatusCode::BAD_REQUEST,
                    Json(ConnectionCrudResponse { name, action: "delete".to_string(), success: false, message: format!("Falló reload (se revirtió el cambio): {}", err) }),
                )
                    .into_response()
            }
            Err(message) if message == "Conexión no encontrada" => (
                StatusCode::NOT_FOUND,
                Json(ConnectionCrudResponse { name: original_name, action: "delete".to_string(), success: false, message }),
            )
                .into_response(),
            Err(message) => (
                StatusCode::BAD_REQUEST,
                Json(ConnectionCrudResponse { name: original_name, action: "delete".to_string(), success: false, message }),
            )
                .into_response(),
            _ => (
                StatusCode::BAD_REQUEST,
                Json(ConnectionCrudResponse { name: original_name, action: "delete".to_string(), success: false, message: "Falló operación".to_string() }),
            )
                .into_response(),
        }
    }
}

pub async fn secret_read_handler(
    state: crate::AppState,
    secret_name: String,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(SecretCrudResponse {
                name: secret_name,
                action: "read".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let name = match sanitize_secret_name(&secret_name) {
            Some(value) => value,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(SecretCrudResponse {
                        name: secret_name,
                        action: "read".to_string(),
                        success: false,
                        message: "secret_name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        let path = secret_file_path(&name);
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => match parse_secret_config_for_response(&content) {
                Ok((secret_type, config)) => {
                    (StatusCode::OK, Json(SecretResponse { name, secret_type, config })).into_response()
                }
                Err(err) => {
                    state.logger.error(&format!("Error parseando secret '{}': {}", name, err));
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(SecretCrudResponse {
                            name,
                            action: "read".to_string(),
                            success: false,
                            message: "Archivo de secret inválido".to_string(),
                        }),
                    )
                        .into_response()
                }
            },
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    (
                        StatusCode::NOT_FOUND,
                        Json(SecretCrudResponse {
                            name,
                            action: "read".to_string(),
                            success: false,
                            message: "Secret no encontrado".to_string(),
                        }),
                    )
                        .into_response()
                } else {
                    state.logger.error(&format!("Error leyendo secret: {}", err));
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(SecretCrudResponse {
                            name,
                            action: "read".to_string(),
                            success: false,
                            message: format!("Error leyendo secret: {}", err),
                        }),
                    )
                        .into_response()
                }
            }
        }
    }
}

pub async fn secret_upsert_handler(
    state: crate::AppState,
    secret_name: String,
    secret_type: SecretType,
    config: Value,
    update: bool,
) -> impl IntoResponse {
    let action = if update { "update" } else { "create" };

    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(SecretCrudResponse {
                name: secret_name,
                action: action.to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let original_name = secret_name.clone();
        let action_string = action.to_string();

        let result: Result<(StatusCode, SecretCrudResponse), String> = with_swanctl_lock(|| {
            Box::pin(async move {
                let name = sanitize_secret_name(&secret_name)
                    .ok_or_else(|| "secret_name inválido".to_string())?;
                let config_lines = validate_and_render_secret_config(secret_type, &config)?;

                let path = secret_file_path(&name);
                let exists = tokio::fs::metadata(&path).await.is_ok();
                if !update && exists {
                    return Ok((
                        StatusCode::CONFLICT,
                        SecretCrudResponse {
                            name,
                            action: action_string,
                            success: false,
                            message: "El secret ya existe".to_string(),
                        },
                    ));
                }
                if update && !exists {
                    return Ok((
                        StatusCode::NOT_FOUND,
                        SecretCrudResponse {
                            name,
                            action: action_string,
                            success: false,
                            message: "El secret no existe".to_string(),
                        },
                    ));
                }

                let conf_text = build_secret_conf(&name, secret_type, &config_lines);
                let backup = write_file_atomic_with_backup(&path, &conf_text, Some(0o600)).await?;

                match reload_swanctl_creds().await {
                    Ok(()) => Ok((
                        if update { StatusCode::OK } else { StatusCode::CREATED },
                        SecretCrudResponse {
                            name,
                            action: action_string,
                            success: true,
                            message: "Secret guardado y credenciales recargadas".to_string(),
                        },
                    )),
                    Err(err) => {
                        let _ = restore_backup_or_delete(&path, backup).await;
                        Ok((
                            StatusCode::BAD_REQUEST,
                            SecretCrudResponse {
                                name,
                                action: action_string,
                                success: false,
                                message: format!("Falló load-creds (se revirtió el cambio): {}", err),
                            },
                        ))
                    }
                }
            })
        })
        .await;

        match result {
            Ok((code, body)) => (code, Json(body)).into_response(),
            Err(message) => {
                state
                    .logger
                    .error(&format!("Error procesando secret '{}': {}", original_name, message));
                (
                    StatusCode::BAD_REQUEST,
                    Json(SecretCrudResponse {
                        name: original_name,
                        action: action.to_string(),
                        success: false,
                        message,
                    }),
                )
                    .into_response()
            }
        }
    }
}

pub async fn secret_delete_handler(
    state: crate::AppState,
    secret_name: String,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(SecretCrudResponse {
                name: secret_name,
                action: "delete".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let original_name = secret_name.clone();
        let result: Result<(StatusCode, SecretCrudResponse), String> = with_swanctl_lock(|| {
            Box::pin(async move {
                let name = sanitize_secret_name(&secret_name)
                    .ok_or_else(|| "secret_name inválido".to_string())?;
                let path = secret_file_path(&name);

                let backup = match tokio::fs::read_to_string(&path).await {
                    Ok(v) => Some(v),
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                        return Ok((
                            StatusCode::NOT_FOUND,
                            SecretCrudResponse {
                                name,
                                action: "delete".to_string(),
                                success: false,
                                message: "Secret no encontrado".to_string(),
                            },
                        ))
                    }
                    Err(err) => return Err(format!("Error leyendo secret: {}", err)),
                };

                tokio::fs::remove_file(&path)
                    .await
                    .map_err(|e| format!("Error eliminando secret: {}", e))?;

                match reload_swanctl_creds().await {
                    Ok(()) => Ok((
                        StatusCode::OK,
                        SecretCrudResponse {
                            name,
                            action: "delete".to_string(),
                            success: true,
                            message: "Secret eliminado y credenciales recargadas".to_string(),
                        },
                    )),
                    Err(err) => {
                        let _ = restore_backup_or_delete(&path, backup).await;
                        Ok((
                            StatusCode::BAD_REQUEST,
                            SecretCrudResponse {
                                name,
                                action: "delete".to_string(),
                                success: false,
                                message: format!("Falló load-creds (se revirtió el cambio): {}", err),
                            },
                        ))
                    }
                }
            })
        })
        .await;

        match result {
            Ok((code, body)) => (code, Json(body)).into_response(),
            Err(message) => {
                state.logger.error(&format!(
                    "Error eliminando secret '{}': {}",
                    original_name, message
                ));
                (
                    StatusCode::BAD_REQUEST,
                    Json(SecretCrudResponse {
                        name: original_name,
                        action: "delete".to_string(),
                        success: false,
                        message,
                    }),
                )
                    .into_response()
            }
        }
    }
}

/// Adjunta un certificado de usuario a una conexión administrada por Bifröst.
#[utoipa::path(
    post,
    path = "/api/connections/{connection_name}/certificate",
    request_body = ConnectionCertificateAttachRequest,
    params(("connection_name" = String, Path, description = "Nombre de la conexión")),
    responses(
        (status = 200, description = "Certificado adjuntado", body = ConnectionCrudResponse),
        (status = 400, description = "Solicitud inválida", body = ConnectionCrudResponse),
        (status = 404, description = "Conexión o certificado no encontrado", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operación no soportada", body = ConnectionCrudResponse)
    )
)]
pub async fn attach_certificate_to_connection_handler(
    _state: crate::AppState,
    connection_name: String,
    payload: ConnectionCertificateAttachRequest,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(ConnectionCrudResponse {
                name: connection_name,
                action: "attach-certificate".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let original_name = connection_name.clone();
        let result: Result<(StatusCode, ConnectionCrudResponse), String> = with_swanctl_lock(|| {
            Box::pin(async move {
                let conn_name = sanitize_connection_name(&connection_name)
                    .ok_or_else(|| "connection_name inválido".to_string())?;

                let cert_name = sanitize_certificate_name(&payload.certificate_name)
                    .ok_or_else(|| "certificate_name inválido".to_string())?;

                let conn_path = connection_file_path(&conn_name);
                let cert_path = user_certificate_cert_path(&cert_name);
                if tokio::fs::metadata(&cert_path).await.is_err() {
                    return Ok((
                        StatusCode::NOT_FOUND,
                        ConnectionCrudResponse {
                            name: conn_name,
                            action: "attach-certificate".to_string(),
                            success: false,
                            message: "El certificado de usuario no existe".to_string(),
                        },
                    ));
                }

                if let Some(ca_name) = &payload.remote_ca_name {
                    let sanitized = sanitize_certificate_name(ca_name)
                        .ok_or_else(|| "remote_ca_name inválido".to_string())?;

                    if tokio::fs::metadata(ca_certificate_cert_path(&sanitized))
                        .await
                        .is_err()
                    {
                        return Ok((
                            StatusCode::NOT_FOUND,
                            ConnectionCrudResponse {
                                name: conn_name,
                                action: "attach-certificate".to_string(),
                                success: false,
                                message: "La CA remota especificada no existe".to_string(),
                            },
                        ));
                    }
                }

                let content = match tokio::fs::read_to_string(&conn_path).await {
                    Ok(value) => value,
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                        return Ok((
                            StatusCode::NOT_FOUND,
                            ConnectionCrudResponse {
                                name: conn_name,
                                action: "attach-certificate".to_string(),
                                success: false,
                                message: "La conexión no existe".to_string(),
                            },
                        ))
                    }
                    Err(err) => return Err(format!("Error leyendo conexión: {}", err)),
                };

                let config_body = extract_connection_body(&conn_name, &content).unwrap_or(content);
                let updated = apply_certificate_to_connection_config(&config_body, &payload)?;
                let conf_text = build_connection_conf(&conn_name, &updated);

                let backup = write_file_atomic_with_backup(&conn_path, &conf_text, Some(0o644)).await?;

                match reload_swanctl_conns().await {
                    Ok(()) => Ok((
                        StatusCode::OK,
                        ConnectionCrudResponse {
                            name: conn_name,
                            action: "attach-certificate".to_string(),
                            success: true,
                            message: "Certificado aplicado a la conexión".to_string(),
                        },
                    )),
                    Err(err) => {
                        let _ = restore_backup_or_delete(&conn_path, backup).await;
                        Ok((
                            StatusCode::BAD_REQUEST,
                            ConnectionCrudResponse {
                                name: conn_name,
                                action: "attach-certificate".to_string(),
                                success: false,
                                message: format!(
                                    "Falló reload (se revirtió el cambio): {}",
                                    err
                                ),
                            },
                        ))
                    }
                }
            })
        })
        .await;

        match result {
            Ok((code, body)) => (code, Json(body)).into_response(),
            Err(message) => (
                StatusCode::BAD_REQUEST,
                Json(ConnectionCrudResponse {
                    name: original_name,
                    action: "attach-certificate".to_string(),
                    success: false,
                    message,
                }),
            )
                .into_response(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CaCertificateParams {
    pub common_name: String,
    pub organization: Option<String>,
    pub country: Option<String>,
    pub days: u32,
    pub key_size: u32,
}

#[derive(Debug, Clone)]
pub struct UserCertificateParams {
    pub ca_name: String,
    pub identity: String,
    pub san: Vec<String>,
    pub days: u32,
    pub key_size: u32,
}

pub async fn certificate_read_handler(
    state: crate::AppState,
    certificate_name: String,
    kind: CertificateKind,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(CertificateCrudResponse {
                name: certificate_name,
                kind,
                action: "read".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let name = match sanitize_certificate_name(&certificate_name) {
            Some(v) => v,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(CertificateCrudResponse {
                        name: certificate_name,
                        kind,
                        action: "read".to_string(),
                        success: false,
                        message: "certificate_name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        let cert_path = certificate_cert_path(kind, &name);
        let key_path = certificate_key_path(kind, &name);

        if tokio::fs::metadata(&cert_path).await.is_err() {
            return (
                StatusCode::NOT_FOUND,
                Json(CertificateCrudResponse {
                    name,
                    kind,
                    action: "read".to_string(),
                    success: false,
                    message: "Certificado no encontrado".to_string(),
                }),
            )
                .into_response();
        }

        match get_certificate_metadata(&cert_path).await {
            Ok((subject, issuer, not_after)) => (
                StatusCode::OK,
                Json(CertificateDetailsResponse {
                    name,
                    kind,
                    certificate_path: cert_path.to_string_lossy().to_string(),
                    private_key_path: Some(key_path.to_string_lossy().to_string()),
                    subject,
                    issuer,
                    not_after,
                }),
            )
                .into_response(),
            Err(err) => {
                state
                    .logger
                    .error(&format!("Error leyendo metadata de certificado: {}", err));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CertificateCrudResponse {
                        name,
                        kind,
                        action: "read".to_string(),
                        success: false,
                        message: format!("No se pudo leer metadata del certificado: {}", err),
                    }),
                )
                    .into_response()
            }
        }
    }
}

pub async fn certificate_ca_upsert_handler(
    _state: crate::AppState,
    certificate_name: String,
    params: CaCertificateParams,
    update: bool,
) -> impl IntoResponse {
    let action = if update { "update" } else { "create" };

    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(CertificateCrudResponse {
                name: certificate_name,
                kind: CertificateKind::Ca,
                action: action.to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let original_name = certificate_name.clone();
        let action_string = action.to_string();

        let result: Result<(StatusCode, CertificateCrudResponse), String> = with_swanctl_lock(|| {
            Box::pin(async move {
                let name = sanitize_certificate_name(&certificate_name)
                    .ok_or_else(|| "name inválido".to_string())?;
                if params.common_name.trim().is_empty() {
                    return Ok((
                        StatusCode::BAD_REQUEST,
                        CertificateCrudResponse {
                            name,
                            kind: CertificateKind::Ca,
                            action: action_string,
                            success: false,
                            message: "common_name es requerido".to_string(),
                        },
                    ));
                }

                let cert_path = ca_certificate_cert_path(&name);
                let key_path = ca_certificate_key_path(&name);
                let exists = tokio::fs::metadata(&cert_path).await.is_ok();
                if !update && exists {
                    return Ok((
                        StatusCode::CONFLICT,
                        CertificateCrudResponse {
                            name,
                            kind: CertificateKind::Ca,
                            action: action_string,
                            success: false,
                            message: "La CA ya existe".to_string(),
                        },
                    ));
                }
                if update && !exists {
                    return Ok((
                        StatusCode::NOT_FOUND,
                        CertificateCrudResponse {
                            name,
                            kind: CertificateKind::Ca,
                            action: action_string,
                            success: false,
                            message: "La CA no existe".to_string(),
                        },
                    ));
                }

                if let Err(err) = ensure_certificate_directories().await {
                    return Err(format!("No se pudieron preparar directorios: {}", err));
                }

                let cert_backup = tokio::fs::read(&cert_path).await.ok();
                let key_backup = tokio::fs::read(&key_path).await.ok();

                generate_ca_certificate_files(&cert_path, &key_path, &params).await?;

                match reload_swanctl_creds().await {
                    Ok(()) => Ok((
                        if update { StatusCode::OK } else { StatusCode::CREATED },
                        CertificateCrudResponse {
                            name,
                            kind: CertificateKind::Ca,
                            action: action_string,
                            success: true,
                            message: "CA generada y credenciales recargadas".to_string(),
                        },
                    )),
                    Err(err) => {
                        // rollback best-effort
                        if let Some(prev) = cert_backup {
                            let _ = tokio::fs::write(&cert_path, prev).await;
                        } else {
                            let _ = tokio::fs::remove_file(&cert_path).await;
                        }
                        if let Some(prev) = key_backup {
                            let _ = tokio::fs::write(&key_path, prev).await;
                        } else {
                            let _ = tokio::fs::remove_file(&key_path).await;
                        }

                        Ok((
                            StatusCode::BAD_REQUEST,
                            CertificateCrudResponse {
                                name,
                                kind: CertificateKind::Ca,
                                action: action_string,
                                success: false,
                                message: format!(
                                    "Falló load-creds (se revirtió el cambio): {}",
                                    err
                                ),
                            },
                        ))
                    }
                }
            })
        })
        .await;

        match result {
            Ok((code, body)) => (code, Json(body)).into_response(),
            Err(message) => (
                StatusCode::BAD_REQUEST,
                Json(CertificateCrudResponse {
                    name: original_name,
                    kind: CertificateKind::Ca,
                    action: action.to_string(),
                    success: false,
                    message,
                }),
            )
                .into_response(),
        }
    }
}

pub async fn certificate_user_upsert_handler(
    _state: crate::AppState,
    certificate_name: String,
    params: UserCertificateParams,
    update: bool,
) -> impl IntoResponse {
    let action = if update { "update" } else { "create" };

    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(CertificateCrudResponse {
                name: certificate_name,
                kind: CertificateKind::User,
                action: action.to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let original_name = certificate_name.clone();
        let action_string = action.to_string();

        let result: Result<(StatusCode, CertificateCrudResponse), String> = with_swanctl_lock(|| {
            Box::pin(async move {
                let name = sanitize_certificate_name(&certificate_name)
                    .ok_or_else(|| "name inválido".to_string())?;
                let ca_name = sanitize_certificate_name(&params.ca_name)
                    .ok_or_else(|| "ca_name inválido".to_string())?;

                if params.identity.trim().is_empty() {
                    return Ok((
                        StatusCode::BAD_REQUEST,
                        CertificateCrudResponse {
                            name,
                            kind: CertificateKind::User,
                            action: action_string,
                            success: false,
                            message: "identity es requerido".to_string(),
                        },
                    ));
                }

                let cert_path = user_certificate_cert_path(&name);
                let key_path = user_certificate_key_path(&name);
                let exists = tokio::fs::metadata(&cert_path).await.is_ok();
                if !update && exists {
                    return Ok((
                        StatusCode::CONFLICT,
                        CertificateCrudResponse {
                            name,
                            kind: CertificateKind::User,
                            action: action_string,
                            success: false,
                            message: "El certificado de usuario ya existe".to_string(),
                        },
                    ));
                }
                if update && !exists {
                    return Ok((
                        StatusCode::NOT_FOUND,
                        CertificateCrudResponse {
                            name,
                            kind: CertificateKind::User,
                            action: action_string,
                            success: false,
                            message: "El certificado de usuario no existe".to_string(),
                        },
                    ));
                }

                if let Err(err) = ensure_certificate_directories().await {
                    return Err(format!("No se pudieron preparar directorios: {}", err));
                }

                let ca_cert_path = ca_certificate_cert_path(&ca_name);
                let ca_key_path = ca_certificate_key_path(&ca_name);
                if tokio::fs::metadata(&ca_cert_path).await.is_err()
                    || tokio::fs::metadata(&ca_key_path).await.is_err()
                {
                    return Ok((
                        StatusCode::NOT_FOUND,
                        CertificateCrudResponse {
                            name,
                            kind: CertificateKind::User,
                            action: action_string,
                            success: false,
                            message: "La CA especificada no existe o está incompleta".to_string(),
                        },
                    ));
                }

                let cert_backup = tokio::fs::read(&cert_path).await.ok();
                let key_backup = tokio::fs::read(&key_path).await.ok();

                generate_user_certificate_files(
                    &cert_path,
                    &key_path,
                    &ca_cert_path,
                    &ca_key_path,
                    &params,
                )
                .await?;

                match reload_swanctl_creds().await {
                    Ok(()) => Ok((
                        if update { StatusCode::OK } else { StatusCode::CREATED },
                        CertificateCrudResponse {
                            name,
                            kind: CertificateKind::User,
                            action: action_string,
                            success: true,
                            message: "Certificado de usuario generado y credenciales recargadas"
                                .to_string(),
                        },
                    )),
                    Err(err) => {
                        if let Some(prev) = cert_backup {
                            let _ = tokio::fs::write(&cert_path, prev).await;
                        } else {
                            let _ = tokio::fs::remove_file(&cert_path).await;
                        }
                        if let Some(prev) = key_backup {
                            let _ = tokio::fs::write(&key_path, prev).await;
                        } else {
                            let _ = tokio::fs::remove_file(&key_path).await;
                        }

                        Ok((
                            StatusCode::BAD_REQUEST,
                            CertificateCrudResponse {
                                name,
                                kind: CertificateKind::User,
                                action: action_string,
                                success: false,
                                message: format!(
                                    "Falló load-creds (se revirtió el cambio): {}",
                                    err
                                ),
                            },
                        ))
                    }
                }
            })
        })
        .await;

        match result {
            Ok((code, body)) => (code, Json(body)).into_response(),
            Err(message) => (
                StatusCode::BAD_REQUEST,
                Json(CertificateCrudResponse {
                    name: original_name,
                    kind: CertificateKind::User,
                    action: action.to_string(),
                    success: false,
                    message,
                }),
            )
                .into_response(),
        }
    }
}

pub async fn certificate_delete_handler(
    state: crate::AppState,
    certificate_name: String,
    kind: CertificateKind,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(CertificateCrudResponse {
                name: certificate_name,
                kind,
                action: "delete".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let original_name = certificate_name.clone();
        let result: Result<(StatusCode, CertificateCrudResponse), String> = with_swanctl_lock(|| {
            Box::pin(async move {
                let name = sanitize_certificate_name(&certificate_name)
                    .ok_or_else(|| "name inválido".to_string())?;

                let cert_path = certificate_cert_path(kind, &name);
                let key_path = certificate_key_path(kind, &name);
                if tokio::fs::metadata(&cert_path).await.is_err() {
                    return Ok((
                        StatusCode::NOT_FOUND,
                        CertificateCrudResponse {
                            name,
                            kind,
                            action: "delete".to_string(),
                            success: false,
                            message: "Certificado no encontrado".to_string(),
                        },
                    ));
                }

                let cert_backup = tokio::fs::read(&cert_path).await.ok();
                let key_backup = tokio::fs::read(&key_path).await.ok();

                tokio::fs::remove_file(&cert_path)
                    .await
                    .map_err(|e| format!("Error eliminando certificado: {}", e))?;
                let _ = tokio::fs::remove_file(&key_path).await;

                match reload_swanctl_creds().await {
                    Ok(()) => Ok((
                        StatusCode::OK,
                        CertificateCrudResponse {
                            name,
                            kind,
                            action: "delete".to_string(),
                            success: true,
                            message: "Certificado eliminado y credenciales recargadas".to_string(),
                        },
                    )),
                    Err(err) => {
                        // rollback best-effort
                        if let Some(prev) = cert_backup {
                            let _ = tokio::fs::write(&cert_path, prev).await;
                        }
                        if let Some(prev) = key_backup {
                            let _ = tokio::fs::write(&key_path, prev).await;
                        }

                        Ok((
                            StatusCode::BAD_REQUEST,
                            CertificateCrudResponse {
                                name,
                                kind,
                                action: "delete".to_string(),
                                success: false,
                                message: format!(
                                    "Falló load-creds (se revirtió el cambio): {}",
                                    err
                                ),
                            },
                        ))
                    }
                }
            })
        })
        .await;

        match result {
            Ok((code, body)) => (code, Json(body)).into_response(),
            Err(message) => {
                state.logger.error(&message);
                (
                    StatusCode::BAD_REQUEST,
                    Json(CertificateCrudResponse {
                        name: original_name,
                        kind,
                        action: "delete".to_string(),
                        success: false,
                        message,
                    }),
                )
                    .into_response()
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn connection_file_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/etc/swanctl/conf.d/bifrost-{}.conf", name))
}

#[cfg(target_os = "linux")]
fn certificate_cert_path(kind: CertificateKind, name: &str) -> PathBuf {
    match kind {
        CertificateKind::Ca => ca_certificate_cert_path(name),
        CertificateKind::User => user_certificate_cert_path(name),
    }
}

#[cfg(target_os = "linux")]
fn certificate_key_path(kind: CertificateKind, name: &str) -> PathBuf {
    match kind {
        CertificateKind::Ca => ca_certificate_key_path(name),
        CertificateKind::User => user_certificate_key_path(name),
    }
}

#[cfg(target_os = "linux")]
pub fn ca_certificate_cert_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/etc/swanctl/x509ca/bifrost-ca-{}.crt", name))
}

#[cfg(target_os = "linux")]
pub fn ca_certificate_key_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/etc/swanctl/private/bifrost-ca-{}.key", name))
}

#[cfg(target_os = "linux")]
pub fn user_certificate_cert_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/etc/swanctl/x509/bifrost-user-{}.crt", name))
}

#[cfg(target_os = "linux")]
pub fn user_certificate_key_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/etc/swanctl/private/bifrost-user-{}.key", name))
}

#[cfg(target_os = "linux")]
pub fn sanitize_certificate_name(name: &str) -> Option<String> {
    sanitize_connection_name(name)
}

#[cfg(target_os = "linux")]
async fn ensure_certificate_directories() -> Result<(), std::io::Error> {
    tokio::fs::create_dir_all("/etc/swanctl/private").await?;
    tokio::fs::create_dir_all("/etc/swanctl/x509").await?;
    tokio::fs::create_dir_all("/etc/swanctl/x509ca").await?;
    Ok(())
}

#[cfg(target_os = "linux")]
async fn list_managed_certificates(kind: CertificateKind) -> Result<Vec<String>, std::io::Error> {
    let (dir, prefix) = match kind {
        CertificateKind::Ca => ("/etc/swanctl/x509ca", "bifrost-ca-"),
        CertificateKind::User => ("/etc/swanctl/x509", "bifrost-user-"),
    };

    let mut names = Vec::new();
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(v) => v,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(names),
        Err(err) => return Err(err),
    };

    while let Some(entry) = entries.next_entry().await? {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !file_name.starts_with(prefix) || !file_name.ends_with(".crt") {
            continue;
        }

        names.push(
            file_name
                .trim_start_matches(prefix)
                .trim_end_matches(".crt")
                .to_string(),
        );
    }
    names.sort();
    Ok(names)
}

#[cfg(target_os = "linux")]
fn build_subject(
    common_name: &str,
    organization: Option<&str>,
    country: Option<&str>,
) -> String {
    let mut subject = String::new();
    if let Some(country) = country {
        if !country.trim().is_empty() {
            subject.push_str(&format!("/C={}", country.trim()));
        }
    }
    if let Some(org) = organization {
        if !org.trim().is_empty() {
            subject.push_str(&format!("/O={}", org.trim()));
        }
    }
    subject.push_str(&format!("/CN={}", common_name.trim()));
    subject
}

#[cfg(target_os = "linux")]
async fn run_openssl(args: &[String]) -> Result<(), String> {
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = crate::exec::run_command(
        &crate::exec::ExecConfig::default(),
        "openssl",
        &args_ref,
        Some(std::time::Duration::from_secs(30)),
    )
    .await
    .map_err(|err| format!("No se pudo ejecutar openssl: {:?}", err))?;

    if output.status_code == Some(0) {
        Ok(())
    } else if output.stderr.is_empty() {
        Err("openssl falló sin detalle".to_string())
    } else {
        Err(output.stderr)
    }
}

#[cfg(target_os = "linux")]
async fn generate_ca_certificate_files(
    cert_path: &PathBuf,
    key_path: &PathBuf,
    params: &CaCertificateParams,
) -> Result<(), String> {
    let gen_key_args = vec![
        "genpkey".to_string(),
        "-algorithm".to_string(),
        "RSA".to_string(),
        "-pkeyopt".to_string(),
        format!("rsa_keygen_bits:{}", params.key_size),
        "-out".to_string(),
        key_path.to_string_lossy().to_string(),
    ];
    run_openssl(&gen_key_args).await?;

    let subject = build_subject(
        &params.common_name,
        params.organization.as_deref(),
        params.country.as_deref(),
    );
    let req_args = vec![
        "req".to_string(),
        "-x509".to_string(),
        "-new".to_string(),
        "-key".to_string(),
        key_path.to_string_lossy().to_string(),
        "-sha256".to_string(),
        "-days".to_string(),
        params.days.to_string(),
        "-subj".to_string(),
        subject,
        "-out".to_string(),
        cert_path.to_string_lossy().to_string(),
    ];
    run_openssl(&req_args).await?;

    tokio::fs::set_permissions(key_path, std::fs::Permissions::from_mode(0o600))
        .await
        .map_err(|err| format!("No se pudo fijar permisos de llave privada: {}", err))?;
    tokio::fs::set_permissions(cert_path, std::fs::Permissions::from_mode(0o644))
        .await
        .map_err(|err| format!("No se pudo fijar permisos de certificado: {}", err))?;

    Ok(())
}

#[cfg(target_os = "linux")]
async fn generate_user_certificate_files(
    cert_path: &PathBuf,
    key_path: &PathBuf,
    ca_cert_path: &PathBuf,
    ca_key_path: &PathBuf,
    params: &UserCertificateParams,
) -> Result<(), String> {
    let gen_key_args = vec![
        "genpkey".to_string(),
        "-algorithm".to_string(),
        "RSA".to_string(),
        "-pkeyopt".to_string(),
        format!("rsa_keygen_bits:{}", params.key_size),
        "-out".to_string(),
        key_path.to_string_lossy().to_string(),
    ];
    run_openssl(&gen_key_args).await?;

    let subject = build_subject(&params.identity, None, None);
    let csr_path = std::env::temp_dir().join(format!(
        "bifrost-user-{}-{}.csr",
        params.identity.replace('/', "_"),
        std::process::id()
    ));
    let csr_args = vec![
        "req".to_string(),
        "-new".to_string(),
        "-key".to_string(),
        key_path.to_string_lossy().to_string(),
        "-subj".to_string(),
        subject,
        "-out".to_string(),
        csr_path.to_string_lossy().to_string(),
    ];
    run_openssl(&csr_args).await?;

    let mut sign_args = vec![
        "x509".to_string(),
        "-req".to_string(),
        "-in".to_string(),
        csr_path.to_string_lossy().to_string(),
        "-CA".to_string(),
        ca_cert_path.to_string_lossy().to_string(),
        "-CAkey".to_string(),
        ca_key_path.to_string_lossy().to_string(),
        "-CAcreateserial".to_string(),
        "-out".to_string(),
        cert_path.to_string_lossy().to_string(),
        "-days".to_string(),
        params.days.to_string(),
        "-sha256".to_string(),
    ];

    let mut ext_path: Option<PathBuf> = None;
    if !params.san.is_empty() {
        let mut san_values = Vec::with_capacity(params.san.len());
        for value in &params.san {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.contains(':') {
                san_values.push(trimmed.to_string());
            } else {
                san_values.push(format!("DNS:{}", trimmed));
            }
        }

        if !san_values.is_empty() {
            let ext = std::env::temp_dir().join(format!(
                "bifrost-user-{}-{}.ext",
                params.identity.replace('/', "_"),
                std::process::id()
            ));
            let ext_body = format!(
                "[v3_req]\nbasicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=clientAuth\nsubjectAltName={}\n",
                san_values.join(",")
            );
            tokio::fs::write(&ext, ext_body)
                .await
                .map_err(|err| format!("No se pudo escribir extensión SAN: {}", err))?;
            sign_args.push("-extfile".to_string());
            sign_args.push(ext.to_string_lossy().to_string());
            sign_args.push("-extensions".to_string());
            sign_args.push("v3_req".to_string());
            ext_path = Some(ext);
        }
    }

    let sign_result = run_openssl(&sign_args).await;
    let _ = tokio::fs::remove_file(&csr_path).await;
    if let Some(path) = ext_path {
        let _ = tokio::fs::remove_file(&path).await;
    }
    sign_result?;

    tokio::fs::set_permissions(key_path, std::fs::Permissions::from_mode(0o600))
        .await
        .map_err(|err| format!("No se pudo fijar permisos de llave privada: {}", err))?;
    tokio::fs::set_permissions(cert_path, std::fs::Permissions::from_mode(0o644))
        .await
        .map_err(|err| format!("No se pudo fijar permisos de certificado: {}", err))?;

    Ok(())
}

#[cfg(target_os = "linux")]
async fn get_certificate_metadata(
    cert_path: &PathBuf,
) -> Result<(Option<String>, Option<String>, Option<String>), String> {
    let args = vec![
        "x509".to_string(),
        "-in".to_string(),
        cert_path.to_string_lossy().to_string(),
        "-noout".to_string(),
        "-subject".to_string(),
        "-issuer".to_string(),
        "-enddate".to_string(),
    ];

    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = crate::exec::run_command(
        &crate::exec::ExecConfig::default(),
        "openssl",
        &args_ref,
        Some(std::time::Duration::from_secs(10)),
    )
    .await
    .map_err(|err| format!("No se pudo ejecutar openssl para metadata: {:?}", err))?;

    if output.status_code != Some(0) {
        return Err(if output.stderr.is_empty() {
            "openssl no pudo leer metadata".to_string()
        } else {
            output.stderr
        });
    }

    let mut subject = None;
    let mut issuer = None;
    let mut not_after = None;
    for line in output.stdout.lines() {
        let trimmed = line.trim();
        if let Some(v) = trimmed.strip_prefix("subject=") {
            subject = Some(v.trim().to_string());
        } else if let Some(v) = trimmed.strip_prefix("issuer=") {
            issuer = Some(v.trim().to_string());
        } else if let Some(v) = trimmed.strip_prefix("notAfter=") {
            not_after = Some(v.trim().to_string());
        }
    }
    Ok((subject, issuer, not_after))
}

#[cfg(target_os = "linux")]
fn apply_certificate_to_connection_config(
    config_body: &str,
    payload: &ConnectionCertificateAttachRequest,
) -> Result<String, String> {
    let cert_name = sanitize_certificate_name(&payload.certificate_name)
        .ok_or_else(|| "certificate_name inválido".to_string())?;

    let mut output = config_body.to_string();
    let mut local_values = vec![
        (
            "auth".to_string(),
            "pubkey".to_string(),
        ),
        (
            "certs".to_string(),
            format!("x509/bifrost-user-{}.crt", cert_name),
        ),
    ];
    if let Some(local_id) = &payload.local_id {
        if local_id.trim().is_empty() {
            return Err("local_id no puede ser vacío".to_string());
        }
        local_values.push(("id".to_string(), local_id.trim().to_string()));
    }
    output = upsert_connection_section_values(&output, "local", &local_values);

    let mut remote_values: Vec<(String, String)> = Vec::new();
    if payload.set_remote_auth_pubkey.unwrap_or(true) {
        remote_values.push(("auth".to_string(), "pubkey".to_string()));
    }
    if let Some(ca_name) = &payload.remote_ca_name {
        let sanitized = sanitize_certificate_name(ca_name)
            .ok_or_else(|| "remote_ca_name inválido".to_string())?;
        remote_values.push((
            "cacerts".to_string(),
            format!("x509ca/bifrost-ca-{}.crt", sanitized),
        ));
    }
    if !remote_values.is_empty() {
        output = upsert_connection_section_values(&output, "remote", &remote_values);
    }

    Ok(output)
}

#[cfg(target_os = "linux")]
fn upsert_connection_section_values(
    config_body: &str,
    section: &str,
    values: &[(String, String)],
) -> String {
    let mut lines: Vec<String> = config_body.lines().map(|line| line.to_string()).collect();
    let section_label = format!("{} {{", section);

    if let Some((start, end, indent)) = find_top_level_section_range(&lines, &section_label) {
        let keys: HashSet<&str> = values.iter().map(|(key, _)| key.as_str()).collect();
        let mut kept = Vec::new();
        for line in &lines[start + 1..end] {
            let trimmed = line.trim();
            if let Some((key, _)) = trimmed.split_once('=') {
                if keys.contains(key.trim()) {
                    continue;
                }
            }
            kept.push(line.clone());
        }

        let inner_indent = format!("{}  ", indent);
        let mut rebuilt = Vec::new();
        rebuilt.push(format!("{}{}", indent, section_label));
        for (key, value) in values {
            rebuilt.push(format!("{}{} = {}", inner_indent, key, value));
        }
        rebuilt.extend(kept);
        rebuilt.push(format!("{}}}", indent));

        lines.splice(start..=end, rebuilt);
    } else {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(section_label);
        for (key, value) in values {
            lines.push(format!("  {} = {}", key, value));
        }
        lines.push("}".to_string());
    }

    lines.join("\n")
}

#[cfg(target_os = "linux")]
fn find_top_level_section_range(
    lines: &[String],
    section_label: &str,
) -> Option<(usize, usize, String)> {
    let mut depth: i32 = 0;
    let mut idx = 0;
    while idx < lines.len() {
        let line = &lines[idx];
        let trimmed = line.trim();
        if depth == 0 && trimmed == section_label {
            let indent = line
                .chars()
                .take_while(|c| c.is_ascii_whitespace())
                .collect::<String>();

            depth += brace_delta(line);
            let mut end = idx;
            let mut j = idx + 1;
            while j < lines.len() {
                depth += brace_delta(&lines[j]);
                if depth == 0 {
                    end = j;
                    break;
                }
                j += 1;
            }
            return Some((idx, end, indent));
        }

        depth += brace_delta(line);
        idx += 1;
    }
    None
}

#[cfg(target_os = "linux")]
fn brace_delta(line: &str) -> i32 {
    let open = line.matches('{').count() as i32;
    let close = line.matches('}').count() as i32;
    open - close
}

#[cfg(target_os = "linux")]
fn secret_file_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/etc/swanctl/conf.d/bifrost-secret-{}.conf", name))
}

#[cfg(target_os = "linux")]
pub fn sanitize_connection_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
pub fn sanitize_secret_name(name: &str) -> Option<String> {
    sanitize_connection_name(name)
}

#[cfg(target_os = "linux")]
fn build_secret_conf(name: &str, secret_type: SecretType, config_lines: &[String]) -> String {
    let section_name = format!("{}-{}", secret_type.as_str(), name);

    let mut out = String::from("secrets {\n");
    out.push_str(&format!("  {} {{\n", section_name));
    for line in config_lines {
        out.push_str("    ");
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("  }\n}\n");
    out
}

#[cfg(target_os = "linux")]
fn validate_and_render_secret_config(
    secret_type: SecretType,
    config: &Value,
) -> Result<Vec<String>, String> {
    let object = config
        .as_object()
        .ok_or_else(|| "config debe ser un objeto JSON".to_string())?;

    match secret_type {
        SecretType::Eap | SecretType::Xauth | SecretType::Ntlm | SecretType::Ike | SecretType::Ppk => {
            let allowed = HashSet::from(["secret", "id", "ids"]);
            validate_allowed_keys(object, &allowed)?;

            let secret = extract_required_string(object, "secret")?;
            if secret.trim().is_empty() {
                return Err("config.secret no puede estar vacío".to_string());
            }

            let ids = extract_ids(object)?;
            if ids.is_empty() {
                return Err("config.ids (o config.id) es requerido".to_string());
            }

            let mut lines = vec![format!("secret = {}", secret)];
            for (idx, id) in ids.iter().enumerate() {
                let key = if idx == 0 {
                    "id".to_string()
                } else {
                    format!("id{}", idx + 1)
                };
                lines.push(format!("{} = {}", key, id));
            }
            Ok(lines)
        }
        SecretType::Private
        | SecretType::Rsa
        | SecretType::Ecdsa
        | SecretType::Pkcs8
        | SecretType::Pkcs12 => {
            let allowed = HashSet::from(["file", "secret"]);
            validate_allowed_keys(object, &allowed)?;

            let file = extract_required_string(object, "file")?;
            let secret = extract_required_string(object, "secret")?;
            if file.trim().is_empty() {
                return Err("config.file no puede estar vacío".to_string());
            }
            if secret.trim().is_empty() {
                return Err("config.secret no puede estar vacío".to_string());
            }

            Ok(vec![
                format!("file = {}", file),
                format!("secret = {}", secret),
            ])
        }
        SecretType::Token => {
            let allowed = HashSet::from(["handle", "slot", "module", "pin"]);
            validate_allowed_keys(object, &allowed)?;

            let handle = extract_required_string(object, "handle")?;
            if handle.trim().is_empty() {
                return Err("config.handle no puede estar vacío".to_string());
            }

            let mut lines = vec![format!("handle = {}", handle)];
            if let Some(slot) = object.get("slot") {
                lines.push(format!("slot = {}", render_scalar_value(slot, "config.slot")?));
            }
            if let Some(module) = object.get("module") {
                lines.push(format!("module = {}", render_scalar_value(module, "config.module")?));
            }
            if let Some(pin) = object.get("pin") {
                lines.push(format!("pin = {}", render_scalar_value(pin, "config.pin")?));
            }

            Ok(lines)
        }
    }
}

#[cfg(target_os = "linux")]
fn validate_allowed_keys(
    object: &serde_json::Map<String, Value>,
    allowed: &HashSet<&str>,
) -> Result<(), String> {
    for key in object.keys() {
        if !allowed.contains(key.as_str()) {
            return Err(format!("config.{} no es válido para este tipo de secret", key));
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn extract_required_string(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<String, String> {
    let value = object
        .get(key)
        .ok_or_else(|| format!("config.{} es requerido", key))?;

    match value {
        Value::String(s) => Ok(s.clone()),
        _ => Err(format!("config.{} debe ser string", key)),
    }
}

#[cfg(target_os = "linux")]
fn extract_ids(object: &serde_json::Map<String, Value>) -> Result<Vec<String>, String> {
    if let Some(id) = object.get("id") {
        return match id {
            Value::String(s) if !s.trim().is_empty() => Ok(vec![s.clone()]),
            Value::String(_) => Err("config.id no puede estar vacío".to_string()),
            _ => Err("config.id debe ser string".to_string()),
        };
    }

    if let Some(ids) = object.get("ids") {
        return match ids {
            Value::Array(values) => {
                if values.is_empty() {
                    return Err("config.ids no puede ser vacío".to_string());
                }

                let mut out = Vec::with_capacity(values.len());
                for (idx, value) in values.iter().enumerate() {
                    match value {
                        Value::String(s) if !s.trim().is_empty() => out.push(s.clone()),
                        Value::String(_) => {
                            return Err(format!("config.ids[{}] no puede estar vacío", idx))
                        }
                        _ => return Err(format!("config.ids[{}] debe ser string", idx)),
                    }
                }
                Ok(out)
            }
            _ => Err("config.ids debe ser lista de strings".to_string()),
        };
    }

    Ok(Vec::new())
}

#[cfg(target_os = "linux")]
fn render_scalar_value(value: &Value, field_name: &str) -> Result<String, String> {
    match value {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        _ => Err(format!("{} debe ser string, número o booleano", field_name)),
    }
}

#[cfg(target_os = "linux")]
fn parse_secret_config_for_response(content: &str) -> Result<(SecretType, Value), String> {
    let mut section_name: Option<String> = None;
    let mut assignments: Vec<(String, String)> = Vec::new();
    let mut in_secret_block = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed == "secrets {" {
            continue;
        }

        if trimmed.ends_with('{') && section_name.is_none() {
            let name = trimmed.trim_end_matches('{').trim();
            section_name = Some(name.to_string());
            in_secret_block = true;
            continue;
        }

        if in_secret_block && trimmed == "}" {
            break;
        }

        if in_secret_block {
            if let Some((key, value)) = trimmed.split_once('=') {
                assignments.push((key.trim().to_string(), value.trim().to_string()));
            }
        }
    }

    let section_name = section_name.ok_or_else(|| "No se encontró sección de secret".to_string())?;
    let (prefix, _) = section_name
        .split_once('-')
        .ok_or_else(|| "No se pudo identificar tipo de secret".to_string())?;

    let secret_type = SecretType::from_str(prefix)
        .ok_or_else(|| "Tipo de secret desconocido".to_string())?;

    let mut config_map = serde_json::Map::new();
    let mut ids = Vec::new();
    for (key, value) in assignments {
        if key == "secret" || key == "pin" {
            config_map.insert(key, Value::String("***redacted***".to_string()));
            continue;
        }

        if key == "id" || key.starts_with("id") {
            ids.push(Value::String(value));
            continue;
        }

        config_map.insert(key, Value::String(value));
    }

    if !ids.is_empty() {
        config_map.insert("ids".to_string(), Value::Array(ids));
    }

    Ok((secret_type, Value::Object(config_map)))
}

#[cfg(target_os = "linux")]
fn build_connection_conf(name: &str, config_body: &str) -> String {
    let mut out = String::from("connections {\n");
    out.push_str(&format!("  {} {{\n", name));
    for line in config_body.lines() {
        out.push_str("    ");
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("  }\n}\n");
    out
}

#[cfg(target_os = "linux")]
fn extract_connection_body(name: &str, content: &str) -> Option<String> {
    let needle = format!("{} {{", name);
    let start = content.find(&needle)?;
    let after = &content[start + needle.len()..];
    let end = after.rfind('}')?;
    let body = after[..end].trim_matches('\n');
    let cleaned = body
        .lines()
        .map(|line| line.strip_prefix("    ").unwrap_or(line).to_string())
        .collect::<Vec<_>>()
        .join("\n");
    Some(cleaned.trim().to_string())
}

#[cfg(target_os = "linux")]
pub async fn reload_swanctl_conns() -> Result<(), String> {
    let output = crate::exec::run_command(
        &crate::exec::ExecConfig::default(),
        "swanctl",
        &["--load-conns"],
        Some(std::time::Duration::from_secs(20)),
    )
    .await
    .map_err(|err| format!("No se pudo ejecutar swanctl --load-conns: {:?}", err))?;

    if output.status_code == Some(0) {
        Ok(())
    } else if output.stderr.is_empty() {
        Err("swanctl --load-conns falló sin detalle".to_string())
    } else {
        Err(output.stderr)
    }
}

#[cfg(target_os = "linux")]
pub async fn reload_swanctl_creds() -> Result<(), String> {
    let output = crate::exec::run_command(
        &crate::exec::ExecConfig::default(),
        "swanctl",
        &["--load-creds"],
        Some(std::time::Duration::from_secs(20)),
    )
    .await
    .map_err(|err| format!("No se pudo ejecutar swanctl --load-creds: {:?}", err))?;

    if output.status_code == Some(0) {
        Ok(())
    } else if output.stderr.is_empty() {
        Err("swanctl --load-creds falló sin detalle".to_string())
    } else {
        Err(output.stderr)
    }
}

#[cfg(target_os = "linux")]
pub async fn list_managed_connections() -> Result<Vec<String>, std::io::Error> {
    let mut names = Vec::new();
    let mut dir = tokio::fs::read_dir("/etc/swanctl/conf.d").await?;
    while let Some(entry) = dir.next_entry().await? {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !file_name.starts_with("bifrost-") || !file_name.ends_with(".conf") {
            continue;
        }

        let name = file_name
            .trim_start_matches("bifrost-")
            .trim_end_matches(".conf")
            .to_string();
        names.push(name);
    }
    names.sort();
    Ok(names)
}

#[cfg(target_os = "linux")]
pub async fn list_managed_secrets() -> Result<Vec<String>, std::io::Error> {
    let mut names = Vec::new();
    let mut dir = tokio::fs::read_dir("/etc/swanctl/conf.d").await?;
    while let Some(entry) = dir.next_entry().await? {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !file_name.starts_with("bifrost-secret-") || !file_name.ends_with(".conf") {
            continue;
        }

        let name = file_name
            .trim_start_matches("bifrost-secret-")
            .trim_end_matches(".conf")
            .to_string();
        names.push(name);
    }
    names.sort();
    Ok(names)
}


