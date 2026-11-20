//! Hyprland Wayland compositor integration for Sonata window detection and opacity control.
//!
//! 1. **Window Discovery**: `find_terminal_window_address` prioritizes the focused terminal window
//!    (`activewindow -j`) for multi-window daemons and falls back to ancestor PID matching;
//!    `find_window_address_for_app` locates GUI windows by PID, class, or title.
//! 2. **Opacity IPC**: `lua_set_opacity_script` targets Hyprland's built-in Lua `hyprctl eval`
//!    engine first and falls back to `opacity_batch_command` (`hyprctl --batch`) on older releases.

use std::fs;
use std::process::Command as StdCommand;
use tokio::process::Command as AsyncCommand;

pub fn is_hyprland() -> bool {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
}

fn ancestor_pids(max_depth: usize) -> Vec<u32> {
    let mut pids = Vec::new();
    let mut cur = std::process::id();
    for _ in 0..max_depth {
        pids.push(cur);
        let Ok(stat) = fs::read_to_string(format!("/proc/{cur}/stat")) else {
            break;
        };
        let Some(rparen) = stat.rfind(')') else {
            break;
        };
        let mut fields = stat[rparen + 1..].split_whitespace();
        let _state = fields.next();
        let Some(ppid) = fields.next().and_then(|s| s.parse::<u32>().ok()) else {
            break;
        };
        if ppid <= 1 || ppid == cur {
            break;
        }
        cur = ppid;
    }
    pids
}

