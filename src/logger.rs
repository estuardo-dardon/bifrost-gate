/*
 * Bifröst-Gate: Logger Module
 * Copyright (C) 2026 Estuardo Dardón.
 * 
 * Manejo centralizado de logs con soporte para:
 * - journalctl (systemd)
 * - Archivos en /var/log
 * - Niveles configurables (0=off, 1=info, 3=errors)
 */

use chrono::Local;
use once_cell::sync::OnceCell;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Off = 0,
    Info = 1,
    Error = 3,
}

#[derive(Clone)]
pub struct Logger {
    level: LogLevel,
    context: String,
    log_file: Arc<Mutex<Option<std::fs::File>>>,
    error_log_file: Arc<Mutex<Option<std::fs::File>>>,
    use_journalctl: bool,
}

/// Global async sender used by Logger when initialized
static GLOBAL_ASYNC_SENDER: OnceCell<Sender<String>> = OnceCell::new();

impl Logger {
    /// Constructor alternativo: use `with_custom_paths` to customize file paths.

    /// Crea un logger con rutas personalizadas de archivos
    /// access_log_path: ruta para logs de acceso (solo para servicio)
    /// error_log_path: ruta para logs de error (solo para servicio)
    /// worker_log_path: ruta para logs del worker
    pub fn with_custom_paths(
        level: u8,
        context: &str,
        access_log_path: Option<&str>,
        error_log_path: Option<&str>,
        worker_log_path: Option<&str>,
    ) -> Self {
        let log_level = match level {
            0 => LogLevel::Off,
            1 => LogLevel::Info,
            3 => LogLevel::Error,
            _ => LogLevel::Off,
        };

        let use_journalctl = is_journalctl_available();
        
        let log_file = if !use_journalctl {
            let path = match context {
                "service" => access_log_path.unwrap_or("/var/log/bifrost-service-access.log"),
                "worker" => worker_log_path.unwrap_or("/var/log/bifrost-worker.log"),
                _ => "/var/log/bifrost.log",
            };
            Arc::new(Mutex::new(Self::open_log_file(path)))
        } else {
            Arc::new(Mutex::new(None))
        };

        let error_log_file = if !use_journalctl && context == "service" {
            let path = error_log_path.unwrap_or("/var/log/bifrost-service-error.log");
            Arc::new(Mutex::new(Self::open_log_file(path)))
        } else {
            Arc::new(Mutex::new(None))
        };

        Logger {
            level: log_level,
            context: context.to_string(),
            log_file,
            error_log_file,
            use_journalctl,
        }
    }

    /// Abre o crea el archivo de log en la ruta especificada
    fn open_log_file(log_path: &str) -> Option<std::fs::File> {
        // Crear directorio si no existe
        if let Some(parent) = std::path::Path::new(log_path).parent() {
            let _ = fs::create_dir_all(parent);
        }
        
        // Intentar abrir el archivo en modo append
        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
        {
            Ok(file) => {
                eprintln!("Log file opened: {}", log_path);
                Some(file)
            }
            Err(e) => {
                eprintln!(
                    "Failed to open log file {}: {}. Logs will only be printed to stderr.",
                    log_path, e
                );
                None
            }
        }
    }

    /// Log de información
    pub fn info(&self, message: &str) {
        if self.level >= LogLevel::Info {
            self.write_log("INFO", message);
        }
    }

    /// Log de error
    pub fn error(&self, message: &str) {
        if self.level >= LogLevel::Error {
            self.write_log("ERROR", message);
        }
    }

    /// Log de excepción (Error crítico)
    // `exception` helper removed — prefer using `error()` with formatted error details.

