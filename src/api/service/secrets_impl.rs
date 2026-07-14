//! Gestión de secretos swanctl para Bifröst-Gate.
//!
//! Este módulo concentra toda la lógica de lectura, escritura, validación
//! y recarga de secrets en StrongSwan, separada de la gestión de conexiones.

use std::collections::HashSet;
use std::path::PathBuf;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::Value;

use crate::api::types::*;

// ─── Helpers de infraestructura (re-exportados desde connections) ─────────────

/// Ruta al archivo de un secret administrado por Bifröst.
#[cfg(target_os = "linux")]
fn secret_file_path(name: &str) -> PathBuf {
    PathBuf::from(format!(
        "{}/conf.d/bifrost-secret-{}.conf",
        crate::api::service::connections::get_swanctl_base_dir_pub(),
        name
    ))
}

// ─── Sanitización ─────────────────────────────────────────────────────────────

/// Valida que el nombre de un secret solo contenga caracteres seguros.
/// Acepta: letras ASCII, dígitos, `-`, `_`, `.`
#[cfg(target_os = "linux")]
pub fn sanitize_secret_name(name: &str) -> Option<String> {
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

// ─── Construcción del archivo de configuración ────────────────────────────────

/// Genera el texto TOML/HCL del archivo `.conf` para un secret.
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

// ─── Validación y renderizado del config de entrada ───────────────────────────

/// Valida el JSON de configuración según el tipo de secret y devuelve las
/// líneas listas para escribir en el archivo `.conf`.
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
            let trimmed_secret = secret.trim();
            if trimmed_secret.is_empty() {
                return Err("config.secret no puede estar vacío".to_string());
            }
            if trimmed_secret.len() < 8 {
                return Err(
                    "config.secret debe tener al menos 8 caracteres".to_string(),
                );
            }

            let ids = extract_ids(object)?;
            if ids.is_empty() {
                return Err(format!(
                    "config.id (string) o config.ids (lista de strings) es requerido para el tipo '{}'",
                    secret_type.as_str()
                ));
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
                lines.push(format!(
                    "module = {}",
                    render_scalar_value(module, "config.module")?
                ));
            }
            if let Some(pin) = object.get("pin") {
                lines.push(format!("pin = {}", render_scalar_value(pin, "config.pin")?));
            }

            Ok(lines)
        }
    }
}

// ─── Parser del archivo `.conf` de vuelta a la API ───────────────────────────

/// Parsea el contenido de un archivo `.conf` de secret y lo convierte
/// al formato de respuesta de la API.
///
/// - Los campos `secret` y `pin` se devuelven como `"***redacted***"`.
/// - Si hay un solo ID se devuelve como `"id": "..."`.
/// - Si hay múltiples IDs se devuelven como `"ids": [...]`.
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
            // Usamos splitn(2, '=') para preservar valores que contengan '='
            // (e.g., secrets base64 o PSKs con caracteres especiales).
            let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
            if parts.len() == 2 {
                assignments.push((
                    parts[0].trim().to_string(),
                    parts[1].trim().to_string(),
                ));
            }
        }
    }

    let section_name =
        section_name.ok_or_else(|| "No se encontró sección de secret".to_string())?;
    let (prefix, _) = section_name
        .split_once('-')
        .ok_or_else(|| "No se pudo identificar tipo de secret".to_string())?;

    let secret_type = SecretType::try_from(prefix)
        .map_err(|_| format!("Tipo de secret desconocido: '{}'", prefix))?;

    let mut config_map = serde_json::Map::new();
    let mut ids: Vec<Value> = Vec::new();

    for (key, value) in assignments {
        if key == "secret" || key == "pin" {
            config_map.insert(key, Value::String("***redacted***".to_string()));
            continue;
        }

        // Las claves "id", "id2", "id3", etc. se agregan al vec de IDs.
        if key == "id" || (key.starts_with("id") && key[2..].parse::<u32>().is_ok()) {
            ids.push(Value::String(value));
            continue;
        }

        config_map.insert(key, Value::String(value));
    }

    // Normalizar IDs: 1 → campo "id", varios → campo "ids"
    match ids.len() {
        0 => {}
        1 => {
            config_map.insert("id".to_string(), ids.remove(0));
        }
        _ => {
            config_map.insert("ids".to_string(), Value::Array(ids));
        }
    }

    Ok((secret_type, Value::Object(config_map)))
}

