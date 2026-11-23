//! Local CLI JavaScript runtime bridge (`node`, `bun`, `deno`) for YouTube cipher solving.
//!
//! 1. **Runtime Discovery**: Probes `node`, `bun`, and `deno` once via cross-distro binary lookup.
//! 2. **Atomic Script Cache**: Persists hooked `base.js` scripts with `0o600` file permissions
//!    inside `CorePaths::cipher_script_cache_path(...)` before evaluation.
//! 3. **Bounded Subprocess Execution**: Evaluates `globalThis._yt_solve(sig, n)` with a hard
//!    wall-clock deadline, killing and reaping hung subprocesses on timeout.

use crate::config::resolve_distro_binary;
use sonata_core::paths::CorePaths;
use sonata_core::providers::hifi::{JsCipherBridge, SolvedCipherPair};
use std::fs::{self, OpenOptions};
use std::io::{Read as _, Write as _};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

static CLI_CIPHER_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const CIPHER_TIMEOUT: Duration = Duration::from_millis(3500);
const CIPHER_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Local CLI implementation of [`JsCipherBridge`] backed by `node`, `bun`, or `deno`.
#[derive(Debug, Clone)]
pub struct CliJsCipherBridge {
    paths: CorePaths,
}

impl CliJsCipherBridge {
    /// Creates a CLI cipher bridge that caches hooked player scripts under `paths`.
    pub fn new(paths: CorePaths) -> Self {
        Self { paths }
    }

    fn script_path(&self, player_hash: &str) -> PathBuf {
        let safe_hash: String = player_hash
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        self.paths.cipher_script_cache_path(&safe_hash)
    }
}

pub(crate) fn detect_cli_cipher_runtime() -> Option<&'static str> {
    static CLI_RUNTIME: OnceLock<Option<&'static str>> = OnceLock::new();
    *CLI_RUNTIME.get_or_init(|| {
        ["node", "bun", "deno"].into_iter().find(|&candidate| {
            let bin = resolve_distro_binary(candidate);
            Command::new(&bin)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        })
    })
}

fn write_private_cache_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    #[cfg(unix)]
    let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("cipher_cache");
    let seq = CLI_CIPHER_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = parent.join(format!("{file_name}.{}.{}.tmp", std::process::id(), seq));

    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    opts.mode(0o600);
    let write_res = (|| -> std::io::Result<()> {
        let mut file = opts.open(&tmp)?;
        #[cfg(unix)]
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if write_res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    write_res
}

impl JsCipherBridge for CliJsCipherBridge {
    fn ensure_loaded(&self, player_hash: &str, hooked_script: &str) -> bool {
        if detect_cli_cipher_runtime().is_none() {
            return false;
        }
        let js_path = self.script_path(player_hash);
        if js_path.is_file() {
            return true;
        }
        write_private_cache_file(&js_path, hooked_script.as_bytes()).is_ok()
    }

    fn solve(&self, player_hash: &str, sig: &str, n: &str) -> Option<SolvedCipherPair> {
        let runtime = detect_cli_cipher_runtime()?;
        let js_path = self.script_path(player_hash);
        if !js_path.is_file() {
            return None;
        }

        let js_path_str = js_path.to_string_lossy().to_string();
        let runtime_bin = resolve_distro_binary(runtime);
        let mut cmd = Command::new(&runtime_bin);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if runtime == "deno" {
            cmd.args([
                "eval",
                "(0,eval)(Deno.readTextFileSync(Deno.args[0]));console.log(globalThis._yt_solve(Deno.args[1]||'',Deno.args[2]||''));",
                "--",
                &js_path_str,
                sig,
                n,
            ]);
        } else {
            cmd.args([
                "-e",
                "const fs=require('fs');(0,eval)(fs.readFileSync(process.argv[1],'utf8'));console.log(globalThis._yt_solve(process.argv[2]||'',process.argv[3]||''));",
                "--",
                &js_path_str,
                sig,
                n,
            ]);
        }

        let mut child = cmd.spawn().ok()?;
        let deadline = Instant::now() + CIPHER_TIMEOUT;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return None;
                    }
                    std::thread::sleep(CIPHER_POLL_INTERVAL);
                }
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
            }
        };

        if !status.success() {
            return None;
        }

        let mut stdout_bytes = Vec::new();
        child.stdout.take()?.read_to_end(&mut stdout_bytes).ok()?;
        let stdout = String::from_utf8_lossy(&stdout_bytes);
        let line = stdout
            .lines()
            .rev()
            .find(|l| l.trim_start().starts_with('{'))?;
        serde_json::from_str::<SolvedCipherPair>(line.trim()).ok()
    }
}