    /// Escribe el log en el destino apropiado
    fn write_log(&self, level: &str, message: &str) {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();
        let trace_id = Uuid::new_v4().to_string();
        let thread_id = format!("{:?}", std::thread::current().id());
        let formatted = format!(
            "[{}] [{}] [{}] [{}] [{}] {}",
            timestamp, level, self.context, trace_id, thread_id, message
        );
        // Si existe un logger asíncrono global, enviar el mensaje por el canal
            if let Some(tx) = GLOBAL_ASYNC_SENDER.get() {
                if tx.try_send(formatted.clone()).is_ok() {
                    // También imprimir a stderr para debugging
                    eprintln!("{}", formatted);
                    return;
                } else {
                    // channel full: increment Prometheus counter and fallback to sync write
                    crate::metrics::incr_dropped_logs();
                }
            }

        // Si no hay logger asíncrono, comportarse de forma síncrona
        if self.use_journalctl {
            self.log_to_journalctl(&formatted, level);
        } else {
            // Para el servicio, usa el archivo de error si es ERROR o EXCEPTION
            let is_error = level == "ERROR" || level == "EXCEPTION";
            if is_error && self.context == "service" {
                self.log_to_error_file(&formatted);
            } else {
                self.log_to_file(&formatted);
            }
        }

        // Siempre imprimir a stderr para debugging
        eprintln!("{}", formatted);
    }

    /// Escribe a journalctl
    fn log_to_journalctl(&self, message: &str, level: &str) {
        let priority = match level {
            "INFO" => "6", // info
            "ERROR" => "3", // err
            "EXCEPTION" => "2", // crit
            _ => "6",
        };

        let _ = std::process::Command::new("logger")
            .arg("-p")
            .arg(format!("local0.{}", priority))
            .arg("-t")
            .arg("bifrost-gate")
            .arg(message)
            .spawn();
    }

    /// Escribe a archivo
    fn log_to_file(&self, message: &str) {
        if let Ok(mut file_guard) = self.log_file.lock() {
            if let Some(ref mut file) = *file_guard {
                let _ = writeln!(file, "{}", message);
                let _ = file.flush();
            }
        }
    }

    /// Escribe a archivo de error (solo para servicio)
    fn log_to_error_file(&self, message: &str) {
        if let Ok(mut file_guard) = self.error_log_file.lock() {
            if let Some(ref mut file) = *file_guard {
                let _ = writeln!(file, "{}", message);
                let _ = file.flush();
            }
        }
    }

    /// Log de API request
    pub fn log_api_request(
        &self,
        method: &str,
        path: &str,
        status: u16,
        duration_ms: u128,
    ) {
        if self.level >= LogLevel::Info {
            let message = format!(
                "API_REQUEST: {} {} | Status: {} | Duration: {}ms",
                method, path, status, duration_ms
            );
            self.info(&message);
        }
    }

    /// Log de API error
    pub fn log_api_error(&self, method: &str, path: &str, status: u16, error: &str) {
        if self.level >= LogLevel::Error {
            let message = format!(
                "API_ERROR: {} {} | Status: {} | Error: {}",
                method, path, status, error
            );
            self.error(&message);
        }
    }

    /// Log de worker activity
    pub fn log_worker_activity(&self, worker_id: &str, activity: &str) {
        if self.level >= LogLevel::Info {
            let message = format!("WORKER[{}]: {}", worker_id, activity);
            self.info(&message);
        }
    }

    /// Log de worker error
    pub fn log_worker_error(&self, worker_id: &str, error: &str) {
        if self.level >= LogLevel::Error {
            let message = format!("WORKER_ERROR[{}]: {}", worker_id, error);
            self.error(&message);
        }
    }
}

/// Helper called by the instrumentation macro at function entry
pub fn auto_instrument_enter(fn_name: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();
    let msg = format!("[{}] [INFO] [auto] [{}] Enter {}", timestamp, uuid::Uuid::new_v4(), fn_name);
    if let Some(tx) = GLOBAL_ASYNC_SENDER.get() {
        let _ = tx.try_send(msg);
    } else {
        eprintln!("{}", msg);
    }
}

/// Called by instrumentation macro on normal exit
pub fn auto_instrument_exit(fn_name: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();
    let msg = format!("[{}] [INFO] [auto] [{}] Exit {}", timestamp, Uuid::new_v4(), fn_name);
    if let Some(tx) = GLOBAL_ASYNC_SENDER.get() {
        let _ = tx.try_send(msg);
    } else {
        eprintln!("{}", msg);
    }
}

