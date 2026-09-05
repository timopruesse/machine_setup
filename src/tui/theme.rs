use std::ffi::OsStr;

use ratatui::style::Color;

use crate::tui::state::TASK_PALETTE_LEN;

pub const DETAILS_MIN_WIDTH: u16 = 68;
pub const MIN_USABLE_HEIGHT: u16 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub accent: Color,
    pub accent_alt: Color,
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,
    pub muted: Color,
    pub text: Color,
    pub border: Color,
    pub border_focus: Color,
    pub gauge_bg: Color,
    /// Deeper fill for the progress gauge (bright Neon accents are too light for `text` labels).
    pub gauge_fill_run: Color,
    pub gauge_fill_ok: Color,
    pub gauge_fill_err: Color,
    pub task_palette: [Color; TASK_PALETTE_LEN],
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    pub fn neon() -> Self {
        Self {
            accent: rgb(0xe1, 0x35, 0xff),
            accent_alt: rgb(0x80, 0xff, 0xea),
            success: rgb(0x50, 0xfa, 0x7b),
            error: rgb(0xff, 0x63, 0x63),
            warning: rgb(0xf1, 0xfa, 0x8c),
            info: rgb(0x80, 0xff, 0xea),
            muted: rgb(0x82, 0x87, 0x9f),
            text: rgb(0xf8, 0xf8, 0xf2),
            border: rgb(0x3c, 0x3c, 0x50),
            border_focus: rgb(0xe1, 0x35, 0xff),
            gauge_bg: rgb(0x37, 0x32, 0x4b),
            // cyan_500 / deeper green / deeper red — readable under light `text` labels
            gauge_fill_run: rgb(0x3c, 0xb4, 0xa0),
            gauge_fill_ok: rgb(0x2d, 0xa0, 0x50),
            gauge_fill_err: rgb(0xc4, 0x44, 0x44),
            task_palette: [
                rgb(0xe1, 0x35, 0xff),
                rgb(0x80, 0xff, 0xea),
                rgb(0xff, 0x6a, 0xc1),
                rgb(0x50, 0xfa, 0x7b),
                rgb(0xf1, 0xfa, 0x8c),
                rgb(0xff, 0x55, 0xff),
                rgb(0xbd, 0x93, 0xf9),
                rgb(0xff, 0x99, 0xff),
            ],
        }
    }

    /// Named ANSI approximations of semantic slots (Cyan/Magenta/Green/Red/Yellow/DarkGray/White).
    pub fn mono() -> Self {
        Self {
            accent: Color::Magenta,
            accent_alt: Color::Cyan,
            success: Color::Green,
            error: Color::Red,
            warning: Color::Yellow,
            info: Color::Cyan,
            muted: Color::DarkGray,
            text: Color::White,
            border: Color::DarkGray,
            border_focus: Color::Magenta,
            gauge_bg: Color::DarkGray,
            gauge_fill_run: Color::Cyan,
            gauge_fill_ok: Color::Green,
            gauge_fill_err: Color::Red,
            task_palette: [
                Color::Magenta,
                Color::Cyan,
                Color::LightMagenta,
                Color::Green,
                Color::Yellow,
                Color::Magenta,
                Color::Blue,
                Color::LightMagenta,
            ],
        }
    }

    pub fn resolve() -> Self {
        if should_use_mono(std::env::var_os("NO_COLOR").as_deref()) {
            Self::mono()
        } else {
            Self::neon()
        }
    }

    pub fn task_color(&self, idx: usize) -> Color {
        self.task_palette[idx % TASK_PALETTE_LEN]
    }
}

/// Returns true when `NO_COLOR` is set to a non-empty value.
pub(crate) fn should_use_mono(no_color: Option<&OsStr>) -> bool {
    no_color.is_some_and(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neon_accent_is_electric_purple() {
        let t = Theme::neon();
        assert_eq!(t.accent, Color::Rgb(0xe1, 0x35, 0xff));
    }

    #[test]
    fn resolve_respects_nonempty_no_color() {
        // Test mono semantic slots directly (avoids parallel env mutation).
        let mono = Theme::mono();
        assert_eq!(mono.accent, Color::Magenta);
        assert_eq!(mono.accent_alt, Color::Cyan);
        assert_eq!(mono.success, Color::Green);
        assert_eq!(mono.error, Color::Red);
        assert_eq!(mono.warning, Color::Yellow);
        assert_eq!(mono.muted, Color::DarkGray);
        assert_eq!(mono.text, Color::White);
    }

    #[test]
    fn should_use_mono_logic() {
        use std::ffi::OsStr;
        assert!(should_use_mono(Some(OsStr::new("1"))));
        assert!(!should_use_mono(Some(OsStr::new(""))));
        assert!(!should_use_mono(None));
    }

    #[test]
    fn task_color_wraps_palette() {
        let t = Theme::neon();
        assert_eq!(t.task_color(0), t.task_palette[0]);
        assert_eq!(t.task_color(TASK_PALETTE_LEN), t.task_palette[0]);
    }
}
