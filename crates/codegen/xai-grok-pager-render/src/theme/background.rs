//! Canvas-background override, layered on top of a concrete theme.
//!
//! Built-in themes paint a full RGB canvas (`bg_base`) so elevated surfaces
//! (prompt cards, hover, code blocks) can sit a few luminance steps off a
//! known base. That also *covers* the user's terminal profile, which is why
//! a cream Ghostty/Kitty pane next to GrokDay's `#eeeeee` reads as two
//! different apps.
//!
//! [`BackgroundOverride`] lets the user keep a theme's accents and either:
//!
//! - **`terminal`** — leave `bg_base` / `bg_terminal` as [`Color::Reset`] so
//!   the emulator's own background (including transparency) shows through.
//! - **`#rrggbb`** — replace the canvas and shift the rest of the background
//!   ramp by the same per-channel delta, so cards/code/diffs stay related
//!   to the new color instead of sitting as leftover theme-gray boxes.
//!
//! Applied in [`Theme::current`] *after* polarity is sampled from the
//! original RGB `bg_base` (Reset would otherwise make [`Theme::is_dark`]
//! fall back to dark) and *before* quantization / Windows contrast boost.

use std::collections::HashMap;
use std::sync::Mutex;

use ratatui::style::Color;

use super::Theme;
use super::system_appearance::SystemAppearance;

/// Cached `[ui].background` / `GROK_BACKGROUND`. `None` = not loaded yet.
static OVERRIDE: Mutex<Option<BackgroundOverride>> = Mutex::new(None);

/// How the theme canvas should be painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackgroundOverride {
    /// Use the selected theme's own `bg_base` (default).
    Theme,
    /// Defer the canvas to the terminal (`Color::Reset`).
    Terminal,
    /// Paint this RGB as the new canvas and remap the background ramp.
    Rgb(u8, u8, u8),
}

impl Default for BackgroundOverride {
    fn default() -> Self {
        Self::Theme
    }
}

impl std::fmt::Display for BackgroundOverride {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Theme => f.write_str("theme"),
            Self::Terminal => f.write_str("terminal"),
            Self::Rgb(r, g, b) => write!(f, "#{r:02x}{g:02x}{b:02x}"),
        }
    }
}

impl BackgroundOverride {
    /// Parse a config / env value.
    ///
    /// Accepted:
    /// - empty / `theme` / `default` / `none` → [`Self::Theme`]
    /// - `terminal` / `transparent` / `inherit` / `reset` → [`Self::Terminal`]
    /// - `#rgb`, `#rrggbb`, or 3/6 hex digits without `#` → [`Self::Rgb`]
    ///
    /// Unknown strings return `None` so the caller can ignore a typo
    /// rather than silently treating it as the default.
    pub fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim();
        if s.is_empty() {
            return Some(Self::Theme);
        }
        let lower = s.to_ascii_lowercase();
        match lower.as_str() {
            "theme" | "default" | "none" => return Some(Self::Theme),
            "terminal" | "transparent" | "inherit" | "reset" => return Some(Self::Terminal),
            _ => {}
        }
        parse_hex_rgb(&lower).map(|(r, g, b)| Self::Rgb(r, g, b))
    }

    /// RGB used for [`Theme::is_dark`] instead of the theme's `bg_base`.
    ///
    /// Only [`Self::Rgb`] returns `Some` — `terminal` keeps the selected
    /// theme's polarity (Reset has no luminance).
    pub fn polarity_rgb(self) -> Option<(u8, u8, u8)> {
        match self {
            Self::Rgb(r, g, b) => Some((r, g, b)),
            Self::Theme | Self::Terminal => None,
        }
    }
}

/// Current canvas override. Seeds from env then `[ui].background` on first call.
#[must_use]
pub fn current() -> BackgroundOverride {
    let mut guard = OVERRIDE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(value) = *guard {
        return value;
    }
    let value = load();
    *guard = Some(value);
    value
}

/// Pin the in-memory override without touching disk.
pub fn set(value: BackgroundOverride) {
    *OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()) = Some(value);
}