/// Called by instrumentation macro when a function returns Err
// `auto_instrument_error` removed; macro-generated error logging uses `auto_instrument_exit`
// and callers should log errors explicitly via `Logger::error`/`Logger::exception`.

/// A small writer that forwards tracing `Write` calls into the async logger channel.
pub struct TracingWriter;

impl TracingWriter {
    pub fn new() -> Self {
        TracingWriter
    }
}

impl io::Write for TracingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Try to interpret as UTF-8 and split lines; best-effort, non-buffering.
        let s = String::from_utf8_lossy(buf);
        for line in s.split('\n') {
            if line.trim().is_empty() {
                continue;
            }
            let msg = format!("[service] [TRACE] {}", line.trim());
            if let Some(tx) = GLOBAL_ASYNC_SENDER.get() {
                let _ = tx.try_send(msg.clone());
            } else {
                eprintln!("{}", msg);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Initialize `tracing` to forward events into our async logger. Call this after
/// `init_async_logger` in `main` to capture library tracing events.
pub fn init_tracing_forwarder() {
    let make_writer = || TracingWriter::new();
    let subscriber = tracing_subscriber::fmt().with_writer(make_writer).finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}

// Removed atomic dropped-logs counter; Prometheus counter is used instead.

/// Inicializa el logger asíncrono global. Debe llamarse una vez desde `main`.
pub fn init_async_logger(
    service_access_log: Option<&str>,
    service_error_log: Option<&str>,
    worker_log: Option<&str>,
    use_journalctl: bool,
    channel_capacity: usize,
    rotate_size_bytes: u64,
) {
    // Si ya está inicializado, no hacer nada
    if GLOBAL_ASYNC_SENDER.get().is_some() {
        return;
    }

    // bounded channel for backpressure
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(channel_capacity);
    let _ = GLOBAL_ASYNC_SENDER.set(tx.clone());

    // spawn background task to process messages
    let access = service_access_log.map(|s| s.to_string());
    let error = service_error_log.map(|s| s.to_string());
    let worker = worker_log.map(|s| s.to_string());

    let rotate_size = rotate_size_bytes;

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let msg_clone = msg.clone();
            // If journalctl configured, send to journal
            if use_journalctl {
                tokio::task::spawn_blocking(move || {
                    let _ = std::process::Command::new("logger")
                        .arg("-t")
                        .arg("bifrost-gate")
                        .arg(msg_clone)
                        .status();
                });
                continue;
            }

            // determine destination by inspecting message for context and level
            let dest_path = if msg.contains("[service]") {
                if msg.contains("[ERROR]") || msg.contains("[EXCEPTION]") {
                    error.as_deref()
                } else {
                    access.as_deref()
                }
            } else if msg.contains("[worker]") {
                worker.as_deref()
            } else {
                access.as_deref().or(worker.as_deref())
            };

            if let Some(path) = dest_path {
                let p = path.to_string();
                let m = msg.clone();
                tokio::task::spawn_blocking(move || {
                    // rotate if needed
                    if let Ok(metadata) = std::fs::metadata(&p) {
                        if metadata.len() > rotate_size {
                            let ts = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
                            let new_name = format!("{}.{}", p, ts);
                            let _ = std::fs::rename(&p, &new_name);
                        }
                    }

                    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&p) {
                        let _ = writeln!(f, "{}", m);
                        let _ = f.flush();
                    }
                });
            }
        }
    });
}

/// Verifica si journalctl está disponible en el sistema
fn is_journalctl_available() -> bool {
    match std::process::Command::new("which")
        .arg("journalctl")
        .output()
    {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_creation() {
        let logger = Logger::with_custom_paths(1, "test", None, None, None);
        assert_eq!(logger.level, LogLevel::Info);
    }

    #[test]
    fn test_log_level_error() {
        let logger = Logger::with_custom_paths(3, "test", None, None, None);
        assert_eq!(logger.level, LogLevel::Error);
    }

    #[test]
    fn test_log_level_off() {
        let logger = Logger::with_custom_paths(0, "test", None, None, None);
        assert_eq!(logger.level, LogLevel::Off);
    }
}
