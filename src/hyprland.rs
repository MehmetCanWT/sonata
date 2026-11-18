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

fn lua_set_opacity_script(addr: &str, opacity: f32) -> Option<String> {
    if !is_valid_hex_addr(addr) {
        return None;
    }
    let clamped = opacity.clamp(0.30, 1.00);
    Some(format!(
        "hl.dsp.window.set_prop({{ window = 'address:{addr}', prop = 'alpha', value = {clamped:.2} }}); \
         hl.dsp.window.set_prop({{ window = 'address:{addr}', prop = 'alphainactive', value = {clamped:.2} }})"
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
    let _ = StdCommand::new("hyprctl")
        .args(["eval", &script])
        .output();
}
