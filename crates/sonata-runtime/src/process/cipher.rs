//! Local CLI JavaScript runtime bridge (`node`, `bun`, `deno`) for YouTube cipher solving.
//!
//! 1. **Runtime Discovery**: Probes `node`, `bun`, and `deno` once via cross-distro binary lookup.
//! 2. **Atomic Script Cache**: Persists hooked `base.js` scripts with `0o600` file permissions
//!    inside `CorePaths::cipher_script_cache_path(...)` before evaluation.
//! 3. **Bounded Subprocess Execution**: Evaluates `globalThis._yt_solve(sig, n)` with a hard
//!    wall-clock deadline, killing and reaping hung subprocesses on timeout.

use crate::config::{configure_background_std_command, resolve_distro_binary};
use sonata_core::auth::ensure_private_dir;
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
const DENO_CIPHER_RUNNER_SCRIPT: &str =
    "(0,eval)(Deno.readTextFileSync(Deno.args[0]));console.log(globalThis._yt_solve(Deno.args[1]||'',Deno.args[2]||''));\n";

struct TempRunnerGuard(PathBuf);

impl Drop for TempRunnerGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn create_deno_runner_script(dir: &Path) -> std::io::Result<TempRunnerGuard> {
    ensure_private_dir(dir)?;
    let seq = CLI_CIPHER_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = dir.join(format!(
        "yt_cipher_runner_{}_{}.js",
        std::process::id(),
        seq
    ));
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    opts.mode(0o600);
    let mut file = opts.open(&path)?;
    #[cfg(unix)]
    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    if let Err(err) = file.write_all(DENO_CIPHER_RUNNER_SCRIPT.as_bytes()) {
        let _ = fs::remove_file(&path);
        return Err(err);
    }
    Ok(TempRunnerGuard(path))
}

fn build_deno_run_args(runner_path: &Path, script_path: &Path, sig: &str, n: &str) -> Vec<String> {
    let runner = runner_path.to_string_lossy().into_owned();
    let script = script_path.to_string_lossy().into_owned();
    vec![
        "run".to_string(),
        "--no-prompt".to_string(),
        "--no-remote".to_string(),
        format!("--allow-read={script}"),
        runner,
        script,
        sig.to_string(),
        n.to_string(),
    ]
}

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
            let mut cmd = Command::new(&bin);
            configure_background_std_command(&mut cmd);
            cmd.arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        })
    })
}

fn write_private_cache_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    ensure_private_dir(parent)?;

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

        let runtime_bin = resolve_distro_binary(runtime);
        let mut cmd = Command::new(&runtime_bin);
        configure_background_std_command(&mut cmd);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let _runner_guard = if runtime == "deno" {
            let runner_dir = js_path.parent().unwrap_or(&self.paths.cache_dir);
            let runner = create_deno_runner_script(runner_dir).ok()?;
            let args = build_deno_run_args(&runner.0, &js_path, sig, n);
            cmd.args(&args);
            Some(runner)
        } else {
            let js_path_str = js_path.to_string_lossy().to_string();
            cmd.args([
                "-e",
                "const fs=require('fs');(0,eval)(fs.readFileSync(process.argv[1],'utf8'));console.log(globalThis._yt_solve(process.argv[2]||'',process.argv[3]||''));",
                "--",
                &js_path_str,
                sig,
                n,
            ]);
            None
        };

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloads::test_support::TempTestDir;

    #[test]
    fn deno_invocation_uses_sandboxed_run_with_scoped_read_only_permission() {
        let dir = TempTestDir::new("cipher_deno_sandbox");
        let script = dir.path().join("yt_cipher_abc.js");
        let runner = create_deno_runner_script(dir.path()).expect("create runner");
        let runner_path = runner.0.clone();

        #[cfg(unix)]
        {
            let mode = fs::metadata(&runner_path)
                .expect("runner metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        let args = build_deno_run_args(&runner_path, &script, "sig_val", "n_val");
        assert_eq!(args[0], "run");
        assert!(args.iter().any(|a| a == "--no-prompt"));
        assert!(args.iter().any(|a| a == "--no-remote"));
        assert!(args
            .iter()
            .any(|a| a == &format!("--allow-read={}", script.display())));
        assert!(!args.iter().any(|a| a == "eval"
            || a.starts_with("--allow-net")
            || a.starts_with("--allow-write")
            || a.starts_with("--allow-env")
            || a.starts_with("--allow-run")));

        drop(runner);
        assert!(!runner_path.exists());
    }
}
