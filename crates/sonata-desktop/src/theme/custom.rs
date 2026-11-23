//! Built-in and user CSS theme loader, `.offered` seeder, parser, and fuzzy `--scheme` lookup.
//!
//! 1. **Built-in Catalog**: embeds all 29 shipped `.css` themes at compile time and seeds them into `~/.config/sonata/themes/` once using `.offered`.
//! 2. **CSS Parser**: strips comments, reads `color-scheme` and `--var: value;` declarations regardless of braces, and reports line-numbered errors or unknown variables.
//! 3. **Folder Discovery**: loads valid `.css` themes sorted alphabetically and supports accent/case/hyphen-insensitive lookup.

use super::ramp::{derive_theme_tokens, Rgb, ThemeTokens};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub struct BuiltinThemeFile {
    pub file_name: &'static str,
    pub css: &'static str,
}

pub const BUILTIN_THEMES: &[BuiltinThemeFile] = &[
    BuiltinThemeFile {
        file_name: "Breeze Dark.css",
        css: include_str!("../../themes/Breeze Dark.css"),
    },
    BuiltinThemeFile {
        file_name: "Breeze Light.css",
        css: include_str!("../../themes/Breeze Light.css"),
    },
    BuiltinThemeFile {
        file_name: "Catppuccin Frappé.css",
        css: include_str!("../../themes/Catppuccin Frappé.css"),
    },
    BuiltinThemeFile {
        file_name: "Catppuccin Latte.css",
        css: include_str!("../../themes/Catppuccin Latte.css"),
    },
    BuiltinThemeFile {
        file_name: "Catppuccin Macchiato.css",
        css: include_str!("../../themes/Catppuccin Macchiato.css"),
    },
    BuiltinThemeFile {
        file_name: "Catppuccin Mocha.css",
        css: include_str!("../../themes/Catppuccin Mocha.css"),
    },
    BuiltinThemeFile {
        file_name: "Classic Dark.css",
        css: include_str!("../../themes/Classic Dark.css"),
    },
    BuiltinThemeFile {
        file_name: "Classic Light.css",
        css: include_str!("../../themes/Classic Light.css"),
    },
    BuiltinThemeFile {
        file_name: "Dracula.css",
        css: include_str!("../../themes/Dracula.css"),
    },
    BuiltinThemeFile {
        file_name: "Dracula Alucard.css",
        css: include_str!("../../themes/Dracula Alucard.css"),
    },
    BuiltinThemeFile {
        file_name: "Everforest Dark.css",
        css: include_str!("../../themes/Everforest Dark.css"),
    },
    BuiltinThemeFile {
        file_name: "Everforest Light.css",
        css: include_str!("../../themes/Everforest Light.css"),
    },
    BuiltinThemeFile {
        file_name: "Gruvbox Dark.css",
        css: include_str!("../../themes/Gruvbox Dark.css"),
    },
    BuiltinThemeFile {
        file_name: "Gruvbox Light.css",
        css: include_str!("../../themes/Gruvbox Light.css"),
    },
    BuiltinThemeFile {
        file_name: "macOS Dark.css",
        css: include_str!("../../themes/macOS Dark.css"),
    },
    BuiltinThemeFile {
        file_name: "macOS Light.css",
        css: include_str!("../../themes/macOS Light.css"),
    },
    BuiltinThemeFile {
        file_name: "Nord Polar Night.css",
        css: include_str!("../../themes/Nord Polar Night.css"),
    },
    BuiltinThemeFile {
        file_name: "Nord Snow Storm.css",
        css: include_str!("../../themes/Nord Snow Storm.css"),
    },
    BuiltinThemeFile {
        file_name: "Rosé Pine.css",
        css: include_str!("../../themes/Rosé Pine.css"),
    },
    BuiltinThemeFile {
        file_name: "Rosé Pine Dawn.css",
        css: include_str!("../../themes/Rosé Pine Dawn.css"),
    },
    BuiltinThemeFile {
        file_name: "Rosé Pine Moon.css",
        css: include_str!("../../themes/Rosé Pine Moon.css"),
    },
    BuiltinThemeFile {
        file_name: "Solarized Dark.css",
        css: include_str!("../../themes/Solarized Dark.css"),
    },
    BuiltinThemeFile {
        file_name: "Solarized Light.css",
        css: include_str!("../../themes/Solarized Light.css"),
    },
    BuiltinThemeFile {
        file_name: "Spotify.css",
        css: include_str!("../../themes/Spotify.css"),
    },
    BuiltinThemeFile {
        file_name: "Tokyo Night.css",
        css: include_str!("../../themes/Tokyo Night.css"),
    },
    BuiltinThemeFile {
        file_name: "Tokyo Night Day.css",
        css: include_str!("../../themes/Tokyo Night Day.css"),
    },
    BuiltinThemeFile {
        file_name: "Tokyo Night Storm.css",
        css: include_str!("../../themes/Tokyo Night Storm.css"),
    },
    BuiltinThemeFile {
        file_name: "Windows Dark.css",
        css: include_str!("../../themes/Windows Dark.css"),
    },
    BuiltinThemeFile {
        file_name: "Windows Light.css",
        css: include_str!("../../themes/Windows Light.css"),
    },
];

