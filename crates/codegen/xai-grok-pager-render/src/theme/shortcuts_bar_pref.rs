//! Fork pref: whether the bottom shortcuts bar is reserved.
//!
//! Official Grok always paints a 1-row hint bar. This fork hides it unless
//! `[ui].show_shortcuts_bar` / `GROK_SHOW_SHORTCUTS_BAR` is explicitly on.
//! Not a `/settings` row — keep the hook out of the upstream schema.

use std::collections::HashMap;
use std::sync::Mutex;

/// Cached visibility. `None` = not loaded yet.
static VISIBLE: Mutex<Option<bool>> = Mutex::new(None);

/// Rows to reserve for the agent-view shortcuts bar (`0` or `1`).
#[must_use]
pub fn reserved_height() -> u16 {
    u16::from(is_visible())
}

/// Pin visibility without touching disk (tests).
pub fn set(visible: bool) {
    *VISIBLE.lock().unwrap_or_else(|e| e.into_inner()) = Some(visible);
}

/// Show the bar so layout tests that go through [`Theme::current`]-style
/// helpers keep a 1-row footer.
#[cfg(any(test, feature = "test-support"))]
pub fn reset_for_test() {
    set(true);
}

fn is_visible() -> bool {
    let mut guard = VISIBLE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(value) = *guard {
        return value;
    }
    let value = load();
    *guard = Some(value);
    value
}

fn load() -> bool {
    resolve(env_name().as_deref(), from_disk().as_deref())
}

/// Unset → hidden (this fork's default). `true`/`on`/`1`/`show` → visible.
fn resolve(env_value: Option<&str>, config_value: Option<&str>) -> bool {
    env_value
        .and_then(parse_bool)
        .or_else(|| config_value.and_then(parse_bool))
        .unwrap_or(false)
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => None,
        "1" | "true" | "on" | "yes" | "show" | "visible" => Some(true),
        "0" | "false" | "off" | "no" | "hide" | "hidden" => Some(false),
        _ => None,
    }
}

fn env_name() -> Option<String> {
    env_name_from(&crate::host::collect_unicode_env()).map(str::to_owned)
}

fn env_name_from(env: &HashMap<String, String>) -> Option<&str> {
    for key in ["GROK_SHOW_SHORTCUTS_BAR", "LC_GROK_SHOW_SHORTCUTS_BAR"] {
        let Some(raw) = env
            .get(key)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if parse_bool(raw).is_some() {
            return Some(raw);
        }
    }
    None
}

fn from_disk() -> Option<String> {
    let root = xai_grok_config::load_effective_config_disk_only().ok()?;
    let table = root.as_table()?;
    table
        .get("ui")
        .and_then(|ui| ui.get("show_shortcuts_bar"))
        .map(|v| {
            if let Some(b) = v.as_bool() {
                return if b { "true" } else { "false" }.to_string();
            }
            v.as_str().unwrap_or("").to_string()
        })
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_is_hidden() {
        assert!(!resolve(None, None));
    }

    #[test]
    fn env_wins_over_config() {
        assert!(resolve(Some("true"), Some("false")));
        assert!(!resolve(Some("off"), Some("true")));
    }

    #[test]
    fn parse_aliases() {
        for s in ["true", "ON", "1", "show", "visible"] {
            assert_eq!(parse_bool(s), Some(true), "{s}");
        }
        for s in ["false", "OFF", "0", "hide", "hidden"] {
            assert_eq!(parse_bool(s), Some(false), "{s}");
        }
        assert_eq!(parse_bool("maybe"), None);
    }

    #[test]
    fn env_prefers_grok_over_lc() {
        let env = HashMap::from([
            ("GROK_SHOW_SHORTCUTS_BAR".into(), "0".into()),
            ("LC_GROK_SHOW_SHORTCUTS_BAR".into(), "1".into()),
        ]);
        assert_eq!(env_name_from(&env), Some("0"));
    }
}
