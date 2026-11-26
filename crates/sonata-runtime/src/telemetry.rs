//! Persistent diagnostic log file initialization for Linux Sonata clients.
//!
//! 1. **Log File Writer**: Opens and rotates `$XDG_CACHE_HOME/sonata/sonata.log` using
//!    `tracing-appender`'s non-blocking writer, falling back to stderr on I/O failure.
//! 2. **Global Subscriber**: Installs a compact `tracing-subscriber` with `RUST_LOG` env filtering.

use crate::config::AppConfig;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::writer::BoxMakeWriter;
use tracing_subscriber::EnvFilter;

const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;

#[must_use]
pub struct TelemetryGuard {
    _worker_guard: Option<WorkerGuard>,
}

pub fn init(client: &'static str) -> TelemetryGuard {
    let log_path = AppConfig::cache_dir().join("sonata.log");
    init_at(client, log_path)
}

fn init_at(client: &'static str, log_path: PathBuf) -> TelemetryGuard {
    let (writer, worker_guard, persistent) = match open_log_file(&log_path) {
        Ok(file) => {
            let (writer, guard) = tracing_appender::non_blocking(file);
            (BoxMakeWriter::new(writer), Some(guard), true)
        }
        Err(_) => (BoxMakeWriter::new(std::io::stderr), None, false),
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .with_target(false)
        .compact()
        .finish();
    // UI dependencies may install a `log` facade first. Register tracing
    // directly so an existing facade cannot reject the subscriber too.
    let initialized = tracing::subscriber::set_global_default(subscriber).is_ok();
    if initialized {
        tracing::info!(
            event = "startup",
            client,
            log_path = %log_path.display(),
            persistent,
            version = env!("CARGO_PKG_VERSION"),
            "logging initialized"
        );
    }
    TelemetryGuard {
        _worker_guard: worker_guard,
    }
}

fn open_log_file(path: &Path) -> std::io::Result<File> {
    if let Some(parent) = path.parent() {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder.create(parent)?;
    }
    let truncate = fs::metadata(path).is_ok_and(|metadata| metadata.len() > MAX_LOG_BYTES);
    let mut options = OpenOptions::new();
    options.create(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    if truncate {
        options.truncate(true);
    } else {
        options.append(true);
    }
    let file = options.open(path)?;
    restrict_log_permissions(path);
    Ok(file)
}

#[cfg(unix)]
fn restrict_log_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_log_permissions(_path: &Path) {}

pub fn log_path() -> PathBuf {
    AppConfig::cache_dir().join("sonata.log")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn writes_and_flushes_persistent_log() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sonata-telemetry-{}-{unique}.log",
            std::process::id()
        ));

        let guard = init_at("test", path.clone());
        tracing::warn!(event = "telemetry_test", "telemetry test event");
        drop(guard);

        let contents = fs::read_to_string(&path).expect("telemetry log must be readable");
        assert!(contents.contains("telemetry test event"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = fs::metadata(&path)
                .expect("log metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
        let _ = fs::remove_file(path);
    }
}