// ─── Helpers internos ─────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn validate_allowed_keys(
    object: &serde_json::Map<String, Value>,
    allowed: &HashSet<&str>,
) -> Result<(), String> {
    for key in object.keys() {
        if !allowed.contains(key.as_str()) {
            return Err(format!(
                "config.{} no es válido para este tipo de secret",
                key
            ));
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

/// Extrae la lista de IDs desde `config.id` (string único) o `config.ids` (array).
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

// ─── Listar secrets administrados ─────────────────────────────────────────────

/// Lista todos los secrets administrados por Bifröst (archivos `bifrost-secret-*.conf`).
#[cfg(target_os = "linux")]
pub async fn list_managed_secrets() -> Result<Vec<String>, std::io::Error> {
    let dir_path = format!(
        "{}/conf.d",
        crate::api::service::connections::get_swanctl_base_dir_pub()
    );
    let mut names = Vec::new();
    let mut dir = tokio::fs::read_dir(&dir_path).await?;
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

// ─── Handlers HTTP ────────────────────────────────────────────────────────────

pub async fn list_secrets_handler(
    state: crate::AppState,
    _language: Option<String>,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(SecretCrudResponse {
                code: crate::i18n::CODE_NOT_SUPPORTED,
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
            Ok(secrets) => {
                (StatusCode::OK, Json(SecretListResponse { secrets })).into_response()
            }
            Err(err) => {
                state
                    .logger
                    .error(&format!("Error listando secrets: {}", err));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(SecretCrudResponse {
                        code: crate::i18n::CODE_INTERNAL_ERROR,
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

pub async fn secret_read_handler(
    state: crate::AppState,
    secret_name: String,
    _language: Option<String>,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(SecretCrudResponse {
                code: crate::i18n::CODE_NOT_SUPPORTED,
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
                        code: crate::i18n::CODE_INTERNAL_ERROR,
                        name: secret_name,
                        action: "read".to_string(),
                        success: false,
                        message: "secret_name inválido: solo se permiten letras, dígitos, '-', '_' y '.'".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        let path = secret_file_path(&name);
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => match parse_secret_config_for_response(&content) {
                Ok((secret_type, config)) => (
                    StatusCode::OK,
                    Json(SecretResponse {
                        name,
                        secret_type,
                        config,
                    }),
                )
                    .into_response(),
                Err(err) => {
                    state
                        .logger
                        .error(&format!("Error parseando secret '{}': {}", name, err));
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(SecretCrudResponse {
                            code: crate::i18n::CODE_INTERNAL_ERROR,
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
                            code: crate::i18n::CODE_NOT_FOUND,
                            name,
                            action: "read".to_string(),
                            success: false,
                            message: "Secret no encontrado".to_string(),
                        }),
                    )
                        .into_response()
                } else {
                    state
                        .logger
                        .error(&format!("Error leyendo secret: {}", err));
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(SecretCrudResponse {
                            code: crate::i18n::CODE_INTERNAL_ERROR,
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
    _language: Option<String>,
) -> impl IntoResponse {
    let action = if update { "update" } else { "create" };

    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(SecretCrudResponse {
                code: crate::i18n::CODE_NOT_SUPPORTED,
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

        tracing::debug!("Intentando adquirir lock para secret '{}'", secret_name);

        let result: Result<(StatusCode, SecretCrudResponse), String> =
            crate::api::service::connections::with_swanctl_lock_pub(|| {
                Box::pin(async move {
                    tracing::debug!(
                        "Lock adquirido para secret '{}' tipo {:?}",
                        secret_name,
                        secret_type
                    );

                    let name = sanitize_secret_name(&secret_name).ok_or_else(|| {
                        tracing::debug!("Nombre de secret inválido: '{}'", secret_name);
                        "secret_name inválido: solo se permiten letras, dígitos, '-', '_' y '.'".to_string()
                    })?;

                    let config_lines =
                        validate_and_render_secret_config(secret_type, &config)?;

                    tracing::debug!(
                        "Config de secret '{}' validada: {} líneas",
                        name,
                        config_lines.len()
                    );

                    let path = secret_file_path(&name);
                    let exists = tokio::fs::metadata(&path).await.is_ok();

                    if !update && exists {
                        return Ok((
                            StatusCode::CONFLICT,
                            SecretCrudResponse {
                                code: crate::i18n::CODE_INTERNAL_ERROR,
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
                                code: crate::i18n::CODE_INTERNAL_ERROR,
                                name,
                                action: action_string,
                                success: false,
                                message: "El secret no existe".to_string(),
                            },
                        ));
                    }

                    let conf_text = build_secret_conf(&name, secret_type, &config_lines);
                    let backup = crate::api::service::connections::write_file_atomic_with_backup_pub(
                        &path,
                        &conf_text,
                        Some(0o600),
                    )
                    .await?;

                    match crate::api::service::connections::reload_swanctl_creds().await {
                        Ok(()) => Ok((
                            if update {
                                StatusCode::OK
                            } else {
                                StatusCode::CREATED
                            },
                            SecretCrudResponse {
                                code: crate::i18n::CODE_OK,
                                name,
                                action: action_string,
                                success: true,
                                message: "Secret guardado y credenciales recargadas".to_string(),
                            },
                        )),
                        Err(err) => {
                            let _ = crate::api::service::connections::restore_backup_or_delete_pub(
                                &path, backup,
                            )
                            .await;
                            Ok((
                                StatusCode::BAD_REQUEST,
                                SecretCrudResponse {
                                    code: crate::i18n::CODE_INTERNAL_ERROR,
                                    name,
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

        tracing::debug!("Lock liberado tras procesar secret '{}'", original_name);

        match result {
            Ok((code, body)) => (code, Json(body)).into_response(),
            Err(message) => {
                state
                    .logger
                    .error(&format!("Error procesando secret '{}': {}", original_name, message));
                (
                    StatusCode::BAD_REQUEST,
                    Json(SecretCrudResponse {
                        code: crate::i18n::CODE_INTERNAL_ERROR,
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
    _language: Option<String>,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(SecretCrudResponse {
                code: crate::i18n::CODE_NOT_SUPPORTED,
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
        let result: Result<(StatusCode, SecretCrudResponse), String> =
            crate::api::service::connections::with_swanctl_lock_pub(|| {
                Box::pin(async move {
                    let name = sanitize_secret_name(&secret_name).ok_or_else(|| {
                        "secret_name inválido: solo se permiten letras, dígitos, '-', '_' y '.'".to_string()
                    })?;
                    let path = secret_file_path(&name);

                    let backup = match tokio::fs::read_to_string(&path).await {
                        Ok(v) => Some(v),
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                            return Ok((
                                StatusCode::NOT_FOUND,
                                SecretCrudResponse {
                                    code: crate::i18n::CODE_NOT_FOUND,
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

                    match crate::api::service::connections::reload_swanctl_creds().await {
                        Ok(()) => Ok((
                            StatusCode::OK,
                            SecretCrudResponse {
                                code: crate::i18n::CODE_OK,
                                name,
                                action: "delete".to_string(),
                                success: true,
                                message: "Secret eliminado y credenciales recargadas".to_string(),
                            },
                        )),
                        Err(err) => {
                            let _ = crate::api::service::connections::restore_backup_or_delete_pub(
                                &path, backup,
                            )
                            .await;
                            Ok((
                                StatusCode::BAD_REQUEST,
                                SecretCrudResponse {
                                    code: crate::i18n::CODE_INTERNAL_ERROR,
                                    name,
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
                state.logger.error(&format!(
                    "Error eliminando secret '{}': {}",
                    original_name, message
                ));
                (
                    StatusCode::BAD_REQUEST,
                    Json(SecretCrudResponse {
                        code: crate::i18n::CODE_INTERNAL_ERROR,
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