fn is_valid_hex_addr(addr: &str) -> bool {
    let Some(hex) = addr.strip_prefix("0x") else {
        return false;
    };
    !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_terminal_class(class: &str) -> bool {
    let lower = class.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    ["kitty", "ghostty", "wezterm", "alacritty"]
        .iter()
        .any(|term| lower.contains(term))
        || lower == "foot"
        || lower.starts_with("foot-")
        || lower.ends_with(".foot")
}

pub fn find_terminal_window_address() -> Option<String> {
    if !is_hyprland() {
        return None;
    }
    let ancestors = ancestor_pids(16);

    // Check activewindow first: single-instance daemons (Ghostty/Kitty) share one PID across windows.
    if let Ok(out) = StdCommand::new("hyprctl")
        .args(["activewindow", "-j"])
        .output()
    {
        if out.status.success() {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                let pid_matches = v
                    .get("pid")
                    .and_then(|p| p.as_u64())
                    .is_some_and(|pid| ancestors.contains(&(pid as u32)));
                let class_matches = v
                    .get("class")
                    .and_then(|c| c.as_str())
                    .is_some_and(is_terminal_class)
                    || v.get("initialClass")
                        .and_then(|c| c.as_str())
                        .is_some_and(is_terminal_class);
                if pid_matches || class_matches {
                    if let Some(addr) = v
                        .get("address")
                        .and_then(|v| v.as_str())
                        .filter(|a| is_valid_hex_addr(a))
                    {
                        return Some(addr.to_string());
                    }
                }
            }
        }
    }

    let out = StdCommand::new("hyprctl")
        .args(["clients", "-j"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let clients: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let arr = clients.as_array()?;
    for pid in &ancestors {
        if let Some(addr) = arr.iter().find_map(|c| {
            let cpid = c.get("pid").and_then(|v| v.as_u64())? as u32;
            if cpid == *pid {
                c.get("address")
                    .and_then(|v| v.as_str())
                    .filter(|a| is_valid_hex_addr(a))
                    .map(ToString::to_string)
            } else {
                None
            }
        }) {
            return Some(addr);
        }
    }
    None
}

fn match_app_client_address(
    arr: &[serde_json::Value],
    self_pid: u32,
    app_class: &str,
) -> Option<String> {
    let is_desktop_query = app_class.eq_ignore_ascii_case("sonata-desktop");
    let extract_addr = |client: &serde_json::Value| {
        client
            .get("address")
            .and_then(|v| v.as_str())
            .filter(|a| is_valid_hex_addr(a))
            .map(ToString::to_string)
    };

    // 1. Exact PID match.
    for client in arr {
        if client
            .get("pid")
            .and_then(|v| v.as_u64())
            .is_some_and(|pid| pid as u32 == self_pid)
        {
            if let Some(addr) = extract_addr(client) {
                return Some(addr);
            }
        }
    }

    // 2. Exact class / initialClass match, class containing "sonata", or title == "Sonata".
    for client in arr {
        let class_exact = client
            .get("class")
            .and_then(|v| v.as_str())
            .is_some_and(|c| c.eq_ignore_ascii_case(app_class))
            || client
                .get("initialClass")
                .and_then(|v| v.as_str())
                .is_some_and(|c| c.eq_ignore_ascii_case(app_class));
        let class_sonata = client
            .get("class")
            .and_then(|v| v.as_str())
            .is_some_and(|c| c.to_ascii_lowercase().contains("sonata"))
            || client
                .get("initialClass")
                .and_then(|v| v.as_str())
                .is_some_and(|c| c.to_ascii_lowercase().contains("sonata"));
        let title_matches = is_desktop_query
            && (client
                .get("title")
                .and_then(|v| v.as_str())
                .is_some_and(|t| t == "Sonata")
                || client
                    .get("initialTitle")
                    .and_then(|v| v.as_str())
                    .is_some_and(|t| t == "Sonata"));
        if class_exact || class_sonata || title_matches {
            if let Some(addr) = extract_addr(client) {
                return Some(addr);
            }
        }
    }
    None
}

/// Looks up the Hyprland window address for a GUI app process or Wayland `app_id` / class.
/// Returns `None` cleanly if Hyprland is not active or the window is not mapped yet.
pub fn find_window_address_for_app(app_class: &str) -> Option<String> {
    if !is_hyprland() {
        return None;
    }
    let self_pid = std::process::id();
    let out = StdCommand::new("hyprctl")
        .args(["clients", "-j"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let clients: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let arr = clients.as_array()?;
    match_app_client_address(arr, self_pid, app_class)
}

pub fn lua_set_opacity_script(addr: &str, opacity: f32) -> Option<String> {
    if !is_valid_hex_addr(addr) {
        return None;
    }
    let clamped = opacity.clamp(0.50, 1.00);
    Some(format!(
        "hl.dispatch(hl.dsp.window.set_prop({{ window = 'address:{addr}', prop = 'opacity', value = {clamped:.2} }})); \
         hl.dispatch(hl.dsp.window.set_prop({{ window = 'address:{addr}', prop = 'opacity_inactive', value = {clamped:.2} }})); \
         if hl.exec_scheduled_prop_refresh_immediately then hl.exec_scheduled_prop_refresh_immediately() end"
    ))
}

pub fn opacity_batch_command(addr: &str, opacity: f32) -> Option<String> {
    if !is_valid_hex_addr(addr) {
        return None;
    }
    let clamped = opacity.clamp(0.50, 1.00);
    Some(format!(
        "dispatch setprop address:{addr} alpha {clamped:.2} lock ; \
         dispatch setprop address:{addr} alphainactive {clamped:.2} lock ; \
         setprop address:{addr} alpha {clamped:.2} lock ; \
         setprop address:{addr} alphainactive {clamped:.2} lock ; \
         setprop address:{addr} opacity {clamped:.2} ; \
         setprop address:{addr} opacity_inactive {clamped:.2} ; \
         keyword windowrulev2 opacity {clamped:.2} {clamped:.2},address:{addr}"
    ))
}

fn eval_succeeded(out: &std::process::Output) -> bool {
    out.status.success()
        && !String::from_utf8_lossy(&out.stdout)
            .to_ascii_lowercase()
            .contains("unknown request")
}

pub async fn set_window_opacity(addr: &str, opacity: f32) {
    if let Some(script) = lua_set_opacity_script(addr, opacity) {
        if let Ok(out) = AsyncCommand::new("hyprctl")
            .args(["eval", &script])
            .output()
            .await
        {
            if eval_succeeded(&out) {
                return;
            }
        }
    }

    let Some(batch) = opacity_batch_command(addr, opacity) else {
        return;
    };
    if let Ok(out) = AsyncCommand::new("hyprctl")
        .args(["--batch", &batch])
        .output()
        .await
    {
        if !out.status.success() {
            tracing::debug!(
                addr = addr,
                status = ?out.status.code(),
                "hyprctl --batch opacity update returned non-zero status"
            );
        }
    }
}

pub fn set_window_opacity_sync(addr: &str, opacity: f32) {
    if let Some(script) = lua_set_opacity_script(addr, opacity) {
        if let Ok(out) = StdCommand::new("hyprctl").args(["eval", &script]).output() {
            if eval_succeeded(&out) {
                return;
            }
        }
    }

    let Some(batch) = opacity_batch_command(addr, opacity) else {
        return;
    };
    if let Ok(out) = StdCommand::new("hyprctl")
        .args(["--batch", &batch])
        .output()
    {
        if !out.status.success() {
            tracing::debug!(
                addr = addr,
                status = ?out.status.code(),
                "hyprctl --batch sync opacity update returned non-zero status"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_hex_window_addresses() {
        assert!(is_valid_hex_addr("0x55a1bc23ef00"));
        assert!(is_valid_hex_addr("0x1a2B"));
        assert!(!is_valid_hex_addr("0x"));
        assert!(!is_valid_hex_addr("55a1bc23ef00"));
        assert!(!is_valid_hex_addr("0xzz12"));
        assert!(!is_valid_hex_addr("0x123; dispatch exit"));
    }

    #[test]
    fn builds_lua_and_batch_opacity_commands_with_clamping() {
        assert_eq!(lua_set_opacity_script("invalid", 0.8), None);
        let lua = lua_set_opacity_script("0x55a1bc", 0.30).expect("valid lua script");
        assert!(lua.contains("window = 'address:0x55a1bc'"));
        assert!(lua.contains("prop = 'opacity', value = 0.50"));
        assert!(lua.contains("prop = 'opacity_inactive', value = 0.50"));
        assert!(lua.contains("exec_scheduled_prop_refresh_immediately"));

        assert_eq!(opacity_batch_command("invalid", 0.8), None);
        let cmd = opacity_batch_command("0x55a1bc", 0.25).expect("valid batch");
        assert!(cmd.contains("dispatch setprop address:0x55a1bc alpha 0.50 lock"));
        assert!(cmd.contains("dispatch setprop address:0x55a1bc alphainactive 0.50 lock"));
        assert!(cmd.contains("setprop address:0x55a1bc opacity 0.50"));
        assert!(cmd.contains("keyword windowrulev2 opacity 0.50 0.50,address:0x55a1bc"));

        let cmd_hi = opacity_batch_command("0x55a1bc", 1.50).expect("valid batch");
        assert!(cmd_hi.contains("alpha 1.00 lock"));
    }

    #[test]
    fn matches_app_window_by_pid_class_sonata_or_title() {
        let clients = vec![
            json!({ "pid": 999999u64, "class": "ghostty", "title": "Terminal", "address": "0x1111" }),
            json!({ "pid": 42u64, "class": "", "initialClass": "", "title": "Sonata", "address": "0x2222" }),
        ];
        assert_eq!(
            match_app_client_address(&clients, 12345, "sonata-desktop"),
            Some("0x2222".to_string())
        );
        assert_eq!(
            match_app_client_address(&clients, 999999, "sonata-desktop"),
            Some("0x1111".to_string())
        );
    }

    #[test]
    fn recognizes_terminal_window_classes() {
        assert!(is_terminal_class("kitty"));
        assert!(is_terminal_class("com.mitchellh.ghostty"));
        assert!(is_terminal_class("org.wezfurlong.wezterm"));
        assert!(is_terminal_class("foot"));
        assert!(is_terminal_class("Alacritty"));
        assert!(!is_terminal_class("firefox"));
        assert!(!is_terminal_class(""));
    }
}
