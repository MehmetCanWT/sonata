//! Local CLI JavaScript runtime bridge (`deno` or sandboxed `node --permission`) for YouTube cipher solving.
//!
//! 1. **Sandboxed Runtime Discovery**: Prefers `deno` (`--no-prompt --no-remote --allow-read=...`)
//!    and falls back only to Node.js permission-model sandboxing (`--permission` / `--experimental-permission`),
//!    refusing unsandboxed `node` or `bun` execution.
//! 2. **Atomic Script Cache**: Persists hooked `base.js` scripts with `0o600` file permissions
//!    inside `CorePaths::cipher_script_cache_path(...)` before evaluation.
//! 3. **Bounded Subprocess Execution**: Evaluates `globalThis._yt_solve(sig, n)` with cleared secrets
//!    in environment variables and a hard wall-clock deadline, killing and reaping hung subprocesses on timeout.

use crate::config::{configure_background_std_command, resolve_distro_binary};
use sonata_core::auth::ensure_private_dir;
use sonata_core::connect::generate_request_id;
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
const NODE_CIPHER_RUNNER_SCRIPT: &str =
    "const fs=require('fs');(0,eval)(fs.readFileSync(process.argv[1],'utf8'));console.log(globalThis._yt_solve(process.argv[2]||'',process.argv[3]||''));";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CipherRuntime {
    Deno,
    NodePermission { flag: &'static str },
}

impl CipherRuntime {
    fn binary_name(self) -> &'static str {
        match self {
            Self::Deno => "deno",
            Self::NodePermission { .. } => "node",
        }
    }
}

struct TempRunnerGuard(PathBuf);

impl Drop for TempRunnerGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn apply_clean_subprocess_env(cmd: &mut Command) {
    let path_var = std::env::var_os("PATH");
    #[cfg(windows)]
    let system_root = std::env::var_os("SystemRoot").or_else(|| std::env::var_os("SYSTEMROOT"));
    #[cfg(windows)]
    let comspec = std::env::var_os("COMSPEC");

    cmd.env_clear();
    if let Some(path) = path_var {
        cmd.env("PATH", path);
    }
    #[cfg(windows)]
    {
        if let Some(root) = system_root {
            cmd.env("SystemRoot", root);
        }
        if let Some(shell) = comspec {
            cmd.env("COMSPEC", shell);
        }
    }
}

