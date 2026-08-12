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

use ratatui::style::Color;

use super::Theme;

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
}