const KNOWN_VARS: &[&str] = &[
    "--base",
    "--text",
    "--accent",
    "--gutter",
    "--popup",
    "--surface",
    "--surface-hover",
    "--surface-active",
    "--outline",
    "--subtext",
    "--muted",
    "--accent-hover",
    "--on-accent",
    "--danger",
    "--warning",
];

#[derive(Debug, Clone)]
pub struct LoadedThemes {
    pub themes: Vec<ThemeTokens>,
    pub errors: Vec<String>,
    pub warnings_by_name: HashMap<String, Vec<String>>,
}

/// Seeds newly introduced built-in `.css` files once, honoring deleted files via `.offered`.
pub fn seed_builtin_themes(dir: &Path) {
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    let offered_path = dir.join(".offered");
    let mut offered: HashSet<String> = fs::read_to_string(&offered_path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let mut changed = false;
    for builtin in BUILTIN_THEMES {
        if offered.contains(builtin.file_name) {
            continue;
        }
        let target = dir.join(builtin.file_name);
        if !target.exists() {
            let _ = fs::write(&target, builtin.css);
        }
        offered.insert(builtin.file_name.to_string());
        changed = true;
    }

    if changed {
        let mut lines: Vec<String> = offered.into_iter().collect();
        lines.sort();
        let _ = fs::write(offered_path, lines.join("\n"));
    }
}

pub fn load_themes_dir(dir: &Path) -> LoadedThemes {
    seed_builtin_themes(dir);
    let mut themes = Vec::new();
    let mut errors = Vec::new();
    let mut warnings_by_name: HashMap<String, Vec<String>> = HashMap::new();

    let mut entries: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("css"))
            .collect(),
        Err(_) => Vec::new(),
    };
    entries.sort_by_key(|p| {
        p.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
    });

    for path in entries {
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("theme.css");
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unnamed")
            .to_string();
        match fs::read_to_string(&path) {
            Ok(css) => match parse_css_theme(&name, &css) {
                Ok((tokens, mut warns)) => {
                    if let Some(low) = &tokens.low_contrast_warning {
                        warns.push(low.clone());
                    }
                    if !warns.is_empty() {
                        warnings_by_name.insert(tokens.name.clone(), warns);
                    }
                    themes.push(tokens);
                }
                Err(reason) => {
                    errors.push(format!("{file_name}: {reason}"));
                }
            },
            Err(err) => {
                errors.push(format!("{file_name}: {err}"));
            }
        }
    }

    LoadedThemes {
        themes,
        errors,
        warnings_by_name,
    }
}

pub fn parse_css_theme(name: &str, css: &str) -> Result<(ThemeTokens, Vec<String>), String> {
    let cleaned = strip_css_comments_preserving_lines(css);
    let mut vars: HashMap<String, Rgb> = HashMap::new();
    let mut scheme: Option<bool> = None;
    let mut unknown = Vec::new();

    let mut buf = String::new();
    let mut stmt_line = 1usize;
    let mut in_stmt = false;

    for (idx, line) in cleaned.lines().enumerate() {
        let line_no = idx + 1;
        for ch in line.chars() {
            if ch == '{' || ch == '}' {
                if !buf.trim().is_empty() {
                    process_decl(&buf, stmt_line, &mut scheme, &mut vars, &mut unknown)?;
                }
                buf.clear();
                in_stmt = false;
                continue;
            }
            if ch == ';' {
                if !buf.trim().is_empty() {
                    process_decl(&buf, stmt_line, &mut scheme, &mut vars, &mut unknown)?;
                }
                buf.clear();
                in_stmt = false;
                continue;
            }
            if !in_stmt && !ch.is_whitespace() {
                stmt_line = line_no;
                in_stmt = true;
            }
            if in_stmt {
                buf.push(ch);
            }
        }
        if in_stmt {
            buf.push(' ');
        }
    }
    if !buf.trim().is_empty() {
        process_decl(&buf, stmt_line, &mut scheme, &mut vars, &mut unknown)?;
    }

    let tokens = derive_theme_tokens(name.to_string(), scheme, &vars)?;
    let warns = if unknown.is_empty() {
        Vec::new()
    } else {
        vec![format!("Unknown variable(s): {}", unknown.join(", "))]
    };
    Ok((tokens, warns))
}