/// Pin `Theme` so tests never pick up the developer's `config.toml`.
#[cfg(any(test, feature = "test-support"))]
pub fn reset_for_test() {
    set(BackgroundOverride::Theme);
}

fn load() -> BackgroundOverride {
    resolve(env_name().as_deref(), from_disk().as_deref())
}

fn env_name() -> Option<String> {
    env_name_from(&crate::host::collect_unicode_env()).map(str::to_owned)
}

fn env_name_from(env: &HashMap<String, String>) -> Option<&str> {
    for key in ["GROK_BACKGROUND", "LC_GROK_BACKGROUND"] {
        let Some(raw) = env
            .get(key)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if BackgroundOverride::parse(raw).is_some() {
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
        .and_then(|ui| ui.get("background"))
        .and_then(|v| v.as_str())
        .map(str::to_owned)
}

fn resolve(env_value: Option<&str>, config_value: Option<&str>) -> BackgroundOverride {
    env_value
        .and_then(BackgroundOverride::parse)
        .or_else(|| config_value.and_then(BackgroundOverride::parse))
        .unwrap_or(BackgroundOverride::Theme)
}

/// `#rgb`, `#rrggbb`, or the same without a leading `#`.
fn parse_hex_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some((r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some((r, g, b))
        }
        _ => None,
    }
}

/// Shift `color` by the same per-channel delta that takes `from` to `to`.
///
/// Non-RGB inputs (Reset, named ANSI, Indexed) pass through unchanged —
/// we never invent an RGB for a slot that was deliberately left to the
/// terminal or the 16-color palette.
fn shift_rgb(color: Color, from: Color, to: Color) -> Color {
    let Color::Rgb(fr, fg, fb) = from else {
        return color;
    };
    let Color::Rgb(tr, tg, tb) = to else {
        return color;
    };
    let Color::Rgb(cr, cg, cb) = color else {
        return color;
    };
    let d = |c: u8, f: u8, t: u8| -> u8 { (c as i16 + t as i16 - f as i16).clamp(0, 255) as u8 };
    Color::Rgb(d(cr, fr, tr), d(cg, fg, tg), d(cb, fb, tb))
}

impl Theme {
    /// Apply a canvas override. See the module docs for the contract.
    #[must_use]
    pub fn apply_background_override(self, background: BackgroundOverride) -> Self {
        match background {
            BackgroundOverride::Theme => self,
            BackgroundOverride::Terminal => Self {
                bg_base: Color::Reset,
                bg_terminal: Color::Reset,
                ..self
            },
            BackgroundOverride::Rgb(r, g, b) => {
                let from = self.bg_base;
                let to = Color::Rgb(r, g, b);
                let shift = |c: Color| shift_rgb(c, from, to);
                Self {
                    bg_base: to,
                    bg_light: shift(self.bg_light),
                    bg_dark: shift(self.bg_dark),
                    bg_highlight: shift(self.bg_highlight),
                    bg_hover: shift(self.bg_hover),
                    bg_terminal: shift(self.bg_terminal),
                    bg_visual: shift(self.bg_visual),
                    md_code_bg: shift(self.md_code_bg),
                    scrollbar_bg: shift(self.scrollbar_bg),
                    paste_bg: shift(self.paste_bg),
                    diff_delete_bg: shift(self.diff_delete_bg),
                    diff_insert_bg: shift(self.diff_insert_bg),
                    ..self
                }
            }
        }
    }

    /// Apply the cached canvas override and return `(theme, is_dark)`.
    ///
    /// Polarity is sampled *before* a `terminal` Reset would make
    /// [`Theme::is_dark`] fall back to dark. Called from [`Theme::current`]
    /// before quantization / Windows contrast boost.
    #[must_use]
    pub fn with_fork_canvas(self) -> (Self, bool) {
        let background = current();
        let dark = match background.polarity_rgb() {
            Some((r, g, b)) => super::osc11::classify_luminance(r, g, b) == SystemAppearance::Dark,
            None => self.is_dark(),
        };
        (self.apply_background_override(background), dark)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_theme_aliases() {
        for s in ["", "  ", "theme", "THEME", "default", "none"] {
            assert_eq!(
                BackgroundOverride::parse(s),
                Some(BackgroundOverride::Theme)
            );
        }
    }

    #[test]
    fn parse_terminal_aliases() {
        for s in ["terminal", "transparent", "inherit", "reset", " Terminal "] {
            assert_eq!(
                BackgroundOverride::parse(s),
                Some(BackgroundOverride::Terminal)
            );
        }
    }

    #[test]
    fn parse_hex() {
        assert_eq!(
            BackgroundOverride::parse("#fdf6e3"),
            Some(BackgroundOverride::Rgb(0xfd, 0xf6, 0xe3))
        );
        assert_eq!(
            BackgroundOverride::parse("fdf6e3"),
            Some(BackgroundOverride::Rgb(0xfd, 0xf6, 0xe3))
        );
        assert_eq!(
            BackgroundOverride::parse("#fff"),
            Some(BackgroundOverride::Rgb(255, 255, 255))
        );
        assert_eq!(
            BackgroundOverride::parse("#ABC"),
            Some(BackgroundOverride::Rgb(0xaa, 0xbb, 0xcc))
        );
    }

    #[test]
    fn parse_rejects_unknown() {
        assert_eq!(BackgroundOverride::parse("tokyonight"), None);
        assert_eq!(BackgroundOverride::parse("#gg0000"), None);
        assert_eq!(BackgroundOverride::parse("#ffff"), None);
        assert_eq!(BackgroundOverride::parse("not-a-color"), None);
    }

    #[test]
    fn display_round_trips_hex() {
        let parsed = BackgroundOverride::parse("#FDF6E3").unwrap();
        assert_eq!(parsed.to_string(), "#fdf6e3");
        assert_eq!(BackgroundOverride::parse(&parsed.to_string()), Some(parsed));
    }

    #[test]
    fn terminal_resets_only_the_canvas() {
        let base = Theme::grokday();
        let applied = base.apply_background_override(BackgroundOverride::Terminal);
        assert_eq!(applied.bg_base, Color::Reset);
        assert_eq!(applied.bg_terminal, Color::Reset);
        assert_eq!(applied.bg_light, base.bg_light);
        assert_eq!(applied.bg_dark, base.bg_dark);
        assert_eq!(applied.text_primary, base.text_primary);
        assert_eq!(applied.accent_assistant, base.accent_assistant);
    }

    #[test]
    fn rgb_sets_bg_base_and_shifts_ramp() {
        let base = Theme::grokday();
        let Color::Rgb(br, bg, bb) = base.bg_base else {
            panic!("grokday bg_base must be RGB");
        };
        let applied = base.apply_background_override(BackgroundOverride::Rgb(0xfd, 0xf6, 0xe3));
        assert_eq!(applied.bg_base, Color::Rgb(0xfd, 0xf6, 0xe3));
        // Same delta as bg_base, so the relative step is preserved.
        let Color::Rgb(lr, lg, lb) = base.bg_light else {
            panic!("grokday bg_light must be RGB");
        };
        let Color::Rgb(ar, ag, ab) = applied.bg_light else {
            panic!("remapped bg_light must stay RGB");
        };
        assert_eq!(ar as i16 - lr as i16, 0xfd - br as i16);
        assert_eq!(ag as i16 - lg as i16, 0xf6 - bg as i16);
        assert_eq!(ab as i16 - lb as i16, 0xe3 - bb as i16);
        // Accents stay put — this is a canvas override, not a new theme.
        assert_eq!(applied.accent_assistant, base.accent_assistant);
        assert_eq!(applied.text_primary, base.text_primary);
    }

    #[test]
    fn rgb_clamps_channels_that_would_underflow() {
        // Shift a near-black slot toward an even darker target: clamp at 0.
        let color = Color::Rgb(4, 4, 4);
        let from = Color::Rgb(20, 20, 20);
        let to = Color::Rgb(0, 0, 0);
        assert_eq!(shift_rgb(color, from, to), Color::Rgb(0, 0, 0));
    }

    #[test]
    fn rgb_clamps_channels_that_would_overflow() {
        let color = Color::Rgb(250, 250, 250);
        let from = Color::Rgb(238, 238, 238);
        let to = Color::Rgb(255, 255, 255);
        assert_eq!(shift_rgb(color, from, to), Color::Rgb(255, 255, 255));
    }

    #[test]
    fn shift_leaves_reset_and_named_alone() {
        let from = Color::Rgb(20, 20, 20);
        let to = Color::Rgb(0xfd, 0xf6, 0xe3);
        assert_eq!(shift_rgb(Color::Reset, from, to), Color::Reset);
        assert_eq!(shift_rgb(Color::Red, from, to), Color::Red);
    }

    #[test]
    fn theme_override_is_noop() {
        let base = Theme::groknight();
        assert_eq!(
            base.apply_background_override(BackgroundOverride::Theme)
                .bg_base,
            base.bg_base
        );
    }

    #[test]
    fn grokday_plus_solarized_light_stays_light() {
        // Solarized Light base3. Polarity of the *override* (not Reset)
        // must still read as light so ANSI16 chrome stays inverted.
        let (r, g, b) = (0xfd, 0xf6, 0xe3);
        assert_eq!(
            crate::theme::osc11::classify_luminance(r, g, b),
            crate::theme::system_appearance::SystemAppearance::Light
        );
        let applied = Theme::grokday().apply_background_override(BackgroundOverride::Rgb(r, g, b));
        assert!(!applied.is_dark());
    }

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn resolve_env_wins_over_config() {
        assert_eq!(
            resolve(Some("terminal"), Some("#fdf6e3")),
            BackgroundOverride::Terminal
        );
        assert_eq!(
            resolve(Some("#aabbcc"), Some("terminal")),
            BackgroundOverride::Rgb(0xaa, 0xbb, 0xcc)
        );
    }

    #[test]
    fn resolve_falls_through_to_config_then_theme() {
        assert_eq!(
            resolve(None, Some("terminal")),
            BackgroundOverride::Terminal
        );
        assert_eq!(resolve(None, None), BackgroundOverride::Theme);
        assert_eq!(
            resolve(Some("not-a-color"), Some("#fdf6e3")),
            BackgroundOverride::Rgb(0xfd, 0xf6, 0xe3)
        );
    }

    #[test]
    fn env_prefers_grok_over_lc() {
        let map = env(&[
            ("GROK_BACKGROUND", "terminal"),
            ("LC_GROK_BACKGROUND", "#ffffff"),
        ]);
        assert_eq!(env_name_from(&map), Some("terminal"));
    }

    #[test]
    fn env_skips_empty_and_invalid() {
        let map = env(&[
            ("GROK_BACKGROUND", ""),
            ("LC_GROK_BACKGROUND", "not-a-color"),
        ]);
        assert_eq!(env_name_from(&map), None);
        let map = env(&[("LC_GROK_BACKGROUND", "#fdf6e3")]);
        assert_eq!(env_name_from(&map), Some("#fdf6e3"));
    }

    #[test]
    fn cache_pins_and_reset_for_test() {
        set(BackgroundOverride::Terminal);
        assert_eq!(current(), BackgroundOverride::Terminal);
        reset_for_test();
        assert_eq!(current(), BackgroundOverride::Theme);
    }

    #[test]
    fn with_fork_canvas_applies_cached_terminal() {
        reset_for_test();
        set(BackgroundOverride::Terminal);
        let (theme, dark) = Theme::grokday().with_fork_canvas();
        assert!(!dark, "terminal override keeps GrokDay polarity");
        assert_eq!(theme.bg_base, Color::Reset);
        assert_eq!(theme.accent_assistant, Theme::grokday().accent_assistant);
        reset_for_test();
    }

    #[test]
    fn with_fork_canvas_applies_cached_rgb() {
        reset_for_test();
        set(BackgroundOverride::Rgb(0xfd, 0xf6, 0xe3));
        let (theme, dark) = Theme::grokday().with_fork_canvas();
        assert!(!dark);
        assert_eq!(theme.bg_base, Color::Rgb(0xfd, 0xf6, 0xe3));
        reset_for_test();
    }
}