fn create_deno_runner_script(dir: &Path) -> std::io::Result<TempRunnerGuard> {
    ensure_private_dir(dir)?;
    let seq = CLI_CIPHER_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let rand_suffix = generate_request_id();
    let path = dir.join(format!(
        "yt_cipher_runner_{}_{seq}_{rand_suffix}.js",
        std::process::id(),
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

fn build_node_sandbox_args(
    permission_flag: &str,
    script_path: &Path,
    sig: &str,
    n: &str,
) -> Vec<String> {
    let script = script_path.to_string_lossy().into_owned();
    vec![
        permission_flag.to_string(),
        format!("--allow-fs-read={script}"),
        "-e".to_string(),
        NODE_CIPHER_RUNNER_SCRIPT.to_string(),
        "--".to_string(),
        script,
        sig.to_string(),
        n.to_string(),
    ]
}

fn binary_succeeds(bin: &str, args: &[&str]) -> bool {
    let mut cmd = Command::new(bin);
    configure_background_std_command(&mut cmd);
    apply_clean_subprocess_env(&mut cmd);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Local CLI implementation of [`JsCipherBridge`] backed by sandboxed `deno` or `node --permission`.
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

pub(crate) fn detect_cli_cipher_runtime() -> Option<CipherRuntime> {
    static CLI_RUNTIME: OnceLock<Option<CipherRuntime>> = OnceLock::new();
    *CLI_RUNTIME.get_or_init(|| {
        let deno_bin = resolve_distro_binary("deno");
        if binary_succeeds(&deno_bin, &["--version"]) {
            return Some(CipherRuntime::Deno);
        }

        let node_bin = resolve_distro_binary("node");
        for flag in ["--permission", "--experimental-permission"] {
            if binary_succeeds(&node_bin, &[flag, "-e", ""]) {
                return Some(CipherRuntime::NodePermission { flag });
            }
        }

        if binary_succeeds(&node_bin, &["--version"])
            || binary_succeeds(&resolve_distro_binary("bun"), &["--version"])
        {
            tracing::warn!(
                "Refusing unsandboxed `node`/`bun` runtime for YouTube JS cipher evaluation; install `deno` or Node.js with `--permission`"
            );
        }
        None
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
    let rand_suffix = generate_request_id();
    let tmp = parent.join(format!(
        "{file_name}.{}.{seq}.{rand_suffix}.tmp",
        std::process::id(),
    ));

    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
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
        let canonical_js_path = fs::canonicalize(&js_path).unwrap_or(js_path);

        let runtime_bin = resolve_distro_binary(runtime.binary_name());
        let mut cmd = Command::new(&runtime_bin);
        configure_background_std_command(&mut cmd);
        apply_clean_subprocess_env(&mut cmd);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let _runner_guard = match runtime {
            CipherRuntime::Deno => {
                let runner_dir = canonical_js_path.parent().unwrap_or(&self.paths.cache_dir);
                let runner = create_deno_runner_script(runner_dir).ok()?;
                let args = build_deno_run_args(&runner.0, &canonical_js_path, sig, n);
                cmd.args(&args);
                Some(runner)
            }
            CipherRuntime::NodePermission { flag } => {
                let args = build_node_sandbox_args(flag, &canonical_js_path, sig, n);
                cmd.args(&args);
                None
            }
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

    #[test]
    fn node_invocation_enforces_permission_sandbox_flags() {
        let script = PathBuf::from("/tmp/sonata-test/yt_cipher_xyz.js");
        let args = build_node_sandbox_args("--permission", &script, "sig_1", "n_1");
        assert_eq!(args[0], "--permission");
        assert_eq!(args[1], "--allow-fs-read=/tmp/sonata-test/yt_cipher_xyz.js");
        assert!(!args.iter().any(|a| a == "--allow-fs-write"
            || a.starts_with("--allow-fs-write=")
            || a == "--allow-child-process"
            || a == "--allow-worker"));
    }

    #[test]
    fn cipher_bridge_solves_and_blocks_arbitrary_file_read() {
        if detect_cli_cipher_runtime().is_none() {
            return;
        }
        let dir = TempTestDir::new("cipher_sandbox_exec");
        let paths = CorePaths::new(dir.path().join("cfg"), dir.path().join("cache"));
        let bridge = CliJsCipherBridge::new(paths);

        let solver_js = "globalThis._yt_solve=function(sig,n){return JSON.stringify({sig:'ok:'+sig,n:'ok:'+n});};";
        assert!(bridge.ensure_loaded("goodhash1", solver_js));
        let solved = bridge.solve("goodhash1", "s1", "n1").expect("solve valid");
        assert_eq!(solved.sig.as_deref(), Some("ok:s1"));
        assert_eq!(solved.n.as_deref(), Some("ok:n1"));

        let secret_path = dir.path().join("secret.txt");
        fs::write(&secret_path, b"do-not-read").expect("write secret");
        let evil_js = format!(
            "const f = {}; globalThis._yt_solve = function() {{ const data = (typeof Deno !== 'undefined') ? Deno.readTextFileSync(f) : require('fs').readFileSync(f, 'utf8'); return JSON.stringify({{sig:data,n:null}}); }};",
            serde_json::to_string(&secret_path.to_string_lossy()).unwrap()
        );
        assert!(bridge.ensure_loaded("evilhash1", &evil_js));
        assert!(
            bridge.solve("evilhash1", "s", "n").is_none(),
            "sandboxed JS runtime must block reading files outside the cipher script"
        );
    }
}