fn process_decl(
    decl: &str,
    line_no: usize,
    scheme: &mut Option<bool>,
    vars: &mut HashMap<String, Rgb>,
    unknown: &mut Vec<String>,
) -> Result<(), String> {
    let trimmed = decl.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let Some((key_raw, val_raw)) = trimmed.split_once(':') else {
        return Err(format!("line {line_no}: expected `key: value`"));
    };
    let key = key_raw.trim().to_ascii_lowercase();
    let val = val_raw.trim();

    if key == "color-scheme" {
        match val.to_ascii_lowercase().as_str() {
            "dark" => *scheme = Some(true),
            "light" => *scheme = Some(false),
            _ => return Err(format!("line {line_no}: invalid color-scheme `{val}`")),
        }
        return Ok(());
    }

    if key.starts_with("--") {
        let Some(rgb) = Rgb::parse_css(val) else {
            return Err(format!("line {line_no}: `{val}` is not a colour"));
        };
        if KNOWN_VARS.contains(&key.as_str()) {
            vars.insert(key, rgb);
        } else if !unknown.contains(&key) {
            unknown.push(key);
        }
        return Ok(());
    }

    Ok(())
}

fn strip_css_comments_preserving_lines(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                if bytes[i] == b'\n' {
                    out.push('\n');
                }
                i += 1;
            }
            if i + 1 < bytes.len() {
                i += 2;
            }
        } else {
            let ch = input[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

pub fn builtin_fallback(is_dark: bool) -> ThemeTokens {
    let file = if is_dark {
        BUILTIN_THEMES
            .iter()
            .find(|t| t.file_name == "Classic Dark.css")
    } else {
        BUILTIN_THEMES
            .iter()
            .find(|t| t.file_name == "Classic Light.css")
    };
    let name = if is_dark {
        "Classic Dark"
    } else {
        "Classic Light"
    };
    if let Some(f) = file {
        if let Ok((tokens, _)) = parse_css_theme(name, f.css) {
            return tokens;
        }
    }
    let mut vars = HashMap::new();
    if is_dark {
        vars.insert("--base".to_string(), Rgb::from_u8(20, 22, 25));
        vars.insert("--text".to_string(), Rgb::from_u8(243, 244, 246));
        vars.insert("--accent".to_string(), Rgb::from_u8(255, 51, 61));
    } else {
        vars.insert("--base".to_string(), Rgb::from_u8(255, 255, 255));
        vars.insert("--text".to_string(), Rgb::from_u8(19, 22, 25));
        vars.insert("--accent".to_string(), Rgb::from_u8(224, 33, 43));
    }
    derive_theme_tokens(name.to_string(), Some(is_dark), &vars).expect("builtin fallback valid")
}

/// Normalizes a theme name ignoring case, spaces, hyphens, and common accented letters (`é` -> `e`).
pub fn normalize_scheme_key(name: &str) -> String {
    name.trim()
        .chars()
        .filter_map(|c| {
            let folded = match c {
                'é' | 'è' | 'ê' | 'ë' | 'É' => 'e',
                'á' | 'à' | 'â' | 'ä' => 'a',
                'í' | 'ì' | 'î' | 'ï' => 'i',
                'ó' | 'ò' | 'ô' | 'ö' => 'o',
                'ú' | 'ù' | 'û' | 'ü' => 'u',
                _ => c,
            };
            if folded.is_ascii_alphanumeric() {
                Some(folded.to_ascii_lowercase())
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_29_builtin_css_themes_parse_without_errors() {
        assert_eq!(BUILTIN_THEMES.len(), 29);
        for theme in BUILTIN_THEMES {
            let name = theme.file_name.trim_end_matches(".css");
            let parsed = parse_css_theme(name, theme.css);
            assert!(
                parsed.is_ok(),
                "theme {} failed: {:?}",
                theme.file_name,
                parsed
            );
        }
    }

    #[test]
    fn fuzzy_scheme_matching_handles_accents_and_hyphens() {
        assert_eq!(
            normalize_scheme_key("Catppuccin Frappé"),
            "catppuccinfrappe"
        );
        assert_eq!(
            normalize_scheme_key("catppuccin-frappe"),
            "catppuccinfrappe"
        );
        assert_eq!(normalize_scheme_key("Rosé Pine"), "rosepine");
    }

    #[test]
    fn invalid_css_reports_line_number() {
        let css = ":root {\n  --base: #111111;\n  --text: #eeeeee;\n  --accent: tomato;\n}";
        let err = parse_css_theme("Broken", css).unwrap_err();
        assert!(err.contains("line 4"), "got: {err}");
    }
}
