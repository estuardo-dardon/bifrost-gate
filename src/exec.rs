use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, Clone)]
pub struct ExecConfig {
    pub default_timeout: Duration,
    pub max_output_bytes: usize,
}

impl Default for ExecConfig {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(15),
            max_output_bytes: 64 * 1024,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExecOutput {
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum ExecError {
    Timeout { program: String },
    Spawn { program: String, message: String },
    OutputTooLarge { program: String },
}

fn truncate_to_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…[truncado]", &s[..end])
}

pub async fn run_command(
    cfg: &ExecConfig,
    program: &str,
    args: &[&str],
    timeout_override: Option<Duration>,
) -> Result<ExecOutput, ExecError> {
    let mut cmd = Command::new(program);
    cmd.args(args);

    let program_name = program.to_string();
    let dur = timeout_override.unwrap_or(cfg.default_timeout);

    let output = match timeout(dur, cmd.output()).await {
        Ok(res) => res.map_err(|err| ExecError::Spawn {
            program: program_name.clone(),
            message: err.to_string(),
        })?,
        Err(_) => return Err(ExecError::Timeout { program: program_name }),
    };

    let stdout_raw = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr_raw = String::from_utf8_lossy(&output.stderr).to_string();

    if stdout_raw.len() > cfg.max_output_bytes || stderr_raw.len() > cfg.max_output_bytes {
        return Err(ExecError::OutputTooLarge {
            program: program_name,
        });
    }

    Ok(ExecOutput {
        status_code: output.status.code(),
        stdout: truncate_to_bytes(stdout_raw.trim(), cfg.max_output_bytes),
        stderr: truncate_to_bytes(stderr_raw.trim(), cfg.max_output_bytes),
    })
}

