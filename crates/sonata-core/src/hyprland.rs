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

pub fn find_terminal_window_address() -> Option<String> {
    if !is_hyprland() {
        return None;
    }
    let ancestors = ancestor_pids(10);
    if let Ok(out) = StdCommand::new("hyprctl").args(["clients", "-j"]).output() {
        if out.status.success() {
            if let Ok(clients) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                if let Some(arr) = clients.as_array() {
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
                }
            }
        }
    }

    let out = StdCommand::new("hyprctl")
        .args(["activewindow", "-j"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    v.get("address")
        .and_then(|v| v.as_str())
        .filter(|a| is_valid_hex_addr(a))
        .map(ToString::to_string)
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

    // Prefer exact PID match first, then Wayland class / initialClass match.
    for by_pid in [true, false] {
        for client in arr {
            let matches = if by_pid {
                client
                    .get("pid")
                    .and_then(|v| v.as_u64())
                    .is_some_and(|pid| pid as u32 == self_pid)
            } else {
                client
                    .get("class")
                    .and_then(|v| v.as_str())
                    .is_some_and(|c| c.eq_ignore_ascii_case(app_class))
                    || client
                        .get("initialClass")
                        .and_then(|v| v.as_str())
                        .is_some_and(|c| c.eq_ignore_ascii_case(app_class))
            };
            if matches {
                if let Some(addr) = client
                    .get("address")
                    .and_then(|v| v.as_str())
                    .filter(|a| is_valid_hex_addr(a))
                {
                    return Some(addr.to_string());
                }
            }
        }
    }
    None
}

fn lua_set_opacity_script(addr: &str, opacity: f32) -> Option<String> {
    if !is_valid_hex_addr(addr) {
        return None;
    }
    let clamped = opacity.clamp(0.50, 1.00);
    Some(format!(
        "hl.dispatch(hl.dsp.window.set_prop({{ window = 'address:{addr}', prop = 'opacity', value = {clamped:.2} }})); \
         hl.dispatch(hl.dsp.window.set_prop({{ window = 'address:{addr}', prop = 'opacity_inactive', value = {clamped:.2} }}))"
    ))
}

pub async fn set_window_opacity(addr: &str, opacity: f32) {
    let Some(script) = lua_set_opacity_script(addr, opacity) else {
        return;
    };
    let _ = AsyncCommand::new("hyprctl")
        .args(["eval", &script])
        .output()
        .await;
}

pub fn set_window_opacity_sync(addr: &str, opacity: f32) {
    let Some(script) = lua_set_opacity_script(addr, opacity) else {
        return;
    };
    let _ = StdCommand::new("hyprctl").args(["eval", &script]).output();
}
