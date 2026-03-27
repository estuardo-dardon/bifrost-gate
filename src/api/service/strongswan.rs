use axum::http::StatusCode;
use axum::Json;

use crate::api::types::ServiceControlResponse;

pub async fn strongswan_control_handler(
    state: crate::AppState,
    action: &str,
) -> impl axum::response::IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(ServiceControlResponse {
                service_name: "strongswan".to_string(),
                action: action.to_string(),
                success: false,
                message: "Operacion soportada solo en Linux con systemd".to_string(),
            }),
        );
    }

    #[cfg(target_os = "linux")]
    {
        let unit = match detect_strongswan_unit().await {
            Ok(Some(unit)) => unit,
            Ok(None) => {
                let msg = "No se encontro unidad systemd de StrongSwan (strongswan, strongswan-starter o charon-systemd)".to_string();
                state.logger.error(&msg);
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ServiceControlResponse {
                        service_name: "strongswan".to_string(),
                        action: action.to_string(),
                        success: false,
                        message: msg,
                    }),
                );
            }
            Err(err) => {
                let msg = format!("Error detectando unidad de StrongSwan: {}", err);
                state.logger.error(&msg);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ServiceControlResponse {
                        service_name: "strongswan".to_string(),
                        action: action.to_string(),
                        success: false,
                        message: msg,
                    }),
                );
            }
        };

        match crate::exec::run_command(
            &crate::exec::ExecConfig::default(),
            "systemctl",
            &[action, &unit],
            Some(std::time::Duration::from_secs(15)),
        )
        .await
        {
            Ok(output) => {
                if output.status_code == Some(0) {
                    let msg = format!("Servicio '{}' {} exitosamente", unit, action);
                    state.logger.info(&msg);
                    (
                        StatusCode::OK,
                        Json(ServiceControlResponse {
                            service_name: unit,
                            action: action.to_string(),
                            success: true,
                            message: msg,
                        }),
                    )
                } else {
                    let msg = if output.stderr.is_empty() {
                        format!("systemctl devolvio error al ejecutar '{} {}'", action, unit)
                    } else {
                        output.stderr
                    };
                    state.logger.error(&msg);
                    (
                        StatusCode::BAD_REQUEST,
                        Json(ServiceControlResponse {
                            service_name: unit,
                            action: action.to_string(),
                            success: false,
                            message: msg,
                        }),
                    )
                }
            }
            Err(err) => {
                let msg = format!("No se pudo ejecutar systemctl: {:?}", err);
                state.logger.error(&msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ServiceControlResponse {
                        service_name: unit,
                        action: action.to_string(),
                        success: false,
                        message: msg,
                    }),
                )
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub async fn detect_strongswan_unit() -> Result<Option<String>, std::io::Error> {
    let candidates = ["strongswan", "strongswan-starter", "charon-systemd"];

    for unit in candidates {
        let output = crate::exec::run_command(
            &crate::exec::ExecConfig::default(),
            "systemctl",
            &["show", unit, "--property", "LoadState", "--value"],
            Some(std::time::Duration::from_secs(5)),
        )
        .await
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", err)))?;

        if output.status_code != Some(0) {
            continue;
        }

        let load_state = output.stdout.trim().to_string();
        if load_state != "not-found" && !load_state.is_empty() {
            return Ok(Some(unit.to_string()));
        }
    }

    Ok(None)
}
